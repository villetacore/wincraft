# ISO-02 acceptance run for dist/wincraft-live.iso in disposable UEFI VMs (no -kernel/-initrd).
# Boot 1 (debug, via QEMU fw_cfg): early init handover, overlay root, desktop UID 1000,
#   Wayland compositor as UID 1000 (no Xorg), Windows PE x64 (+ x86 report), screenshots, managed poweroff.
# Boot 2 (default menu entry = release): no root shell on the serial port.
param([int]$TimeoutSeconds = 420, [switch]$SkipReleaseBoot, [ValidateSet('auto', 'tcg', 'whpx')][string]$Accel = 'auto', [string]$Iso = '')
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$dist = "$project\dist"
$qemuDir = 'C:\Program Files\qemu'
$qemu = "$qemuDir\qemu-system-x86_64.exe"
$iso = if ($Iso) { $Iso } else { @("$dist\wincraft-os.iso", "$dist\wincraft-live.iso") | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
if (!$iso) { throw "No ISO in dist; run Build-WinCraft.cmd" }
Write-Output "Testing $iso"

$logPrefix = 'smoke-iso'
. "$PSScriptRoot\vm-lib.ps1"

$results = [ordered]@{}
$vm = $null
try {
    # ---- Boot 1: debug ---------------------------------------------------------------
    $started = Get-Date
    $vm = Start-Vm 'debug' $true
    if (!(Wait-Serial $vm 'WINCRAFT_EARLY_HANDOVER' 240)) { throw "No early-init handover; see $($vm.LogFile)" }
    if (!(Wait-Serial $vm 'WINCRAFT_INIT_READY pid=1 mode=debug' 120)) { throw 'System init did not report debug mode' }
    if (!(Wait-Serial $vm 'WINCRAFT_DESKTOP_READY' $TimeoutSeconds)) { throw "Desktop did not become ready; see $($vm.LogFile)" }
    $results["Boot to desktop (debug, $Accel)"] = "{0:N0} s" -f ((Get-Date) - $started).TotalSeconds
    # Runs a command as the desktop user with the session environment (su exists in every image).
    # The Wayland and XWayland socket names come from the file winlogon's compositor writes.
    Send-Serial $vm 'asuser() { su user -s /bin/sh -c "set -a; . /run/user/1000/wincraft-session.env; set +a; export HOME=/home/user XDG_RUNTIME_DIR=/run/user/1000 WINEPREFIX=/home/user/.wine WINEDEBUG=-all; $*"; }'
    $wine = 'asuser wine'
    Send-Serial $vm ("D=`$(pidof -s explorer); echo WC''_DESKTOP_UID=`$(awk '/^Uid:/{print `$2}' /proc/`$D/status); " +
        "C=`$(pidof -s labwc); echo WC''_COMPOSITOR_UID=`$(awk '/^Uid:/{print `$2}' /proc/`$C/status); " +
        "pidof Xorg >/dev/null && echo WC''_XORG_PRESENT; " +
        "grep -q '^overlay / overlay' /proc/mounts && echo WC''_ROOT_OVERLAY; " +
        # FIX-01: the prefix state file says preparing / ready / failed (with a reason).
        "S=/run/user/1000/wine-prefix.state; n=0; while ! grep -qE '^state=(ready|failed)' `$S 2>/dev/null && [ `$n -lt 400 ]; do sleep 1; n=`$((n+1)); done; echo WC''_PREFIX_WAIT=`$n; " +
        "sed 's/^/WC_PREFIX_/' `$S; " +
        # APP-01: the launcher runs the EXE, follows it and journals its exit status.
        "asuser start /apps/win32-demo.exe --smoke; echo WC''_START_EXIT=`$?; " +
        "grep 'event=exit' /home/user/.local/state/wincraft/launches.log | grep -q 'status=0' && echo WC''_JOURNAL_OK; rm -f /tmp/wincraft-win32-ok.txt; " +
        # APP-02: a DOS program is refused with an explanation, not handed to Wine.
        "printf 'MZ' > /tmp/dos.exe; chmod 644 /tmp/dos.exe; asuser start /tmp/dos.exe 2>/dev/null; echo WC''_DOS_EXIT=`$?; " +
        "grep 'event=error' /home/user/.local/state/wincraft/launches.log | grep -q 'DOS program' && echo WC''_DOS_EXPLAINED; " +
        "$wine /apps/win32-demo.exe --smoke; stat -c WC''_X64_FILE_UID=%u /tmp/wincraft-win32-ok.txt; rm -f /tmp/wincraft-win32-ok.txt; " +
        # A data disk replaces /apps; the read-only system layer still has the image's fixtures.
        "X=/apps/win32-demo-x86.exe; test -f `$X || X=/media/wincraft-lower/apps/win32-demo-x86.exe; " +
        "if test -f `$X; then $wine `$X --smoke; stat -c WC''_X86_FILE_UID=%u /tmp/wincraft-win32-ok.txt; else echo WC''_X86_MISSING; fi; " +
        "echo GUEST''_TEST_DONE")
    if (!(Wait-Serial $vm 'GUEST_TEST_DONE' 700)) { throw 'Guest checks timed out' }
    if ($vm.Log -match 'WC_PREFIX_WAIT=(\d+)') { $results['Wine prefix ready after desktop'] = "$($Matches[1]) s" }
    $log = $vm.Log
    $check = {
        param($name, $pattern)
        $ok = $log -match $pattern
        $results[$name] = if ($ok) { 'PASS' } else { 'FAIL' }
        return $ok
    }
    $pass = $true
    $pass = (& $check 'Root is squashfs + RAM overlay' 'WC_ROOT_OVERLAY') -and $pass
    $pass = (& $check 'Desktop runs as UID 1000' 'WC_DESKTOP_UID=1000') -and $pass
    $pass = (& $check 'Wayland compositor runs as UID 1000' 'WC_COMPOSITOR_UID=1000') -and $pass
    $results['No Xorg server'] = if ($log -match 'WC_XORG_PRESENT') { 'FAIL' } else { 'PASS' }
    $pass = $pass -and ($log -notmatch 'WC_XORG_PRESENT')
    $pass = (& $check 'Win32 x64 EXE runs (kernel32)' 'WINCRAFT_WIN32_OK pid=') -and $pass
    $pass = (& $check 'EXE file written as UID 1000' 'WC_X64_FILE_UID=1000') -and $pass
    $pass = (& $check 'Wine prefix state: ready with runtime' 'WC_PREFIX_state=ready\s+WC_PREFIX_runtime=wine-') -and $pass
    $pass = (& $check 'Launcher: EXE exit status 0, journaled' 'WC_START_EXIT=0[\s\S]*WC_JOURNAL_OK') -and $pass
    $pass = (& $check 'Launcher: DOS program refused with reason' 'WC_DOS_EXIT=1[\s\S]*WC_DOS_EXPLAINED') -and $pass
    if (Test-Path "$dist\win32-demo-x86.exe") {
        # Reported, not gating: 32-bit support depends on the Wine build (COMP-01).
        $results['Win32 x86 EXE (informational)'] = if ($log -match 'WC_X86_FILE_UID=1000') { 'PASS' }
            elseif ($log -match 'WC_X86_MISSING') { 'fixture not in image' } else { 'FAIL (see serial log)' }
    }

    Open-Qmp $vm
    Save-Screen $vm "$dist\iso-desktop.png"
    # Click the pinned File Explorer button, second after the browser (taskbar at 1280x800;
    # absolute coordinates 0..32767).
    Invoke-Qmp $vm '{"execute":"input-send-event","arguments":{"events":[{"type":"abs","data":{"axis":"x","value":2970}},{"type":"abs","data":{"axis":"y","value":31783}}]}}' | Out-Null
    Start-Sleep -Seconds 2   # hover first: the guest is busy initializing Wine
    Invoke-Qmp $vm '{"execute":"input-send-event","arguments":{"events":[{"type":"btn","data":{"down":true,"button":"left"}}]}}' | Out-Null
    Start-Sleep -Milliseconds 400
    Invoke-Qmp $vm '{"execute":"input-send-event","arguments":{"events":[{"type":"btn","data":{"down":false,"button":"left"}}]}}' | Out-Null
    # A shell window is its own process; under TCG it can take several seconds to appear.
    $deadline = (Get-Date).AddSeconds(40)
    do {
        Start-Sleep -Seconds 2
        Save-Screen $vm "$dist\iso-explorer.png"
        $opened = Test-Near (Get-Pixel "$dist\iso-explorer.png" 700 400) 0xFF 0xFF 0xFF 12
    } until ($opened -or (Get-Date) -gt $deadline)
    # Taskbar: light strip along the bottom edge (1280x800, 48 px high), not wallpaper.
    $taskbar = Test-Near (Get-Pixel "$dist\iso-desktop.png" 900 790) 0xEE 0xF1 0xF5
    $results['Taskbar visible at the bottom'] = if ($taskbar) { 'PASS' } else { 'FAIL' }
    # The click opened a File Explorer window (white content area where wallpaper was).
    $window = $opened
    $results['Click on taskbar opens File Explorer'] = if ($window) { 'PASS' } else { 'FAIL' }
    $pass = $pass -and $taskbar -and $window
    Send-Serial $vm "asuser 'wine /apps/win32-demo.exe' >/tmp/win32-gui.log 2>&1 &"
    # The MessageBoxW window must reach the window manager (and so the taskbar).
    Send-Serial $vm 'n=0; until asuser "explorer --list-windows" | grep -q "Win32 demo" || [ $n -ge 60 ]; do sleep 1; n=$((n+1)); done; asuser "explorer --list-windows" | grep -q "Win32 demo" && echo WC""_GUI_WINDOW_OK; echo GUI""_CHECK_DONE'
    $null = Wait-Serial $vm 'GUI_CHECK_DONE' 120
    $results['Win32 GUI window (MessageBoxW) shown'] = if ($vm.Log -match 'WC_GUI_WINDOW_OK') { 'PASS' } else { 'FAIL' }
    $pass = $pass -and ($vm.Log -match 'WC_GUI_WINDOW_OK')
    Start-Sleep -Seconds 2
    Save-Screen $vm "$dist\iso-win32-demo.png"

    # DESK-00 (needs internet on the host): DHCP through QEMU's NAT, then WinCraft Browser
    # loads an HTTPS page; its window, titled by the page, reaches the taskbar.
    Send-Serial $vm ('n=0; until grep -q nameserver /etc/resolv.conf 2>/dev/null || [ $n -ge 30 ]; do sleep 1; n=$((n+1)); done; ' +
        'wget -q -O /dev/null http://example.org/ && echo WC""_NET_OK; ' +
        'asuser "browser https://example.org/" >/tmp/browser.log 2>&1 & ' +
        'n=0; until asuser "explorer --list-windows" | grep -q "Example Domain" || [ $n -ge 90 ]; do sleep 1; n=$((n+1)); done; ' +
        'asuser "explorer --list-windows" | grep -q "Example Domain" && echo WC""_BROWSER_OK; echo NET""_CHECK_DONE')
    $null = Wait-Serial $vm 'NET_CHECK_DONE' 200
    $results['Network: DHCP + HTTP (needs internet)'] = if ($vm.Log -match 'WC_NET_OK') { 'PASS' } else { 'FAIL' }
    $results['Browser opens an HTTPS page'] = if ($vm.Log -match 'WC_BROWSER_OK') { 'PASS' } else { 'FAIL' }
    $pass = $pass -and ($vm.Log -match 'WC_NET_OK') -and ($vm.Log -match 'WC_BROWSER_OK')
    Start-Sleep -Seconds 3
    Save-Screen $vm "$dist\iso-browser.png"

    # Managed shutdown requested by the unprivileged user, as the Settings button does.
    Send-Serial $vm 'asuser touch /run/wincraft/control/poweroff'
    $null = Wait-Serial $vm 'WINCRAFT_SHUTDOWN poweroff' 30
    $off = $vm.Process.WaitForExit(60000)
    $results['Poweroff requested by user, VM exits'] = if ($off -and $vm.Log -match 'WINCRAFT_SHUTDOWN poweroff') { 'PASS' } else { 'FAIL' }
    $pass = $pass -and $off
    Stop-Vm $vm; $vm = $null

    # ---- Boot 2: release (default menu entry) ------------------------------------------
    if (!$SkipReleaseBoot) {
        $vm = Start-Vm 'release' $false
        if (!(Wait-Serial $vm 'WINCRAFT_INIT_READY pid=1 mode=release' 360)) { throw "Release boot did not reach init; see $($vm.LogFile)" }
        $null = Wait-Serial $vm 'WINCRAFT_DESKTOP_READY' $TimeoutSeconds
        Send-Serial $vm 'echo SHELL_PROBE_$((40+2))'
        $shell = Wait-Serial $vm 'SHELL_PROBE_42' 15
        $results['Release boot: no serial root shell'] = if ($shell) { 'FAIL' } else { 'PASS' }
        $results['Release boot: desktop ready'] = if ($vm.Log -match 'WINCRAFT_DESKTOP_READY') { 'PASS' } else { 'FAIL' }
        $pass = $pass -and !$shell -and ($vm.Log -match 'WINCRAFT_DESKTOP_READY')
        Stop-Vm $vm; $vm = $null
    }
} finally {
    Stop-Vm $vm
    $results.GetEnumerator() | ForEach-Object { '{0,-40} {1}' -f $_.Key, $_.Value }
}
if (!$pass) { throw 'ISO smoke test FAILED' }
Write-Output 'PASS: WinCraft live ISO (UEFI) - overlay root, Wayland user session, Win32 EXE, managed shutdown, release lockdown'
