param([int]$TimeoutSeconds = 240)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
function Free-Port {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback,0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop(); return $port
}
$qmpPort=Free-Port
$serialPort=Free-Port
$argsList = @('-name','WinCraft-smoke','-machine','q35','-accel','tcg','-cpu','Nehalem','-m','4096','-smp','2',
    '-kernel',"`"$project\dist\vmlinuz`"",'-initrd',"`"$project\dist\initramfs.cpio.gz`"",
    '-append','"rdinit=/init console=tty0 console=ttyS0,115200 loglevel=4 wincraft.debug"',
    '-vga','std','-device','qemu-xhci','-device','usb-tablet','-nic','none','-no-reboot','-display','none',
    '-qmp',"tcp:127.0.0.1:$qmpPort,server=on,wait=off",'-serial',"tcp:127.0.0.1:$serialPort,server=on,wait=off")
$hasData=Test-Path -LiteralPath "$project\dist\data.ext4"
if ($hasData) { $argsList += @('-drive',"`"file=$project\dist\data.ext4,format=raw,if=virtio,snapshot=on`"") }
$process=Start-Process -FilePath $qemu -ArgumentList $argsList -WindowStyle Hidden -PassThru -RedirectStandardError "$project\dist\qemu-smoke-errors.log"
$serial=$null; $qmp=$null
try {
    $deadline=(Get-Date).AddSeconds($TimeoutSeconds)
    while (!$serial -and (Get-Date) -lt $deadline) {
        try { $serial=[System.Net.Sockets.TcpClient]::new('127.0.0.1',$serialPort) } catch { Start-Sleep -Milliseconds 200 }
    }
    if (!$serial) {throw 'Serial connection failed'}
    $stream=$serial.GetStream(); $buffer=New-Object byte[] 65536; $log=''
    while ((Get-Date) -lt $deadline -and $log -notmatch 'WINCRAFT_DESKTOP_READY') {
        if ($process.HasExited) {throw 'QEMU exited during boot'}
        if ($stream.DataAvailable) {
            $n=$stream.Read($buffer,0,$buffer.Length); $log += [Text.Encoding]::UTF8.GetString($buffer,0,$n)
            [IO.File]::WriteAllText("$project\dist\smoke-serial.log",$log)
            if ($log -match 'Kernel panic') {throw 'Guest kernel panic; see dist/smoke-serial.log'}
        }
        Start-Sleep -Milliseconds 200
    }
    [IO.File]::WriteAllText("$project\dist\smoke-serial.log",$log)
    if ($log -notmatch 'WINCRAFT_DESKTOP_READY') {throw 'Desktop did not become ready; inspect dist/smoke-serial.log'}
    # Execute real Windows PE in the guest, then observe its filesystem side effect.
    $wine='su-exec user env HOME=/home/user DISPLAY=:0 XAUTHORITY=/run/user/1000/Xauthority WINEPREFIX=/home/user/.wine WINEDEBUG=-all WINEDLLOVERRIDES=mscoree,mshtml= wine'
    $command="mountpoint -q /data && echo WINCRAFT_DATA_MOUNTED; $wine /apps/win32-demo.exe --smoke; cat /tmp/wincraft-win32-ok.txt; test -s /tmp/wincraft-win32-ok.txt && echo WINCRAFT_FILE_OK; echo GUEST_TEST_DONE`n"
    $bytes=[Text.Encoding]::ASCII.GetBytes($command); $stream.Write($bytes,0,$bytes.Length)
    $deadline=(Get-Date).AddSeconds(180); $after=''
    while ((Get-Date) -lt $deadline -and $after -notmatch '(?m)^GUEST_TEST_DONE\r?$') {
        if ($stream.DataAvailable) {
            $n=$stream.Read($buffer,0,$buffer.Length); $after += [Text.Encoding]::UTF8.GetString($buffer,0,$n)
            [IO.File]::WriteAllText("$project\dist\smoke-serial.log",$log+$after)
        }
        Start-Sleep -Milliseconds 200
    }
    $qmp=[System.Net.Sockets.TcpClient]::new('127.0.0.1',$qmpPort)
    $reader=[IO.StreamReader]::new($qmp.GetStream()); $writer=[IO.StreamWriter]::new($qmp.GetStream()); $writer.AutoFlush=$true
    $null=$reader.ReadLine()
    $writer.WriteLine('{"execute":"qmp_capabilities"}'); $null=$reader.ReadLine()
    $request=@{execute='screendump';arguments=@{filename="$project\dist\desktop.png";format='png'}} | ConvertTo-Json -Compress
    $writer.WriteLine($request); $response=$reader.ReadLine()
    if ($response -match 'error') {throw $response}
    if ($after -notmatch 'WINCRAFT_WIN32_OK pid=') {throw 'Win32 smoke failed; inspect serial log'}
    if ($after -notmatch '(?m)(?:^|ticks=\d+)WINCRAFT_FILE_OK\r?$') {throw 'Windows EXE did not create the expected guest file'}
    if ($hasData -and $after -notmatch '(?m)^WINCRAFT_DATA_MOUNTED\r?$') {throw 'Data disk failed to mount'}
    # Exercise the USB pointer by opening Files from the taskbar.
    $writer.WriteLine('{"execute":"input-send-event","arguments":{"events":[{"type":"abs","data":{"axis":"x","value":4448}},{"type":"abs","data":{"axis":"y","value":31685}}]}}')
    $null=$reader.ReadLine(); Start-Sleep -Milliseconds 300
    $writer.WriteLine('{"execute":"input-send-event","arguments":{"events":[{"type":"btn","data":{"down":true,"button":"left"}}]}}')
    $null=$reader.ReadLine(); Start-Sleep -Milliseconds 150
    $writer.WriteLine('{"execute":"input-send-event","arguments":{"events":[{"type":"btn","data":{"down":false,"button":"left"}}]}}')
    $null=$reader.ReadLine(); Start-Sleep -Seconds 2
    $writer.WriteLine((@{execute='screendump';arguments=@{filename="$project\dist\explorer.png";format='png'}} | ConvertTo-Json -Compress))
    $null=$reader.ReadLine()
    $bytes=[Text.Encoding]::ASCII.GetBytes("$wine /apps/win32-demo.exe >/tmp/win32-gui.log 2>&1 &`n"); $stream.Write($bytes,0,$bytes.Length)
    Start-Sleep -Seconds 6
    $writer.WriteLine((@{execute='screendump';arguments=@{filename="$project\dist\win32-demo.png";format='png'}} | ConvertTo-Json -Compress))
    $null=$reader.ReadLine()
    Write-Output 'PASS: Linux boot, Rust PID1, graphical desktop, Windows PE / Win32 execution'
} finally {
    if ($serial) {$serial.Dispose()}; if ($qmp) {$qmp.Dispose()}
    if (!$process.HasExited) { Stop-Process -Id $process.Id }
}
