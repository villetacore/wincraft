use crate::user::Account;
use std::{
    fs,
    os::unix::fs::{chown, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use wc_runtime::prefix::{self, State};

const CONTROL_DIR: &str = "/run/wincraft/control";
const SEATD_SOCKET: &str = "/run/seatd.sock";
/// Written by `winlogon --report-session`, which the compositor runs once it is ready.
const SESSION_ENV: &str = "wincraft-session.env";

fn wait_for(
    what: &str,
    seconds: u64,
    child: &mut Child,
    ready: impl Fn() -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        if ready() {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("{what} exited during start: {status}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("{what} was not ready within {seconds} s"))
}

/// seatd (root) hands the display and input devices to the compositor, which then runs
/// as the user: unlike Xorg before it, no part of the graphics stack keeps root rights.
fn start_seatd(account: &Account) -> Result<Child, String> {
    let _ = fs::remove_file(SEATD_SOCKET);
    let mut seatd = Command::new("seatd")
        .args(["-u", &account.name, "-g", &account.name])
        .spawn()
        .map_err(|e| format!("cannot start seatd: {e}"))?;
    wait_for("seatd", 10, &mut seatd, || Path::new(SEATD_SOCKET).exists())?;
    Ok(seatd)
}

/// Starts labwc (Wayland compositor and window manager, XWayland for X11 and Windows
/// programs) as the user and waits until it reports its sockets.
/// In debug boots the compositor logs verbosely to `labwc.log` in the runtime directory.
fn start_compositor(
    account: &Account,
    runtime_dir: &str,
    debug: bool,
) -> Result<(Child, Vec<(String, String)>), String> {
    let report = Path::new(runtime_dir).join(SESSION_ENV);
    let _ = fs::remove_file(&report);
    // XWayland's socket directory must exist and be sticky before the user's compositor
    // creates its socket there.
    let _ = fs::create_dir_all("/tmp/.X11-unix");
    let _ = fs::set_permissions("/tmp/.X11-unix", fs::Permissions::from_mode(0o1777));
    let mut command = Command::new("labwc");
    command.args(["--startup", "/usr/bin/winlogon --report-session"]);
    if debug {
        command.arg("--debug");
        let log = Path::new(runtime_dir).join("labwc.log");
        if let Ok(file) = fs::File::create(&log) {
            let _ = chown(&log, Some(account.uid), Some(account.gid));
            command.stderr(file);
        }
    }
    account.apply(&mut command, &account.compositor_environment(runtime_dir));
    let mut labwc = command
        .spawn()
        .map_err(|e| format!("cannot start labwc: {e}"))?;
    wait_for("labwc", 60, &mut labwc, || report.exists())?;
    let reported = fs::read_to_string(&report).map_err(|e| format!("{}: {e}", report.display()))?;
    let sockets: Vec<(String, String)> = reported
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(k, v)| matches!(*k, "WAYLAND_DISPLAY" | "DISPLAY") && !v.is_empty())
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    if !sockets.iter().any(|(k, _)| k == "WAYLAND_DISPLAY") {
        let _ = labwc.kill();
        return Err("labwc reported no WAYLAND_DISPLAY".into());
    }
    Ok((labwc, sockets))
}

/// `winlogon --report-session`: runs as the user inside the compositor's environment and
/// hands its socket names to the session manager (atomically: write, then rename).
pub fn report() -> i32 {
    let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return 1;
    };
    let mut text = String::new();
    for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
        if let Ok(value) = std::env::var(key) {
            text.push_str(&format!("{key}={value}\n"));
        }
    }
    let target = Path::new(&dir).join(SESSION_ENV);
    let partial = Path::new(&dir).join(format!("{SESSION_ENV}.tmp"));
    match fs::write(&partial, text).and_then(|_| fs::rename(&partial, &target)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("winlogon --report-session: {e}");
            1
        }
    }
}

fn spawn_as(
    account: &Account,
    env: &[(String, String)],
    program: &str,
    args: &[&str],
) -> Option<Child> {
    let mut command = Command::new(program);
    command.args(args);
    account.apply(&mut command, env);
    match command.spawn() {
        Ok(child) => Some(child),
        Err(e) => {
            println!("WINCRAFT_WARN {program}: {e}");
            None
        }
    }
}

/// Runs a session-side tool to completion, killing it after `limit` (a hung `wineboot`
/// must not leave the prefix "preparing" forever).
fn run_as(
    account: &Account,
    env: &[(String, String)],
    program: &str,
    args: &[&str],
    limit: Duration,
) -> Result<(), String> {
    let mut child = spawn_as(account, env, program, args)
        .ok_or_else(|| format!("{program} could not be started"))?;
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!("{program} {} failed ({status})", args.join(" ")))
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{program} {} did not finish within {} s",
                    args.join(" "),
                    limit.as_secs()
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(format!("{program}: {e}")),
        }
    }
}

/// The Wine build in the image, as a runtime id ("wine-11.0").
fn installed_runtime(account: &Account, env: &[(String, String)]) -> Result<String, String> {
    let mut command = Command::new("wine");
    command.arg("--version");
    account.apply(&mut command, env);
    let out = command
        .output()
        .map_err(|e| format!("Wine is not available: {e}"))?;
    wc_runtime::runtime_id(&String::from_utf8_lossy(&out.stdout))
        .ok_or_else(|| "Wine did not report its version".into())
}

/// Creates or updates the prefix for `runtime`. A prefix damaged by a power cut is moved
/// aside, never deleted; a prefix of another Wine build is updated (`wineboot --update`).
fn prepare_prefix(
    account: &Account,
    env: &[(String, String)],
    runtime: &str,
) -> Result<(), String> {
    let prefix = account.home.join(".wine");
    if prefix.exists() && !prefix::is_valid(&prefix) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let broken = account.home.join(format!(".wine.broken-{stamp}"));
        match fs::rename(&prefix, &broken) {
            Ok(()) => println!(
                "WINCRAFT_WINE_PREFIX_RESET moved damaged prefix to {}",
                broken.display()
            ),
            Err(e) => return Err(format!("the damaged prefix cannot be moved aside: {e}")),
        }
    }
    let action = if !prefix::is_valid(&prefix) {
        "--init"
    } else if wc_runtime::prefix_runtime(&prefix).as_deref() != Some(runtime) {
        "--update"
    } else {
        return Ok(());
    };
    // Headless: without a display, wineboot creates no windows (its setup windows would
    // otherwise sit invisibly over the desktop and swallow clicks).
    let mut boot_env: Vec<(String, String)> = env
        .iter()
        .filter(|(k, _)| k != "DISPLAY" && k != "WAYLAND_DISPLAY")
        .cloned()
        .collect();
    // Skip the optional Mono/Gecko download prompts during preparation only.
    boot_env.retain(|(k, _)| k != "WINEDLLOVERRIDES");
    boot_env.push((
        "WINEDLLOVERRIDES".into(),
        "mscoree,mshtml=;winemenubuilder.exe=d".into(),
    ));
    println!("WINCRAFT_WINE_PREPARING wineboot {action} runtime={runtime}");
    run_as(
        account,
        &boot_env,
        "wineboot",
        &[action],
        Duration::from_secs(300),
    )?;
    run_as(
        account,
        &boot_env,
        "wineserver",
        &["-w"],
        Duration::from_secs(60),
    )?;
    if !prefix::is_valid(&prefix) {
        return Err("the prefix is incomplete after wineboot (no valid registry)".into());
    }
    wc_runtime::set_prefix_runtime(&prefix, runtime)
        .map_err(|e| format!("cannot record the runtime: {e}"))?;
    let _ = chown(
        prefix.join(".wincraft-runtime"),
        Some(account.uid),
        Some(account.gid),
    );
    Ok(())
}

/// Prepares the Win32 environment in the background (FIX-01). Its state — preparing,
/// ready with the runtime, or failed with a reason — is what launchers wait for.
fn prepare_wine(account: Account, env: Vec<(String, String)>, state_path: PathBuf) {
    let publish = move |state: &State| {
        if let Err(e) = prefix::write(&state_path, state) {
            println!("WINCRAFT_WARN cannot write {}: {e}", state_path.display());
        }
    };
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    publish(&State::Preparing { since });
    thread::spawn(move || {
        let result = installed_runtime(&account, &env)
            .and_then(|runtime| prepare_prefix(&account, &env, &runtime).map(|_| runtime));
        match result {
            Ok(runtime) => {
                publish(&State::Ready {
                    runtime: runtime.clone(),
                });
                println!("WINCRAFT_WINE_READY runtime={runtime}");
            }
            Err(reason) => {
                println!("WINCRAFT_WINE_FAILED {reason}");
                publish(&State::Failed { reason });
            }
        }
    });
}

/// Ends every process of the session account (after the shell or compositor is gone).
fn end_user_processes(uid: u32) {
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
                continue;
            };
            let owner = fs::read_to_string(entry.path().join("status"))
                .ok()
                .and_then(|s| {
                    s.lines().find(|l| l.starts_with("Uid:")).and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u32>().ok())
                    })
                });
            if owner == Some(uid) {
                // SAFETY: plain signal delivery to a process of the session account.
                unsafe { libc::kill(pid, signal) };
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}

/// A user-side process restarted when it exits (the shell), like explorer.exe.
struct Restartable {
    program: &'static str,
    args: &'static [&'static str],
    child: Option<Child>,
    failures: u32,
    started: Instant,
}

impl Restartable {
    fn new(program: &'static str, args: &'static [&'static str]) -> Self {
        Self {
            program,
            args,
            child: None,
            failures: 0,
            started: Instant::now(),
        }
    }

    fn keep_alive(&mut self, account: &Account, env: &[(String, String)]) {
        let exited = match &mut self.child {
            Some(child) => child.try_wait().ok().flatten().is_some(),
            None => true,
        };
        if !exited {
            return;
        }
        if self.child.is_some() {
            self.failures = if self.started.elapsed() < Duration::from_secs(20) {
                self.failures + 1
            } else {
                0
            };
            println!("Session process {} exited; restarting", self.program);
            // Back off on repeated crashes, but never leave the user without a shell.
            thread::sleep(Duration::from_secs(1u64 << self.failures.min(4)));
        }
        self.started = Instant::now();
        self.child = spawn_as(account, env, self.program, self.args);
    }
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run() -> i32 {
    let account = match Account::lookup("user") {
        Ok(a) => a,
        Err(e) => {
            println!("WINCRAFT_WINLOGON_FAILED: {e}");
            return 1;
        }
    };
    let runtime_dir = format!("/run/user/{}", account.uid);
    let wine_state = prefix::state_file(Path::new(&runtime_dir));
    let _ = fs::remove_file(&wine_state);
    for control in ["logoff"] {
        let _ = fs::remove_file(Path::new(CONTROL_DIR).join(control));
    }

    let mut seatd = match start_seatd(&account) {
        Ok(s) => s,
        Err(e) => {
            println!("WINCRAFT_WINLOGON_FAILED: {e}");
            return 1;
        }
    };
    let debug = fs::read_to_string("/proc/cmdline")
        .is_ok_and(|c| c.split_whitespace().any(|w| w == "wincraft.debug"))
        || fs::read("/sys/firmware/qemu_fw_cfg/by_name/opt/wincraft/debug/raw")
            .is_ok_and(|v| v.first() == Some(&b'1'));
    let (mut compositor, sockets) = match start_compositor(&account, &runtime_dir, debug) {
        Ok(c) => c,
        Err(e) => {
            println!("WINCRAFT_WINLOGON_FAILED: {e}");
            stop(&mut seatd);
            return 1;
        }
    };
    let mut env = account.environment(&runtime_dir, &sockets);
    if debug {
        env.push(("WINCRAFT_DEBUG".into(), "1".into()));
    }
    let sockets_text: Vec<String> = sockets.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!("WINCRAFT_COMPOSITOR_READY {}", sockets_text.join(" "));

    prepare_wine(account.clone(), env.clone(), wine_state);
    let mut shell = Restartable::new("explorer", &[]);
    shell.keep_alive(&account, &env);
    println!(
        "WINCRAFT_SESSION_STARTED user={} uid={}",
        account.name, account.uid
    );

    let code = loop {
        if let Ok(Some(status)) = compositor.try_wait() {
            println!("Compositor exited ({status}); ending session");
            break 1;
        }
        if let Ok(Some(status)) = seatd.try_wait() {
            println!("seatd exited ({status}); ending session");
            break 1;
        }
        if Path::new(CONTROL_DIR).join("logoff").exists() {
            println!("WINCRAFT_LOGOFF");
            break 0;
        }
        shell.keep_alive(&account, &env);
        thread::sleep(Duration::from_millis(250));
    };
    stop(&mut compositor);
    end_user_processes(account.uid);
    stop(&mut seatd);
    code
}
