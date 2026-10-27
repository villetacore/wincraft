#!/usr/bin/env python3
"""Check only publishable files; build trees and local settings stay private."""
import pathlib
import re
import subprocess
import sys

files = subprocess.check_output(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]
).decode().split("\0")
patterns = {
    "personal email": re.compile(r"\b[A-Za-z0-9._%+-]+@(?!users\.noreply\.github\.com\b|example\.(?:com|org)\b)[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b", re.I),
    "personal host path": re.compile(r"(?:/Users/|/home/)(?!user(?:[/\s\"']|$)|root(?:[/\s\"']|$))[A-Za-z0-9_.-]+/|[C-Z]:\\Users\\(?!<?user>?\\)[^\\\s]+\\", re.I),
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "access token": re.compile(r"(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[A-Z0-9]{16})"),
}
failed = False
for name in sorted(set(filter(None, files))):
    p = pathlib.Path(name)
    if not p.is_file():
        continue
    if p.stat().st_size > 10 * 1024 * 1024:
        print(f"{name}: unexpected large source file")
        failed = True
        continue
    try:
        data = p.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        print(f"{name}: unexpected binary file in source repository")
        failed = True
        continue
    for label, pattern in patterns.items():
        if pattern.search(data):
            print(f"{name}: {label} detected (value hidden)")
            failed = True
if failed:
    sys.exit(1)
print("Repository privacy and source-size checks passed.")
