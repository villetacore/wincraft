# Windows-host entry point for the WinCraft OS build:
#   1. Win32 test fixtures (x64 and, if the target is installed, x86) with rustc
#   2. the isolated builder chroot in WSL (first run only)
#   3. build/build.sh inside it: Buildroot + Rust components -> dist/wincraft-os.iso
# Extra arguments go to build.sh as Buildroot targets (e.g. linux-rebuild).
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Targets)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force -Path "$project\dist" | Out-Null
& rustc --edition 2021 -O --target x86_64-pc-windows-msvc "$project\tests\fixtures\win32-demo.rs" -o "$project\dist\win32-demo.exe"
if ($LASTEXITCODE -ne 0) { throw 'Win32 x64 fixture build failed' }
if ((& rustup target list --installed) -contains 'i686-pc-windows-msvc') {
    & rustc --edition 2021 -O --target i686-pc-windows-msvc "$project\tests\fixtures\win32-demo.rs" -o "$project\dist\win32-demo-x86.exe"
    if ($LASTEXITCODE -ne 0) { throw 'Win32 x86 fixture build failed' }
}
$linuxPath = (& wsl -d Ubuntu -- wslpath -a ($project -replace '\\', '/')).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Ubuntu WSL is required' }
# Native stderr must not become a terminating error (Windows PowerShell 5.1).
$ErrorActionPreference = 'Continue'
& wsl -d Ubuntu -u root -- test -f /var/lib/wincraft/builder/etc/os-release
if ($LASTEXITCODE -ne 0) {
    & wsl -d Ubuntu -u root -- bash "$linuxPath/build/builder.sh" setup
    if ($LASTEXITCODE -ne 0) { throw 'Builder setup failed' }
}
& wsl -d Ubuntu -u root -- bash "$linuxPath/build/builder.sh" run bash build/build.sh @Targets
if ($LASTEXITCODE -ne 0) { throw 'WinCraft OS build failed' }
