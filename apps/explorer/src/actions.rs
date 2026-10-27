//! What every shell window can ask for, and how it is carried out.
//!
//! The shell runs one window per process (desktop, taskbar, Start menu, each folder
//! window, dialogs). A window never opens another window in-process: it starts a new
//! `explorer --<mode>` process or asks the desktop process over IPC.
use crate::{
    catalog::Action,
    folder::Location,
    ipc,
    system::{self, Power},
};
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
use wc_launch::Plan;
use wc_paths::Namespace;
use wc_ui::Theme;

#[derive(Debug)]
pub enum Cmd {
    OpenLocation(Location),
    /// ShellExecute on a path: folders open a window, files their program.
    Open(PathBuf),
    Spawn(&'static str, Vec<String>),
    Catalog(Action),
    ToggleStart,
    RunDialog,
    Settings,
    Power(Power),
    SetTheme(Theme),
    Message(String),
}

/// Children are waited for in the background, so finished ones never linger as zombies.
pub fn detach(child: std::process::Child) {
    let mut child = child;
    std::thread::spawn(move || child.wait());
}

/// Starts another window of the shell (`explorer --<mode> ...`).
pub fn spawn_self(args: &[&str]) -> Result<(), String> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("explorer"));
    Command::new(exe)
        .args(args)
        .spawn()
        .map(detach)
        .map_err(|e| format!("Cannot open a shell window: {e}"))
}

pub fn location_arg(location: &Location) -> String {
    match location {
        Location::ThisPc => "shell:ThisPC".into(),
        Location::RecycleBin => "shell:RecycleBinFolder".into(),
        Location::Dir(p) => p.display().to_string(),
    }
}

/// Carries out a command. Errors are returned as text for the caller to show.
pub fn execute(cmd: Cmd, ns: &Namespace) -> Result<(), String> {
    match cmd {
        Cmd::OpenLocation(location) => spawn_self(&["--folder", &location_arg(&location)]),
        Cmd::Open(path) => match wc_launch::plan(&path) {
            Plan::Folder(dir) => spawn_self(&["--folder", &dir.display().to_string()]),
            Plan::Unsupported(reason) => Err(reason),
            plan => wc_launch::spawn(&plan)
                .map(detach)
                .map_err(|e| format!("Cannot open {}:\n{e}", ns.to_windows(&path))),
        },
        Cmd::Spawn(program, args) => Command::new(program)
            .args(&args)
            .spawn()
            .map(detach)
            .map_err(|e| format!("Cannot start {program}: {e}")),
        Cmd::Catalog(action) => match action {
            Action::ThisPc => execute(Cmd::OpenLocation(Location::ThisPc), ns),
            Action::OpenPath(p) | Action::Launch(p) => execute(Cmd::Open(p), ns),
            Action::Command(program, args) => execute(
                Cmd::Spawn(program, args.iter().map(|a| a.to_string()).collect()),
                ns,
            ),
            Action::RunDialog => execute(Cmd::RunDialog, ns),
            Action::Settings => execute(Cmd::Settings, ns),
        },
        Cmd::ToggleStart => ipc::forward(&ipc::Request::StartMenu)
            .or_else(|_| spawn_self(&["--start-menu"]).map_err(std::io::Error::other))
            .map_err(|e| e.to_string()),
        Cmd::RunDialog => spawn_self(&["--run-dialog"]),
        Cmd::Settings => spawn_self(&["--settings-window"]),
        Cmd::Power(action) => {
            system::request_power(action).map_err(|e| format!("The request could not be sent: {e}"))
        }
        Cmd::SetTheme(theme) => theme
            .save()
            .map_err(|e| format!("Could not save the theme: {e}")),
        Cmd::Message(text) => Err(text),
    }
}

/// For windows without their own message area: errors open a message box window.
pub fn execute_or_report(cmd: Cmd, ns: &Namespace) {
    if std::env::var_os("WINCRAFT_DEBUG").is_some() {
        eprintln!("WINCRAFT_CMD {cmd:?}");
    }
    if let Err(text) = execute(cmd, ns) {
        let _ = spawn_self(&["--message", &text]);
    }
}

/// Follows the theme chosen in Settings (written to a file, polled by every window).
pub struct ThemeWatch {
    pub theme: Theme,
    applied: Option<bool>,
    checked: Instant,
}

impl ThemeWatch {
    pub fn new() -> Self {
        Self {
            theme: Theme::load(),
            applied: None,
            checked: Instant::now(),
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        if self.checked.elapsed() > Duration::from_secs(2) {
            self.theme = Theme::load();
            self.checked = Instant::now();
        }
        if self.applied != Some(self.theme.dark) {
            self.theme.apply(ctx);
            self.applied = Some(self.theme.dark);
        }
    }
}

/// Runs one shell window as this process's only window.
pub fn run_window<A: eframe::App + 'static>(
    viewport: egui::ViewportBuilder,
    make: impl FnOnce(&egui::Context) -> A + 'static,
) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: viewport.with_app_id("explorer"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "explorer",
        options,
        Box::new(move |cc| {
            wc_ui::theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(make(&cc.egui_ctx)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Runs a shell surface that the compositor places by layer (desktop, taskbar).
pub fn run_layer<A: wc_wayland::layer::ShellApp + 'static>(
    options: wc_wayland::layer::Options,
    make: impl FnOnce(&egui::Context) -> A,
) -> Result<(), String> {
    wc_wayland::layer::run(options, |ctx| {
        wc_ui::theme::install_fonts(ctx);
        Box::new(make(ctx))
    })
}

/// A terminal window whose shell starts in `dir` (argv only, no shell interpolation).
pub fn terminal_in(dir: &std::path::Path) -> Cmd {
    Cmd::Spawn(
        wc_launch::TERMINAL,
        vec![
            "--title".into(),
            "Terminal".into(),
            "--working-directory".into(),
            dir.display().to_string(),
        ],
    )
}

/// `WINCRAFT_DEBUG=1`: log pointer buttons and focus to stderr (the serial console).
pub fn debug_input(ctx: &egui::Context, window: &str) {
    if std::env::var_os("WINCRAFT_DEBUG").is_none() {
        return;
    }
    ctx.input(|i| {
        for event in &i.raw.events {
            if matches!(
                event,
                egui::Event::PointerButton { .. } | egui::Event::WindowFocused(_)
            ) {
                eprintln!("WINCRAFT_INPUT {window}: {event:?}");
            }
        }
    });
}
