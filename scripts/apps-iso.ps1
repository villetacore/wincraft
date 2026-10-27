# COMP-02 / APP-09: real Windows programs in a disposable VM, as a user would use them.
#   7-Zip:      download -> silent install -> files in C:\Program Files, Start menu shortcut,
#               File Manager window on the taskbar.
#   Cave Story: download zip -> Explorer "Extract All" -> 32-bit DirectDraw game window that
#               keeps running.
# Programs come from tests/apps/catalog.tsv (pinned SHA-256), cached in dist/apps; the guest
# fetches them from a small HTTP server on the host (10.0.2.2 in QEMU's NAT network).
param([ValidateSet('auto', 'tcg', 'whpx')][string]$Accel = 'auto', [string]$Iso = '', [int]$TimeoutSeconds = 420)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$dist = "$project\dist"
$qemuDir = 'C:\Program Files\qemu'
$qemu = "$qemuDir\qemu-system-x86_64.exe"
$iso = if ($Iso) { $Iso } else { @("$dist\wincraft-os.iso", "$dist\wincraft-live.iso") | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
if (!$iso) { throw "No ISO in dist; run Build-WinCraft.cmd" }
$logPrefix = 'apps-iso'
. "$PSScriptRoot\vm-lib.ps1"
Write-Output "Testing programs on $iso"

# ---- Programs: cached download with pinned hashes ----------------------------------------
$apps = "$dist\apps"
New-Item -ItemType Directory -Force -Path $apps | Out-Null
$catalog = Get-Content "$project\tests\apps\catalog.tsv" | Where-Object { $_ -and $_ -notmatch '^(#|id\t)' } | ForEach-Object {
    $f = $_ -split "`t"; [pscustomobject]@{ Id = $f[0]; Name = $f[1]; Url = $f[4]; File = $f[5]; Sha256 = $f[6] }
}
foreach ($app in $catalog) {
    $path = "$apps\$($app.File)"
    if (!(Test-Path -LiteralPath $path)) {
        Write-Output "Downloading $($app.Name) from $($app.Url)"
        Invoke-WebRequest -Uri $app.Url -OutFile $path -UseBasicParsing
    }
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLower()
    if ($hash -ne $app.Sha256) { Remove-Item -LiteralPath $path; throw "$($app.File): SHA-256 $hash does not match the catalog" }
}

# ---- Host HTTP server for the guest (GET of one file name from dist/apps) ------------------
$port = Free-Port
$server = Start-Job -ArgumentList $port, $apps -ScriptBlock {
    param($port, $root)
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $port)
    $listener.Start()
    while ($true) {
        $client = $listener.AcceptTcpClient()
        try {
            $stream = $client.GetStream()
            $reader = [IO.StreamReader]::new($stream)
            $request = $reader.ReadLine()
            while (($line = $reader.ReadLine()) -ne '' -and $line -ne $null) {}
            $name = [IO.Path]::GetFileName(([uri]"http://x$(($request -split ' ')[1])").AbsolutePath)
            $file = Join-Path $root $name
            if ($name -and (Test-Path -LiteralPath $file -PathType Leaf)) {
                $bytes = [IO.File]::ReadAllBytes($file)
                $head = [Text.Encoding]::ASCII.GetBytes("HTTP/1.0 200 OK`r`nContent-Length: $($bytes.Length)`r`nContent-Type: application/octet-stream`r`n`r`n")
                $stream.Write($head, 0, $head.Length); $stream.Write($bytes, 0, $bytes.Length)
            } else {
                $head = [Text.Encoding]::ASCII.GetBytes("HTTP/1.0 404 Not Found`r`nContent-Length: 0`r`n`r`n")
                $stream.Write($head, 0, $head.Length)
            }
        } catch {} finally { $client.Close() }
    }
}

$results = [ordered]@{}
$vm = $null
$pass = $true
try {
    $vm = Start-Vm 'apps' $true
    if (!(Wait-Serial $vm 'WINCRAFT_DESKTOP_READY' $TimeoutSeconds)) { throw "Desktop did not become ready; see $($vm.LogFile)" }
    Send-Serial $vm 'asuser() { su user -s /bin/sh -c "set -a; . /run/user/1000/wincraft-session.env; set +a; export HOME=/home/user XDG_RUNTIME_DIR=/run/user/1000 WINEPREFIX=/home/user/.wine WINEDEBUG=-all; $*"; }'
    Send-Serial $vm ("H=http://10.0.2.2:$port; D=/home/user/Downloads; mkdir -p `$D; chown 1000:1000 `$D; " +
        "n=0; while ! grep -qE '^state=(ready|failed)' /run/user/1000/wine-prefix.state 2>/dev/null && [ `$n -lt 400 ]; do sleep 1; n=`$((n+1)); done; " +
        "echo WC''_PREFIX_`$(head -1 /run/user/1000/wine-prefix.state); echo READY''_DONE")
    if (!(Wait-Serial $vm 'READY_DONE' 450)) { throw 'Wine prefix did not settle' }

    # 7-Zip: the installer through the launcher (as a double click would), silently.
    Send-Serial $vm ('asuser "wget -q -O $D/7z2604-x64.exe $H/7z2604-x64.exe"; sha256sum $D/7z2604-x64.exe | cut -c1-64 | sed "s/^/WC_7Z_SHA=/"; ' +
        'asuser "start $D/7z2604-x64.exe /S"; echo WC''_7Z_SETUP_EXIT=$?; ' +
        'P="/home/user/.wine/drive_c/Program Files/7-Zip"; test -f "$P/7zFM.exe" && echo WC''_7Z_FILES; ' +
        'find /home/user/.wine/drive_c -iname "7-Zip File Manager.lnk" | grep -q . && echo WC''_7Z_SHORTCUT; ' +
        "asuser `"start '`$P/7zFM.exe'`" >/dev/null 2>&1 & " +
        'n=0; until asuser "explorer --list-windows" | grep -qi 7zfm || [ $n -ge 60 ]; do sleep 1; n=$((n+1)); done; ' +
        'asuser "explorer --list-windows" | grep -i 7zfm | sed "s/^/WC_7Z_WINDOW /"; echo SEVENZIP''_DONE')
    $null = Wait-Serial $vm 'SEVENZIP_DONE' 300
    $log = $vm.Log
    $sevenZip = $catalog | Where-Object Id -eq '7zip'
    $checks = [ordered]@{
        '7-Zip: download hash matches' = "WC_7Z_SHA=$($sevenZip.Sha256)"
        '7-Zip: installer exit status 0' = 'WC_7Z_SETUP_EXIT=0'
        '7-Zip: installed in C:\Program Files' = 'WC_7Z_FILES'
        '7-Zip: Start menu shortcut' = 'WC_7Z_SHORTCUT'
        '7-Zip: File Manager window' = 'WC_7Z_WINDOW \d+'
    }
    Open-Qmp $vm
    Start-Sleep -Seconds 3
    Save-Screen $vm "$dist\apps-7zip.png"

    # Cave Story: zip -> Extract All -> 32-bit game.
    Send-Serial $vm ('asuser "wget -q -O $D/cavestoryen.zip $H/cavestoryen.zip"; sha256sum $D/cavestoryen.zip | cut -c1-64 | sed "s/^/WC_CS_SHA=/"; ' +
        'asuser "explorer --extract $D/cavestoryen.zip"; G=$D/cavestoryen/CaveStory/Doukutsu.exe; n=0; until test -f $G || [ $n -ge 30 ]; do sleep 1; n=$((n+1)); done; test -f $G && echo WC''_CS_EXTRACTED; ' +
        'asuser "start $G" >/tmp/game.log 2>&1 & ' +
        'n=0; until asuser "explorer --list-windows" | grep -qi doukutsu || [ $n -ge 90 ]; do sleep 1; n=$((n+1)); done; ' +
        'asuser "explorer --list-windows" | grep -i doukutsu | sed "s/^/WC_CS_WINDOW /"; sleep 20; ' +
        'pgrep -f Doukutsu.exe >/dev/null && echo WC''_CS_RUNNING; echo GAME''_DONE')
    $null = Wait-Serial $vm 'GAME_DONE' 300
    $log = $vm.Log
    $caveStory = $catalog | Where-Object Id -eq 'cavestory'
    $checks['Cave Story: download hash matches'] = "WC_CS_SHA=$($caveStory.Sha256)"
    $checks['Cave Story: zip extracted (Extract All)'] = 'WC_CS_EXTRACTED'
    $checks['Cave Story: game window (32-bit)'] = 'WC_CS_WINDOW \d+'
    $checks['Cave Story: still running after 20 s'] = 'WC_CS_RUNNING'
    Save-Screen $vm "$dist\apps-cavestory.png"
    foreach ($name in $checks.Keys) {
        $ok = $log -match $checks[$name]
        $results[$name] = if ($ok) { 'PASS' } else { 'FAIL' }
        $pass = $pass -and $ok
    }
    if ($log -match 'WC_PREFIX_state=(\w+)') { $results['Wine prefix'] = $Matches[1] }
} finally {
    Stop-Vm $vm
    Stop-Job $server -ErrorAction SilentlyContinue; Remove-Job $server -Force -ErrorAction SilentlyContinue
    $results.GetEnumerator() | ForEach-Object { '{0,-42} {1}' -f $_.Key, $_.Value }
}
if (!$pass) { throw 'Program scenarios FAILED' }
Write-Output 'PASS: 7-Zip installs and runs, Cave Story extracts and runs (screenshots: dist\apps-*.png)'
