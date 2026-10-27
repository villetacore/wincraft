use crate::storage;
use std::{
    fs,
    os::unix::fs::{chown, PermissionsExt},
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

const USER_UID: u32 = 1000;
const USER_HOME: &str = "/home/user";
const RUNTIME_DIR: &str = "/run/user/1000";
/// The unprivileged session requests power actions by creating files here.
const CONTROL_DIR: &str = "/run/wincraft/control";
const DATA_LABEL: &str = "WINCRAFT_DATA";
/// "persistent <device>" or "temporary <reason>": what the desktop tells the user (FIX-02).
const PERSISTENCE: &str = "/run/wincraft/persistence";
/// A service that dies sooner than this counts as a startup failure.
const QUICK_EXIT: Duration = Duration::from_secs(30);
const MAX_QUICK_FAILURES: u32 = 5;
const KNOWN_FOLDERS: [&str; 6] = [
    "Desktop",
    "Documents",
    "Downloads",
    "Music",
    "Pictures",
    "Videos",
];

fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .is_ok_and(|s| s.success())
}

fn is_mounted(target: &str) -> bool {
    fs::read_to_string("/proc/mounts").is_ok_and(|m| {
        m.lines()
            .any(|l| l.split_whitespace().nth(1) == Some(target))
    })
}

fn mount(fstype: &str, target: &str, options: &str) -> Result<(), String> {
    let _ = fs::create_dir_all(target);
    let already = if target == "/proc" {
        Path::new("/proc/self").exists()
    } else {
        is_mounted(target)
    };
    if already || run("mount", &["-t", fstype, "-o", options, fstype, target]) {
        Ok(())
    } else {
        Err(format!("cannot mount {fstype} on {target}"))
    }
}

/// Reaps exited children; returns their pids, or None when no children remain.
fn reap() -> Option<Vec<i32>> {
    let mut exited = vec![];
    loop {
        let mut status = 0;
        // SAFETY: waitpid only writes the provided status integer.
        match unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } {
            0 => return Some(exited),
            pid if pid > 0 => exited.push(pid),
            _ if exited.is_empty() => return None,
            _ => return Some(exited),
        }
    }
}

fn debug_mode() -> bool {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    cmdline.split_whitespace().any(|w| w == "wincraft.debug")
        || fs::read("/sys/firmware/qemu_fw_cfg/by_name/opt/wincraft/debug/raw")
            .is_ok_and(|v| v.first() == Some(&b'1'))
}

fn spawn_serial_shell() -> Option<i32> {
    Command::new("/bin/sh")
        .arg("-c")
        .arg("exec /bin/sh -i </dev/ttyS0 >/dev/ttyS0 2>&1")
        .env("HOME", "/root")
        .spawn()
        .ok()
        .map(|c| c.id() as i32)
}

/// PID 1 must never exit: report, keep reaping; debug boots get a shell.
fn fatal(reason: &str, debug: bool) -> ! {
    println!("WINCRAFT_INIT_FAILED: {reason}");
    if debug {
        spawn_serial_shell();
    } else {
        println!("Restart and choose the debug boot entry for a recovery shell.");
    }
    loop {
        let _ = reap();
        thread::sleep(Duration::from_secs(1));
    }
}

/// Puts /home and /apps on the data disk. Either both are there and verified in
/// /proc/mounts, or nothing is: a half-mounted disk would silently keep some files in RAM.
fn mount_data_disk() -> Result<String, String> {
    let device = storage::find_by_label(DATA_LABEL).ok_or("no data disk (label WINCRAFT_DATA)")?;
    let device = device.to_string_lossy().into_owned();
    if !run(
        "mount",
        &["-t", "ext4", "-o", "nosuid,nodev", &device, "/data"],
    ) || !is_mounted("/data")
    {
        return Err(format!("the data disk {device} could not be mounted"));
    }
    let mut bound: Vec<&str> = vec![];
    for (source, target) in [("/data/home", "/home"), ("/data/apps", "/apps")] {
        let _ = fs::create_dir_all(source);
        let _ = fs::create_dir_all(target);
        if !run("mount", &["--bind", source, target]) || !is_mounted(target) {
            for done in bound.iter().rev() {
                run("umount", &[done]);
            }
            run("umount", &["/data"]);
            return Err(format!(
                "{target} could not be placed on the data disk {device}"
            ));
        }
        bound.push(target);
    }
    Ok(device)
}

/// The first network interface (other than loopback), for the DHCP client.
fn network_interface() -> Option<String> {
    let mut names: Vec<String> = fs::read_dir("/sys/class/net")
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "lo")
        .collect();
    names.sort();
    names.into_iter().next()
}

fn owned_dir(path: &str, mode: u32) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("{path}: {e}"))?;
    chown(path, Some(USER_UID), Some(USER_UID)).map_err(|e| format!("{path}: {e}"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| format!("{path}: {e}"))
}

/// Profile and runtime directories; a profile created on a fresh data disk gets the
/// known folders, like the image's own /home/user.
fn prepare_user() -> Result<(), String> {
    owned_dir(USER_HOME, 0o755)?;
    for folder in KNOWN_FOLDERS {
        let path = format!("{USER_HOME}/{folder}");
        if !Path::new(&path).exists() {
            owned_dir(&path, 0o755)?;
        }
    }
    fs::create_dir_all("/run/wincraft").map_err(|e| e.to_string())?;
    owned_dir(RUNTIME_DIR, 0o700)?;
    owned_dir(CONTROL_DIR, 0o700)
}

/// Windows executables start from any shell: `./setup.exe` runs through Wine.
fn register_binfmt() {
    if mount(
        "binfmt_misc",
        "/proc/sys/fs/binfmt_misc",
        "nosuid,nodev,noexec",
    )
    .is_err()
    {
        return;
    }
    let wine = ["/usr/bin/wine", "/usr/lib/wine/wine"]
        .into_iter()
        .find(|p| Path::new(p).exists());
    if let Some(wine) = wine {
        if !Path::new("/proc/sys/fs/binfmt_misc/windows").exists() {
            let _ = fs::write(
                "/proc/sys/fs/binfmt_misc/register",
                format!(":windows:M::MZ::{wine}:"),
            );
        }
    }
}

fn set_hostname(name: &str) {
    // SAFETY: pointer and length describe a valid byte string.
    unsafe { libc::sethostname(name.as_ptr().cast(), name.len()) };
}

/// A restartable child process with crash backoff.
struct Service {
    name: &'static str,
    program: &'static str,
    args: Vec<String>,
    pid: Option<i32>,
    started: Instant,
    quick_failures: u32,
    restart_at: Option<Instant>,
}

impl Service {
    fn new(name: &'static str, program: &'static str, args: Vec<String>) -> Self {
        Self {
            name,
            program,
            args,
            pid: None,
            started: Instant::now(),
            quick_failures: 0,
            restart_at: Some(Instant::now()),
        }
    }

    fn tick(&mut self) {
        if self.restart_at.is_some_and(|t| Instant::now() >= t) {
            self.restart_at = None;
            self.started = Instant::now();
            match Command::new(self.program).args(&self.args).spawn() {
                Ok(child) => self.pid = Some(child.id() as i32),
                Err(e) => {
                    println!("WINCRAFT_WARN cannot start {}: {e}", self.name);
                    self.exited();
                }
            }
        }
    }

    fn exited(&mut self) {
        self.pid = None;
        if self.started.elapsed() < QUICK_EXIT {
            self.quick_failures += 1;
        } else {
            self.quick_failures = 0;
        }
        if self.quick_failures >= MAX_QUICK_FAILURES {
            println!(
                "WINCRAFT_SERVICE_FAILED {} after {} quick exits; not restarting",
                self.name, self.quick_failures
            );
            return;
        }
        let delay = Duration::from_secs(2u64 << self.quick_failures.min(4));
        println!(
            "Service {} exited; restarting in {}s",
            self.name,
            delay.as_secs()
        );
        self.restart_at = Some(Instant::now() + delay);
    }
}

fn shutdown(reboot: bool) -> ! {
    println!(
        "WINCRAFT_SHUTDOWN {}",
        if reboot { "reboot" } else { "poweroff" }
    );
    // SAFETY: plain signal delivery; pid -1 from PID 1 targets every other process.
    unsafe { libc::kill(-1, libc::SIGTERM) };
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && reap().is_some() {
        thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: as above.
    unsafe { libc::kill(-1, libc::SIGKILL) };
    thread::sleep(Duration::from_millis(300));
    let _ = reap();
    // SAFETY: sync has no preconditions.
    unsafe { libc::sync() };
    for target in ["/apps", "/home", "/data"] {
        if is_mounted(target) && !run("umount", &[target]) {
            run("mount", &["-o", "remount,ro", target]);
        }
    }
    // SAFETY: as above; reboot(2) does not return on success.
    unsafe {
        libc::sync();
        libc::reboot(if reboot {
            libc::RB_AUTOBOOT
        } else {
            libc::RB_POWER_OFF
        });
    }
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

pub fn main() {
    assert_eq!(
        std::process::id(),
        1,
        "wincraft-init must only run as guest PID 1"
    );
    // The kernel starts PID 1 without PATH; every child inherits this one.
    std::env::set_var(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );
    let critical = [
        ("proc", "/proc", "nosuid,nodev,noexec"),
        ("sysfs", "/sys", "nosuid,nodev,noexec"),
        ("devtmpfs", "/dev", "mode=0755,nosuid"),
        ("devpts", "/dev/pts", "gid=5,mode=620,ptmxmode=666"),
        ("tmpfs", "/dev/shm", "mode=1777,nosuid,nodev"),
        ("tmpfs", "/tmp", "mode=1777,nosuid,nodev"),
        ("tmpfs", "/run", "mode=0755,nosuid,nodev"),
    ];
    let mut failure = None;
    for (fstype, target, options) in critical {
        if let Err(e) = mount(fstype, target, options) {
            failure.get_or_insert(e);
        }
    }
    // WinCraft's own kernel has these built in; a modular (developer) kernel needs them.
    if Path::new("/sbin/modprobe").exists() {
        for module in [
            "qemu_fw_cfg",
            "bochs",
            "evdev",
            "usbhid",
            "hid_generic",
            "xhci_pci",
            "psmouse",
            "atkbd",
            "virtio_pci",
            "virtio_blk",
            "ext4",
            "binfmt_misc",
            "virtio_net",
            "e1000",
            "snd_hda_intel",
        ] {
            let _ = Command::new("/sbin/modprobe").arg(module).status();
        }
    }
    let debug = debug_mode();
    if let Some(reason) = failure {
        fatal(&reason, debug);
    }
    set_hostname("wincraft");
    for dir in ["/root", "/apps", "/home", "/data"] {
        let _ = fs::create_dir_all(dir);
    }
    let persistence = mount_data_disk();
    if let Err(e) = prepare_user() {
        fatal(&format!("user profile setup: {e}"), debug);
    }
    let state = match &persistence {
        Ok(device) => {
            println!("WINCRAFT_DATA_MOUNTED {device}");
            format!("persistent {device}\n")
        }
        Err(reason) => {
            println!("WINCRAFT_DATA_TEMPORARY {reason}");
            format!("temporary {reason}\n")
        }
    };
    let _ = fs::write(PERSISTENCE, state);
    register_binfmt();
    let mode = if debug { "debug" } else { "release" };
    println!("WINCRAFT_INIT_READY pid=1 mode={mode}");
    // libinput (in the compositor) needs the udev database to discover keyboard and mouse.
    run("udevd", &["--daemon"]);
    run("udevadm", &["trigger"]);
    run("udevadm", &["settle"]);

    // The serial root shell is a development aid and exists only in debug boots.
    let mut serial_shell = if debug { spawn_serial_shell() } else { None };
    // Network (DESK-00): DHCP on the first interface, restarted like any service.
    run("ip", &["link", "set", "lo", "up"]);
    let mut services = vec![Service::new("winlogon", "winlogon", vec![])];
    match network_interface() {
        Some(interface) => {
            println!("WINCRAFT_NETWORK dhcp on {interface}");
            run("ip", &["link", "set", &interface, "up"]);
            services.push(Service::new(
                "network",
                "udhcpc",
                [
                    "-f",
                    "-i",
                    &interface,
                    "-s",
                    "/usr/share/udhcpc/default.script",
                    "-t",
                    "5",
                    "-T",
                    "3",
                    "-A",
                    "10",
                ]
                .map(String::from)
                .to_vec(),
            ));
        }
        None => println!("WINCRAFT_NETWORK none"),
    }
    loop {
        if Path::new(CONTROL_DIR).join("poweroff").exists() {
            shutdown(false);
        }
        if Path::new(CONTROL_DIR).join("reboot").exists() {
            shutdown(true);
        }
        for pid in reap().unwrap_or_default() {
            if Some(pid) == serial_shell {
                serial_shell = spawn_serial_shell();
            }
            for service in services.iter_mut().filter(|s| s.pid == Some(pid)) {
                service.exited();
            }
        }
        for service in &mut services {
            service.tick();
        }
        thread::sleep(Duration::from_millis(100));
    }
}
