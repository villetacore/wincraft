//! start: the WinCraft launcher (APP-01), the process side of ShellExecute.
//!
//! ```text
//! start [--] <file> [args...]    open a file: Windows programs through Wine, documents
//!                                with their application (the same policy as Explorer)
//! start --wine <args...>         a Wine tool: regedit, winecfg, cmd, ...
//! ```
//!
//! Before anything Win32 runs, it waits (bounded) until the Wine prefix is ready and reports
//! a failed preparation instead of hanging (FIX-01). Every launch is journaled with argv,
//! working directory, runtime, PID and exit status (`~/.local/state/wincraft/launches.log`).
//! The launcher stays the parent of the program until it exits, so the result is recorded.
#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    launcher::run(std::env::args_os().skip(1).collect())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("start runs only inside the WinCraft guest");
}

#[cfg(target_os = "linux")]
mod launcher {
    use std::{
        ffi::OsString,
        os::unix::process::{CommandExt, ExitStatusExt},
        path::{Path, PathBuf},
        process::{Command, ExitCode},
        time::Duration,
    };
    use wc_launch::Plan;
    use wc_runtime::{
        journal::{self, Event, Record},
        prefix,
    };

    /// First-boot preparation of the prefix takes about a minute without hardware acceleration.
    const READY_TIMEOUT: Duration = Duration::from_secs(240);

    fn interactive() -> bool {
        // SAFETY: isatty only inspects the descriptor.
        unsafe { libc::isatty(0) == 1 }
    }

    /// Errors go to the terminal when there is one, otherwise to a message box.
    fn report(text: &str) {
        if interactive() {
            eprintln!("start: {text}");
        } else if Command::new("explorer")
            .arg("--message")
            .arg(text)
            .spawn()
            .is_err()
        {
            eprintln!("start: {text}");
        }
    }

    fn env_path(name: &str) -> Option<PathBuf> {
        std::env::var_os(name).map(PathBuf::from)
    }

    fn record_for(command: &Command) -> Record {
        Record {
            program: command.get_program().to_string_lossy().into_owned(),
            args: command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect(),
            cwd: command
                .get_current_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    pub fn run(args: Vec<OsString>) -> ExitCode {
        let journal = env_path("HOME").map(|h| journal::path(&h));
        let log = |event: Event, record: &Record| {
            if let Some(journal) = &journal {
                let _ = journal::append(journal, event, record);
            }
        };
        let (mut command, win32) = match parse(&args) {
            Ok(c) => c,
            Err(message) => {
                log(
                    Event::Error,
                    &Record {
                        program: args
                            .first()
                            .map(|a| a.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        detail: message.clone(),
                        ..Default::default()
                    },
                );
                report(&message);
                return ExitCode::FAILURE;
            }
        };
        let mut record = record_for(&command);
        if win32 {
            let state = env_path("XDG_RUNTIME_DIR").map(|d| prefix::state_file(&d));
            match state.map(|s| prefix::wait_ready(&s, READY_TIMEOUT, Duration::from_millis(250))) {
                Some(Ok(runtime)) => record.runtime = runtime,
                Some(Err(e)) => {
                    record.detail = e.to_string();
                    log(Event::Error, &record);
                    report(&e.to_string());
                    return ExitCode::FAILURE;
                }
                None => {}
            }
        }
        // Outside a terminal the program gets its own process group: the job of this launch
        // (a terminal keeps its foreground group, or reading stdin would stop the program).
        if !interactive() {
            command.process_group(0);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                record.detail = e.to_string();
                log(Event::Error, &record);
                report(&format!("Cannot start {}: {e}", record.program));
                return ExitCode::FAILURE;
            }
        };
        record.pid = Some(child.id());
        log(Event::Start, &record);
        let status = child.wait();
        record.status = status
            .as_ref()
            .ok()
            .map(|s| s.code().unwrap_or_else(|| -s.signal().unwrap_or(0)));
        if let Err(e) = &status {
            record.detail = e.to_string();
        }
        log(Event::Exit, &record);
        match record.status {
            Some(0) => ExitCode::SUCCESS,
            Some(code) => ExitCode::from(code.clamp(1, 255) as u8),
            None => ExitCode::FAILURE,
        }
    }

    /// The command to run and whether it needs the Win32 environment.
    fn parse(args: &[OsString]) -> Result<(Command, bool), String> {
        let first = args
            .first()
            .ok_or("usage: start [--] <file> [args...] | start --wine <args...>")?;
        if first == "--wine" {
            let mut command = Command::new("wine");
            command.args(&args[1..]);
            return Ok((command, true));
        }
        let rest = if first == "--" { &args[1..] } else { args };
        let target = rest.first().ok_or("start: no file given")?;
        let path = Path::new(target);
        let plan = match wc_launch::plan(path) {
            Plan::Windows { program, .. } => Plan::Windows {
                program,
                args: rest[1..].to_vec(),
            },
            Plan::Native { program, .. } => Plan::Native {
                program,
                args: rest[1..].to_vec(),
            },
            Plan::Folder(dir) => {
                let mut command = Command::new("explorer");
                command.arg("--folder").arg(dir);
                return Ok((command, false));
            }
            Plan::Unsupported(message) => return Err(message),
            plan => plan,
        };
        let win32 = wc_launch::needs_win32(&plan);
        wc_launch::command(&plan)
            .map(|c| (c, win32))
            .map_err(|e| e.to_string())
    }
}
