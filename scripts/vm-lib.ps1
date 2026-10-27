# Shared by the ISO acceptance scripts (smoke-iso.ps1, apps-iso.ps1): disposable UEFI VMs
# with serial console and QMP. Expects $qemuDir, $qemu, $iso, $dist, $Accel and $logPrefix
# in the dot-sourcing script.

function Free-Port {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop(); return $port
}

function Start-Vm([string]$name, [bool]$debug) {
    $vm = @{ Qmp = Free-Port; Serial = Free-Port; Vars = "$env:TEMP\wincraft-$name-vars-$PID.fd"; Log = '' }
    Copy-Item "$qemuDir\share\edk2-i386-vars.fd" $vm.Vars -Force
    $a = @('-name', "WinCraft-$name", '-machine', 'q35') + $(if ($Accel -eq 'auto') { @('-accel', 'whpx', '-accel', 'tcg') } else { @('-accel', $Accel) }) + @('-cpu', 'Nehalem', '-m', '4096', '-smp', '2',
        '-drive', "`"if=pflash,format=raw,unit=0,readonly=on,file=$qemuDir\share\edk2-x86_64-code.fd`"",
        '-drive', "`"if=pflash,format=raw,unit=1,file=$($vm.Vars)`"",
        '-cdrom', "`"$iso`"", '-boot', 'd',
        '-vga', 'std', '-device', 'qemu-xhci', '-device', 'usb-tablet', '-nic', 'user,model=virtio-net-pci', '-no-reboot', '-display', 'none',
        '-qmp', "tcp:127.0.0.1:$($vm.Qmp),server=on,wait=off", '-serial', "tcp:127.0.0.1:$($vm.Serial),server=on,wait=off")
    if ($debug) { $a += @('-fw_cfg', 'name=opt/wincraft/debug,string=1') }
    if (Test-Path -LiteralPath "$dist\data.ext4") { $a += @('-drive', "`"file=$dist\data.ext4,format=raw,if=virtio,snapshot=on`"") }
    $vm.Process = Start-Process -FilePath $qemu -ArgumentList $a -WindowStyle Hidden -PassThru -RedirectStandardError "$dist\qemu-$name-errors.log"
    $deadline = (Get-Date).AddSeconds(30)
    while (!$vm.Client -and (Get-Date) -lt $deadline) {
        try { $vm.Client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', $vm.Serial) } catch { Start-Sleep -Milliseconds 200 }
    }
    if (!$vm.Client) { throw "Serial connection failed ($name)" }
    $vm.Stream = $vm.Client.GetStream()
    $vm.LogFile = "$dist\$logPrefix-$name-serial.log"
    return $vm
}

# Reads serial output until $pattern matches the log (or timeout); returns whether it matched.
function Wait-Serial($vm, [string]$pattern, [int]$seconds) {
    $buffer = New-Object byte[] 65536
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ($vm.Log -match $pattern) { return $true }
        if ($vm.Process.HasExited) { return [bool]($vm.Log -match $pattern) }
        if ($vm.Stream.DataAvailable) {
            $n = $vm.Stream.Read($buffer, 0, $buffer.Length)
            $vm.Log += [Text.Encoding]::UTF8.GetString($buffer, 0, $n)
            [IO.File]::WriteAllText($vm.LogFile, $vm.Log)
            if ($vm.Log -match 'Kernel panic|WINCRAFT_EARLY_FAILED|WINCRAFT_INIT_FAILED') { throw "Guest boot failure; see $($vm.LogFile)" }
        } else { Start-Sleep -Milliseconds 200 }
    }
    return [bool]($vm.Log -match $pattern)
}

function Send-Serial($vm, [string]$text) {
    $bytes = [Text.Encoding]::ASCII.GetBytes($text + "`n"); $vm.Stream.Write($bytes, 0, $bytes.Length)
}

function Open-Qmp($vm) {
    $vm.QmpClient = [System.Net.Sockets.TcpClient]::new('127.0.0.1', $vm.Qmp)
    $vm.Reader = [IO.StreamReader]::new($vm.QmpClient.GetStream())
    $vm.Writer = [IO.StreamWriter]::new($vm.QmpClient.GetStream()); $vm.Writer.AutoFlush = $true
    $null = $vm.Reader.ReadLine()
    Invoke-Qmp $vm '{"execute":"qmp_capabilities"}' | Out-Null
}

function Invoke-Qmp($vm, [string]$json) {
    $vm.Writer.WriteLine($json)
    while ($true) { $line = $vm.Reader.ReadLine(); if ($line -notmatch '"event"') { return $line } }
}

function Save-Screen($vm, [string]$file) {
    $r = Invoke-Qmp $vm ((@{ execute = 'screendump'; arguments = @{ filename = $file; format = 'png' } }) | ConvertTo-Json -Compress)
    if ($r -match '"error"') { throw "screendump failed: $r" }
}

# Colour of one screenshot pixel, for layout checks that a changed screen alone cannot prove.
function Get-Pixel([string]$file, [int]$x, [int]$y) {
    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::FromFile($file)
    try { return $bitmap.GetPixel($x, $y) } finally { $bitmap.Dispose() }
}
function Test-Near($color, [int]$r, [int]$g, [int]$b, [int]$tolerance = 24) {
    return ([math]::Abs($color.R - $r) -le $tolerance) -and ([math]::Abs($color.G - $g) -le $tolerance) -and ([math]::Abs($color.B - $b) -le $tolerance)
}

function Stop-Vm($vm) {
    if (!$vm) { return }
    foreach ($c in @($vm.Client, $vm.QmpClient)) { if ($c) { $c.Dispose() } }
    if ($vm.Process -and !$vm.Process.HasExited) { Stop-Process -Id $vm.Process.Id -Force }
    Start-Sleep -Milliseconds 500
    Remove-Item -LiteralPath $vm.Vars -ErrorAction SilentlyContinue
}
