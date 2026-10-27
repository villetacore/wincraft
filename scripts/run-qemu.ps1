param([switch]$Headless, [switch]$NoData, [int]$Memory = 4096)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
if (!(Test-Path -LiteralPath $qemu)) { $qemu = (Get-Command qemu-system-x86_64 -ErrorAction Stop).Source }
foreach ($file in @('vmlinuz','initramfs.cpio.gz')) {
    if (!(Test-Path -LiteralPath "$project\dist\$file")) { throw "Build the OS first: missing dist/$file" }
}
$qemuArgs = @('-name','WinCraft Unix','-machine','q35','-accel','tcg','-cpu','Nehalem','-m',"$Memory",'-smp','2',
    '-kernel',"$project\dist\vmlinuz",'-initrd',"$project\dist\initramfs.cpio.gz",
    '-append','rdinit=/init console=tty0 console=ttyS0,115200 loglevel=4 wincraft.debug',
    '-vga','std','-device','qemu-xhci','-device','usb-tablet','-nic','none','-no-reboot')
if (!$NoData -and (Test-Path -LiteralPath "$project\dist\data.ext4")) {
    $qemuArgs += @('-drive',"file=$project\dist\data.ext4,format=raw,if=virtio")
}
if ($Headless) { $qemuArgs += @('-display','none','-serial','stdio','-monitor','none') }
else { $qemuArgs += @('-display','gtk','-serial',"file:$project\dist\serial.log") }
& $qemu @qemuArgs
exit $LASTEXITCODE
