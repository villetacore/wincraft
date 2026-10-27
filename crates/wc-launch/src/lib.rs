//! ShellExecute: what happens when the user opens something.
//!
//! [`plan`] is pure policy (testable on any host): it inspects the file and returns a
//! [`Plan`]; formats the system cannot run get a message instead of a failing process
//! (APP-02). [`command`] turns a plan into a process description, [`spawn`] starts it
//! (Windows programs through the `start` launcher). Arguments always travel as argv;
//! nothing is ever interpolated into a shell command line.
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{Child, Command},
};
use wc_formats::{Arch, Format};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Open a folder in File Explorer.
    Folder(PathBuf),
    /// Windows program through the Win32 subsystem (Wine).
    Windows {
        program: PathBuf,
        args: Vec<OsString>,
    },
    /// Windows Installer package: `msiexec /i`.
    Installer(PathBuf),
    /// Windows batch file: `cmd /c`.
    Batch(PathBuf),
    /// Native Linux program or executable script.
    Native {
        program: PathBuf,
        args: Vec<OsString>,
    },
    /// Document opened by an associated WinCraft application.
    Associated {
        app: &'static str,
        document: PathBuf,
    },
    /// Nothing can open it; the message is shown to the user.
    Unsupported(String),
}

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "log", "ini", "inf", "cfg", "conf", "md", "csv", "json", "xml", "toml", "yaml", "yml",
    "reg", "nfo", "rs", "c", "h", "py", "sh",
];

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// What the installed runtimes can run; [`Runtimes::detect`] looks at the system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Runtimes {
    /// Wine Mono is installed: .NET Framework programs can start.
    pub dotnet: bool,
}

impl Runtimes {
    pub fn detect() -> Self {
        // The places Wine itself looks for Mono: system-wide, or inside the prefix.
        let mut mono = vec![
            PathBuf::from("/usr/share/wine/mono"),
            PathBuf::from("/opt/wine/mono"),
        ];
        if let Some(prefix) = std::env::var_os("WINEPREFIX") {
            mono.push(Path::new(&prefix).join("drive_c/windows/mono"));
        }
        Runtimes {
            dotnet: mono.iter().any(|d| d.is_dir()),
        }
    }
}

const DOS_MESSAGE: &str = "This is a DOS program. WinCraft runs DOS programs in a separate DOS \
    environment, which is not part of this version yet.";

/// Decides how to open `path`, given its metadata, first bytes and the installed runtimes.
pub fn plan_for(
    path: &Path,
    is_dir: bool,
    executable: bool,
    header: &[u8],
    runtimes: Runtimes,
) -> Plan {
    if is_dir {
        return Plan::Folder(path.to_path_buf());
    }
    let ext = extension(path);
    match ext.as_str() {
        "msi" => return Plan::Installer(path.to_path_buf()),
        "bat" | "cmd" => return Plan::Batch(path.to_path_buf()),
        "lnk" => {
            return Plan::Windows {
                program: path.to_path_buf(),
                args: vec![],
            };
        }
        "html" | "htm" | "xhtml" | "svg" | "pdf" => {
            return Plan::Associated {
                app: BROWSER,
                document: path.to_path_buf(),
            };
        }
        // Like Windows, a zip opens as its files: Explorer extracts it next to itself.
        "zip" => {
            return Plan::Native {
                program: "explorer".into(),
                args: vec!["--extract".into(), path.as_os_str().to_owned()],
            };
        }
        // Internet shortcuts: `[InternetShortcut]` with `URL=`.
        "url" => {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            return match text.lines().find_map(|l| l.trim().strip_prefix("URL=")) {
                Some(url) if url.starts_with("http://") || url.starts_with("https://") => {
                    Plan::Native {
                        program: BROWSER.into(),
                        args: vec![url.into()],
                    }
                }
                _ => Plan::Unsupported("This Internet shortcut has no web address.".into()),
            };
        }
        // COM programs have no header at all.
        "com" => return Plan::Unsupported(DOS_MESSAGE.into()),
        _ => {}
    }
    match wc_formats::classify(header) {
        Format::Pe { dll: true, .. } => {
            Plan::Unsupported("This is a library (DLL) and cannot be started on its own.".into())
        }
        Format::Pe { arch: Arch::X86 | Arch::X64, dotnet, .. } => {
            if dotnet && !runtimes.dotnet {
                Plan::Unsupported(
                    "This is a .NET Framework program. The .NET runtime for Windows programs \
                     (Wine Mono) is not installed in this version yet."
                        .into(),
                )
            } else {
                Plan::Windows { program: path.to_path_buf(), args: vec![] }
            }
        }
        Format::Pe { arch: Arch::Arm64, .. } => Plan::Unsupported(
            "This Windows program is built for ARM64 processors. WinCraft runs Windows programs \
             for x64 and x86 (32-bit) processors."
                .into(),
        ),
        Format::Pe { arch: Arch::Other(machine), .. } => Plan::Unsupported(format!(
            "This Windows program is built for an unsupported processor (machine type {machine:#06x})."
        )),
        Format::Ne => Plan::Unsupported(
            "This is a 16-bit Windows program (Windows 3.x). The Win32 subsystem of WinCraft runs \
             32-bit and 64-bit Windows programs only."
                .into(),
        ),
        Format::Dos => Plan::Unsupported(DOS_MESSAGE.into()),
        Format::Elf { .. } | Format::Script { .. } if executable => {
            Plan::Native { program: path.to_path_buf(), args: vec![] }
        }
        Format::Elf { .. } => Plan::Unsupported(
            "This Linux program is not marked as executable.".into(),
        ),
        _ if TEXT_EXTENSIONS.contains(&ext.as_str()) || looks_like_text(header) => {
            Plan::Associated { app: "notepad", document: path.to_path_buf() }
        }
        _ => Plan::Unsupported(format!(
            "No application is associated with \"{}\" files yet.",
            if ext.is_empty() { "this type of" } else { &ext }
        )),
    }
}

fn looks_like_text(header: &[u8]) -> bool {
    !header.is_empty()
        && std::str::from_utf8(header).is_ok_and(|s| {
            s.chars()
                .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        })
}

/// Reads what [`plan_for`] needs from the file system.
pub fn plan(path: &Path) -> Plan {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Plan::Unsupported(format!("{}: {e}", path.display())),
    };
    let executable = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            false
        }
    };
    let mut header = Vec::new();
    if meta.is_file() {
        use std::io::Read;
        if let Ok(f) = std::fs::File::open(path) {
            let _ = f
                .take(wc_formats::HEADER_LEN as u64)
                .read_to_end(&mut header);
        }
    }
    plan_for(path, meta.is_dir(), executable, &header, Runtimes::detect())
}

/// WinCraft Browser (`apps/browser`), for web pages and web addresses.
pub const BROWSER: &str = "browser";

/// The terminal emulator (a native Wayland client); the program to run follows its options.
pub const TERMINAL: &str = "foot";

/// The launcher (`apps/start`): waits for the Win32 environment, journals the launch and
/// follows the process. Windows programs are always started through it.
pub const LAUNCHER: &str = "start";

/// Plans that need the Win32 environment (Wine prefix) to be ready.
pub fn needs_win32(plan: &Plan) -> bool {
    matches!(
        plan,
        Plan::Windows { .. } | Plan::Installer(_) | Plan::Batch(_)
    )
}

/// The program, its argv and working directory for a plan, without starting it.
/// `Folder` and `Unsupported` are handled by the caller (the shell).
pub fn command(plan: &Plan) -> io::Result<Command> {
    let (mut command, cwd) = match plan {
        Plan::Windows { program, args } => {
            let mut c = Command::new("wine");
            if extension(program) == "lnk" {
                c.args(["start", "/unix"]);
            }
            c.arg(program).args(args);
            (c, program.parent().map(Path::to_path_buf))
        }
        Plan::Installer(msi) => {
            let mut c = Command::new("wine");
            c.args(["msiexec", "/i"]).arg(msi);
            (c, msi.parent().map(Path::to_path_buf))
        }
        Plan::Batch(bat) => {
            let mut c = Command::new(TERMINAL);
            c.args(["--title", "Command Prompt", "wine", "cmd", "/k"])
                .arg(bat);
            (c, bat.parent().map(Path::to_path_buf))
        }
        Plan::Native { program, args } => {
            let mut c = Command::new(program);
            c.args(args);
            (c, program.parent().map(Path::to_path_buf))
        }
        Plan::Associated { app, document } => {
            let mut c = Command::new(app);
            c.arg(document);
            (c, document.parent().map(Path::to_path_buf))
        }
        Plan::Folder(_) | Plan::Unsupported(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a process plan",
            ));
        }
    };
    if let Some(dir) = cwd.filter(|d| d.is_dir()) {
        command.current_dir(dir);
    }
    Ok(command)
}

/// Starts a plan: Win32 plans through the [`LAUNCHER`], everything else directly.
pub fn spawn(plan: &Plan) -> io::Result<Child> {
    let target = match plan {
        Plan::Windows { program, args } => Some((program, args.as_slice())),
        Plan::Installer(path) | Plan::Batch(path) => Some((path, &[][..])),
        _ => None,
    };
    match target {
        Some((path, args)) => Command::new(LAUNCHER)
            .arg("--")
            .arg(path)
            .args(args)
            .spawn(),
        None => command(plan)?.spawn(),
    }
}

/// Built-in names accepted by the Run dialog, like `notepad` or `cmd` on Windows.
/// Windows tools go through the launcher (`start --wine <tool>`), which waits for the prefix.
pub fn run_command(name: &str) -> Option<Command> {
    let typed = name.trim();
    let lower = typed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.") {
        let mut c = Command::new(BROWSER);
        c.arg(typed);
        return Some(c);
    }
    let (program, args): (&str, &[&str]) = match lower.as_str() {
        "browser" | "msedge" | "msedge.exe" | "iexplore" | "iexplore.exe" | "edge" => {
            (BROWSER, &[])
        }
        "notepad" | "notepad.exe" => ("notepad", &[]),
        "taskmgr" | "taskmgr.exe" => ("taskmgr", &[]),
        "explorer" | "explorer.exe" => ("explorer", &["shell:ThisPC"]),
        "cmd" | "cmd.exe" => (
            TERMINAL,
            &["--title", "Command Prompt", LAUNCHER, "--wine", "cmd"],
        ),
        "terminal" | "bash" | "xterm" | "foot" | "sh" => (TERMINAL, &["--title", "Terminal"]),
        "regedit" | "regedit.exe" => (LAUNCHER, &["--wine", "regedit"]),
        "winecfg" | "control" => (LAUNCHER, &["--wine", "winecfg"]),
        "winver" => (LAUNCHER, &["--wine", "winver"]),
        _ => return None,
    };
    let mut c = Command::new(program);
    c.args(args);
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Runtimes = Runtimes { dotnet: false };

    fn pe_header() -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        b[0x94..0x96].copy_from_slice(&240u16.to_le_bytes());
        b[0x98..0x9A].copy_from_slice(&0x20Bu16.to_le_bytes());
        b
    }

    #[test]
    fn windows_programs_go_to_wine_by_content() {
        let p = Path::new("/apps/setup.EXE");
        assert!(matches!(
            plan_for(p, false, false, &pe_header(), NONE),
            Plan::Windows { .. }
        ));
        // Content wins over the extension: a renamed PE still runs as a program.
        assert!(matches!(
            plan_for(Path::new("/x/game.bin"), false, false, &pe_header(), NONE),
            Plan::Windows { .. }
        ));
    }

    #[test]
    fn documents_and_special_types() {
        assert_eq!(
            plan_for(Path::new("/d/readme.TXT"), false, false, b"hello", NONE),
            Plan::Associated {
                app: "notepad",
                document: "/d/readme.TXT".into()
            }
        );
        assert_eq!(
            plan_for(Path::new("/d/a.msi"), false, false, b"", NONE),
            Plan::Installer("/d/a.msi".into())
        );
        assert_eq!(
            plan_for(Path::new("/d/run.bat"), false, false, b"@echo", NONE),
            Plan::Batch("/d/run.bat".into())
        );
        assert_eq!(
            plan_for(Path::new("/d"), true, true, b"", NONE),
            Plan::Folder("/d".into())
        );
        assert!(matches!(
            plan_for(Path::new("/d/x.bin"), false, false, &[0, 1, 2, 0xFF], NONE),
            Plan::Unsupported(_)
        ));
    }

    #[test]
    fn linux_programs_need_the_executable_bit() {
        let elf = b"\x7fELF\x02\x01\x01";
        assert!(matches!(
            plan_for(Path::new("/usr/bin/htop"), false, true, elf, NONE),
            Plan::Native { .. }
        ));
        assert!(matches!(
            plan_for(Path::new("/tmp/htop"), false, false, elf, NONE),
            Plan::Unsupported(_)
        ));
    }

    fn with_machine(machine: u16) -> Vec<u8> {
        let mut b = pe_header();
        b[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        b
    }

    #[test]
    fn unsupported_windows_formats_explain_themselves() {
        let msg = |p: Plan| match p {
            Plan::Unsupported(m) => m,
            other => panic!("expected a message, got {other:?}"),
        };
        assert!(msg(plan_for(
            Path::new("/a/arm.exe"),
            false,
            false,
            &with_machine(0xAA64),
            NONE
        ))
        .contains("ARM64"));
        assert!(msg(plan_for(
            Path::new("/a/ia64.exe"),
            false,
            false,
            &with_machine(0x200),
            NONE
        ))
        .contains("0x0200"));
        let mut ne = vec![0u8; 0x100];
        ne[..2].copy_from_slice(b"MZ");
        ne[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        ne[0x80..0x82].copy_from_slice(b"NE");
        assert!(msg(plan_for(Path::new("/a/old.exe"), false, false, &ne, NONE)).contains("16-bit"));
        assert!(msg(plan_for(
            Path::new("/a/game.exe"),
            false,
            false,
            b"MZ\x90\0",
            NONE
        ))
        .contains("DOS"));
        assert!(msg(plan_for(
            Path::new("/a/edit.COM"),
            false,
            false,
            &[0xB4, 0x09],
            NONE
        ))
        .contains("DOS"));
    }

    #[test]
    fn dotnet_programs_need_mono() {
        let mut b = pe_header();
        // CLR runtime header: data directory 14 of a PE32+ optional header.
        let clr = 0x98 + 112 + 14 * 8;
        b[clr..clr + 4].copy_from_slice(&0x2000u32.to_le_bytes());
        assert!(
            matches!(plan_for(Path::new("/a/tool.exe"), false, false, &b, NONE), Plan::Unsupported(m) if m.contains(".NET"))
        );
        assert!(matches!(
            plan_for(
                Path::new("/a/tool.exe"),
                false,
                false,
                &b,
                Runtimes { dotnet: true }
            ),
            Plan::Windows { .. }
        ));
    }

    #[test]
    fn win32_plans_go_through_the_launcher() {
        assert!(needs_win32(&Plan::Windows {
            program: "/a/x.exe".into(),
            args: vec![]
        }));
        assert!(needs_win32(&Plan::Installer("/a/x.msi".into())));
        assert!(!needs_win32(&Plan::Associated {
            app: "notepad",
            document: "/a/x.txt".into()
        }));
        let cmd = command(&Plan::Windows {
            program: "/a/My Shortcut.lnk".into(),
            args: vec![],
        })
        .unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(cmd.get_program(), "wine");
        assert_eq!(args, ["start", "/unix", "/a/My Shortcut.lnk"]);
    }

    #[test]
    fn run_dialog_knows_windows_names() {
        assert!(run_command("Notepad.exe").is_some());
        assert!(run_command("cmd").is_some());
        assert_eq!(
            run_command("https://example.org").unwrap().get_program(),
            BROWSER
        );
        assert_eq!(run_command("msedge").unwrap().get_program(), BROWSER);
        assert!(run_command("definitely-not-a-command").is_none());
    }
}
