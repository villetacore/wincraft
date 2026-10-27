#!/usr/bin/env python3
"""Boot the actual ISO in a disposable, network-isolated BIOS/UEFI QEMU VM."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("iso", type=pathlib.Path)
parser.add_argument("--uefi", action="store_true")
parser.add_argument("--timeout", type=int, default=900)
args = parser.parse_args()
iso = args.iso.resolve(strict=True)
log = pathlib.Path("dist/smoke-uefi.log" if args.uefi else "dist/smoke-bios.log").resolve()
log.parent.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="wincraft-smoke-") as tmp:
    command = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "Nehalem", "-m", "4096",
               "-smp", "2", "-vga", "std", "-display", "none", "-monitor", "none",
               "-serial", f"file:{log}", "-nic", "none", "-no-reboot",
               "-boot", "d", "-cdrom", str(iso)]
    if args.uefi:
        firmware = pathlib.Path("/usr/share/OVMF")
        variables = pathlib.Path(tmp) / "OVMF_VARS.fd"
        shutil.copyfile(firmware / "OVMF_VARS_4M.fd", variables)
        command += ["-drive", f"if=pflash,format=raw,readonly=on,file={firmware}/OVMF_CODE_4M.fd",
                    "-drive", f"if=pflash,format=raw,file={variables}"]
    log.write_text("")
    with log.with_suffix(".qemu.log").open("w") as errors:
        vm = subprocess.Popen(command, stdout=errors, stderr=errors)
        try:
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                output = log.read_text(errors="replace")
                if any(marker in output for marker in ["WINCRAFT_INIT_FAILED", "WINCRAFT_WINLOGON_FAILED", "Kernel panic", "WINCRAFT_BOOT_FAILED"]):
                    raise SystemExit(f"Guest boot failed; inspect {log.name}")
                if "WINCRAFT_INIT_READY pid=1 mode=release" in output and "WINCRAFT_DESKTOP_READY" in output:
                    if "debug=1" in output or "mode=debug" in output:
                        raise SystemExit("Unexpected debug mode in default boot")
                    print(f"PASS: {'UEFI' if args.uefi else 'BIOS'} ISO boot reached release desktop")
                    break
                if vm.poll() is not None:
                    raise SystemExit(f"QEMU exited before desktop; inspect {log.name}")
                time.sleep(2)
            else:
                raise SystemExit(f"Desktop boot timed out; inspect {log.name}")
        finally:
            if vm.poll() is None:
                vm.terminate()
                try:
                    vm.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    vm.kill()
                    vm.wait()
