# Security policy

WinCraft 0.4 is an experimental developer preview. Only the current development line receives fixes; there is no support window, production hardening promise, or automatic update mechanism.

Run previews in disposable virtual machines. Do not use them for sensitive workloads. Treat Windows executables as untrusted software; Wine is not a security boundary. The debug boot entry deliberately provides a privileged serial shell.

## Reporting a vulnerability

Use the repository's **Security → Report a vulnerability** option if private vulnerability reporting is enabled. If it is unavailable, open an issue requesting a private reporting channel without including exploit steps or sensitive data. Do not post secrets, personal files, or a working exploit in a public issue.

Include the affected revision/release, VM configuration, expected behavior, impact, and a minimal reproduction when a private channel is available. Reports and fixes are handled on a best-effort basis.
