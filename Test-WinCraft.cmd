@echo off
rem Automatic acceptance test of dist\wincraft-live.iso in throwaway VMs (several minutes).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\smoke-iso.ps1" %*
pause
