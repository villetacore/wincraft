Welcome to WinCraft OS 0.4 (developer live preview)

This machine runs a Linux kernel, a Rust PID 1 and a Rust desktop.
The desktop runs as the ordinary account "user" (UID 1000), not as root.
The system image is read-only; changes live in RAM and vanish at shutdown
unless a disk labelled WINCRAFT_DATA is attached (then /home and /apps persist).
Use Files to open text files and Windows executables (try /apps/win32-demo.exe).
Windows APIs are supplied by Wine; compatibility depends on the application.
Networking uses DHCP when a supported interface is present. The "debug" boot entry opens a root shell on the
serial port; normal boots do not. This is not a production OS.
