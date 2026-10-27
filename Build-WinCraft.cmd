@echo off
rem Builds WinCraft OS from source (WSL builder + Buildroot). The first build takes hours.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build\build.ps1" %*
pause
