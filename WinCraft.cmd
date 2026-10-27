@echo off
rem Boots the WinCraft live ISO in QEMU (UEFI). Close the QEMU window or use Settings - Shut down to exit.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-iso.ps1" %*
if errorlevel 1 pause
