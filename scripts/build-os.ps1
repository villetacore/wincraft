param(
    # Reuse the previous build chroot (fast iteration). Default is a fresh, clean builder.
    [switch]$Reuse,
    # Skip the developer direct-boot initramfs; only the live ISO is produced.
    [switch]$SkipDevInitramfs
)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force -Path "$project\dist" | Out-Null
& rustc --edition 2021 --target x86_64-pc-windows-msvc "$project\examples\win32-demo.rs" -o "$project\dist\win32-demo.exe"
if ($LASTEXITCODE -ne 0) { throw 'Windows demo build failed' }
# 32-bit fixture (COMP-01) when the i686 target is installed: rustup target add i686-pc-windows-msvc
if ((& rustup target list --installed) -contains 'i686-pc-windows-msvc') {
    & rustc --edition 2021 --target i686-pc-windows-msvc "$project\examples\win32-demo.rs" -o "$project\dist\win32-demo-x86.exe"
    if ($LASTEXITCODE -ne 0) { throw 'Windows x86 demo build failed' }
}
$linuxPath = (& wsl -d Ubuntu -- wslpath -a ($project -replace '\\', '/')).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Ubuntu WSL is required' }
$envs = @()
if ($Reuse) { $envs += 'WINCRAFT_REUSE_ROOT=1' }
if ($SkipDevInitramfs) { $envs += 'WINCRAFT_SKIP_INITRAMFS=1' }
# Native stderr must not become a terminating error (Windows PowerShell 5.1 with redirection).
$ErrorActionPreference = 'Continue'
& wsl -d Ubuntu -u root -- env @envs bash "$linuxPath/scripts/build-os.sh"
if ($LASTEXITCODE -ne 0) { throw 'OS build failed' }
