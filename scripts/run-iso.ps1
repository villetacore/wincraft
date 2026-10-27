# Boots the WinCraft ISO in QEMU like a real PC: UEFI firmware, CD-ROM, no -kernel.
# Hardware acceleration (WHPX) is used when Windows provides it, otherwise TCG emulation.
# The distribution is built for x86-64-v2 (BR2_x86_x86_64_v2): QEMU's default qemu64 CPU
# lacks SSE4.2/POPCNT and glibc refuses to start, so a v2 model (Nehalem) is used.
param(
    [switch]$Bios,      # legacy BIOS (SeaBIOS) instead of UEFI
    [switch]$NoData,    # do not attach dist/data.ext4 even if it exists
    [int]$Memory = 8192,
    [int]$Cpus = 4,
    [string]$Iso = "",
    [ValidateSet('auto', 'tcg', 'whpx')][string]$Accel = 'auto',
    # GTK by default: QEMU's SDL window on Windows freezes (window and guest) when the
    # kernel switches the display mode after GRUB, at least together with WHPX.
    [ValidateSet('gtk', 'sdl')][string]$Display = 'gtk'
)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$qemuDir = 'C:\Program Files\qemu'
$qemu = "$qemuDir\qemu-system-x86_64.exe"
if (!(Test-Path -LiteralPath $qemu)) { $qemu = (Get-Command qemu-system-x86_64 -ErrorAction Stop).Source; $qemuDir = Split-Path $qemu }
# The own distribution when built, otherwise the transitional 0.3 image.
if (!$Iso) { $Iso = @("$project\dist\wincraft-os.iso", "$project\dist\wincraft-live.iso") | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
if (!$Iso) { throw 'Build the OS first: Build-WinCraft.cmd' }
Write-Host "Booting $Iso"
$qemuArgs = @('-name', 'WinCraft OS', '-machine', 'q35') + $(if ($Accel -eq 'auto') { @('-accel', 'whpx', '-accel', 'tcg') } else { @('-accel', $Accel) }) + @('-cpu', 'Nehalem', '-m', "$Memory", '-smp', "$Cpus",
    '-cdrom', $Iso, '-boot', 'd', '-vga', 'std', '-device', 'qemu-xhci', '-device', 'usb-tablet',
    '-nic', 'user,model=virtio-net-pci', '-audiodev', 'dsound,id=snd0', '-device', 'intel-hda', '-device', 'hda-duplex,audiodev=snd0', '-display', $Display, '-serial', "file:$project\dist\serial.log")
if (!$Bios) {
    $vars = "$project\dist\uefi-vars.fd"
    if (!(Test-Path -LiteralPath $vars)) { Copy-Item "$qemuDir\share\edk2-i386-vars.fd" $vars }
    $qemuArgs += @('-drive', "if=pflash,format=raw,unit=0,readonly=on,file=$qemuDir\share\edk2-x86_64-code.fd",
        '-drive', "if=pflash,format=raw,unit=1,file=$vars")
}
if (!$NoData -and (Test-Path -LiteralPath "$project\dist\data.ext4")) {
    $qemuArgs += @('-drive', "file=$project\dist\data.ext4,format=raw,if=virtio")
}
& $qemu @qemuArgs
exit $LASTEXITCODE
