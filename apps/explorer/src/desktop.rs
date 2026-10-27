//! The desktop window and shell host: wallpaper, desktop icons, the IPC endpoint for
//! later `explorer` invocations, and supervision of the taskbar process.
use crate::{
    actions::{self, Cmd, ThemeWatch},
    files,
    folder::Location,
    ipc::{self, Request},
    system::Power,
};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Sense};
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::mpsc::{channel, Receiver},
    time::{Duration, Instant},
};
use wc_paths::{KnownFolder, Namespace};
use wc_ui::{icons::icon_for_name, Icon};
use wc_wayland::layer::{Anchor, KeyboardInteractivity, Layer, Options};

const CELL: egui::Vec2 = egui::vec2(88.0, 92.0);

#[derive(Clone, Debug, PartialEq)]
enum Target {
    Location(Location),
    Path(PathBuf),
}

#[derive(Clone, Debug)]
struct Item {
    label: String,
    icon: Icon,
    target: Target,
}

/// The desktop is the background layer of the whole screen, under every window and
/// under the taskbar (it ignores the taskbar's reserved zone).
pub fn run(first: Request) -> Result<(), String> {
    let options = Options {
        namespace: "wincraft-desktop",
        layer: Layer::Background,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        size: (0, 0),
        exclusive_zone: -1,
        keyboard: KeyboardInteractivity::OnDemand,
    };
    actions::run_layer(options, move |ctx| {
        let (sender, requests) = channel();
        let wake = ctx.clone();
        if let Err(e) = ipc::serve(sender.clone(), move || wake.request_repaint()) {
            eprintln!("explorer: shell socket: {e}");
        }
        if first != Request::Shell {
            let _ = sender.send(first);
        }
        Desktop::new(requests)
    })
}

pub struct Desktop {
    ns: Namespace,
    theme: ThemeWatch,
    requests: Receiver<Request>,
    folder: PathBuf,
    items: Vec<Item>,
    selected: Option<usize>,
    refreshed: Instant,
    taskbar: Option<Child>,
    taskbar_started: Instant,
    start_menu: Option<Child>,
    announced: bool,
    /// Why user data is not on disk, when it is not (written by wincraft-init, FIX-02).
    temporary: Option<String>,
}

/// `Some(reason)` when changes live only in RAM and are lost at shutdown.
fn temporary_storage() -> Option<String> {
    let state = std::fs::read_to_string("/run/wincraft/persistence").ok()?;
    state.strip_prefix("temporary").map(|r| r.trim().to_owned())
}

impl Desktop {
    fn new(requests: Receiver<Request>) -> Self {
        let ns = Namespace::from_env();
        let mut desktop = Self {
            folder: ns.known_folder(KnownFolder::Desktop),
            ns,
            theme: ThemeWatch::new(),
            requests,
            items: vec![],
            selected: None,
            refreshed: Instant::now(),
            taskbar: None,
            taskbar_started: Instant::now(),
            start_menu: None,
            announced: false,
            temporary: None,
        };
        desktop.refresh();
        desktop
    }

    fn refresh(&mut self) {
        let mut items = vec![
            Item {
                label: "This PC".into(),
                icon: Icon::ThisPc,
                target: Target::Location(Location::ThisPc),
            },
            Item {
                label: "Recycle Bin".into(),
                icon: Icon::RecycleBin,
                target: Target::Location(Location::RecycleBin),
            },
            Item {
                label: self.ns.user.clone(),
                icon: Icon::Folder,
                target: Target::Path(self.ns.home.clone()),
            },
            Item {
                label: "Applications (D:)".into(),
                icon: Icon::Drive,
                target: Target::Path(self.ns.apps.clone()),
            },
        ];
        if let Ok(entries) = files::list(&self.folder) {
            items.extend(entries.into_iter().filter(|e| !e.hidden).map(|e| Item {
                icon: icon_for_name(&e.name, e.is_dir),
                label: e.name,
                target: Target::Path(e.path),
            }));
        }
        self.items = items;
        self.temporary = temporary_storage();
        self.refreshed = Instant::now();
    }

    fn open_cmd(item: &Item) -> Cmd {
        match &item.target {
            Target::Location(l) => Cmd::OpenLocation(l.clone()),
            Target::Path(p) => Cmd::Open(p.clone()),
        }
    }

    fn self_command(args: &[&str]) -> Option<Child> {
        let exe = std::env::current_exe().ok()?;
        Command::new(exe).args(args).spawn().ok()
    }

    /// The taskbar is a separate process; restart it if it dies (like explorer.exe does).
    fn supervise_taskbar(&mut self) {
        let alive = self
            .taskbar
            .as_mut()
            .is_some_and(|c| c.try_wait().ok().flatten().is_none());
        if !alive && self.taskbar_started.elapsed() > Duration::from_secs(2) {
            self.taskbar = Self::self_command(&["--taskbar"]);
            self.taskbar_started = Instant::now();
        }
    }

    fn close_start(&mut self) {
        if let Some(mut child) = self.start_menu.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Start button / Win key: open the Start menu, or close it if open.
    fn toggle_start(&mut self) {
        if let Some(child) = &mut self.start_menu {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
                self.start_menu = None;
                return;
            }
        }
        self.start_menu = Self::self_command(&["--start-menu"]);
    }

    fn handle(&mut self, request: Request) {
        let cmd = match request {
            Request::Shell => Cmd::OpenLocation(Location::ThisPc),
            Request::StartMenu => {
                self.toggle_start();
                return;
            }
            Request::CloseStart => {
                self.close_start();
                return;
            }
            Request::Run => Cmd::RunDialog,
            Request::Settings => Cmd::Settings,
            Request::Open(target) => match target.to_ascii_lowercase().as_str() {
                "shell:thispc" | "shell:mycomputerfolder" => Cmd::OpenLocation(Location::ThisPc),
                "shell:recyclebinfolder" => Cmd::OpenLocation(Location::RecycleBin),
                _ => match self
                    .ns
                    .to_unix(&target)
                    .or_else(|| Some(PathBuf::from(&target)))
                {
                    Some(path) if path.exists() => Cmd::Open(path),
                    _ => Cmd::Message(format!("WinCraft cannot find '{target}'.")),
                },
            },
        };
        actions::execute_or_report(cmd, &self.ns);
    }
}

impl wc_wayland::layer::ShellApp for Desktop {
    fn update(&mut self, ctx: &egui::Context) {
        self.theme.tick(ctx);
        actions::debug_input(ctx, "desktop");
        while let Ok(request) = self.requests.try_recv() {
            self.handle(request);
        }
        self.supervise_taskbar();
        if self.refreshed.elapsed() > Duration::from_secs(3) {
            self.refresh();
        }
        let mut cmds = vec![];
        let dark = self.theme.theme.dark;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let full = ui.max_rect();
                wc_ui::wallpaper::paint(ui.painter(), full, dark);
                // The desktop never takes focus from the Start menu, so close it explicitly.
                if ui.input(|i| i.pointer.any_pressed()) {
                    self.close_start();
                }
                let background = ui.interact(full, ui.id().with("desktop-bg"), Sense::click());
                if background.clicked() {
                    self.selected = None;
                }
                background.context_menu(|ui| {
                    if ui.button("New folder").clicked() {
                        if let Err(e) = files::create_unique(&self.folder, "New folder", "", true) {
                            cmds.push(Cmd::Message(format!("Cannot create a folder: {e}")));
                        }
                        ui.close();
                    }
                    if ui.button("New text document").clicked() {
                        if let Err(e) =
                            files::create_unique(&self.folder, "New Text Document", ".txt", false)
                        {
                            cmds.push(Cmd::Message(format!("Cannot create a file: {e}")));
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Refresh").clicked() {
                        ui.close();
                    }
                    if ui.button("Open in Terminal").clicked() {
                        cmds.push(actions::terminal_in(&self.folder));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Personalize").clicked() {
                        cmds.push(Cmd::Settings);
                        ui.close();
                    }
                    if ui.button("Sign out").clicked() {
                        cmds.push(Cmd::Power(Power::SignOut));
                        ui.close();
                    }
                });

                // Icons fill columns top to bottom, left to right, like the Windows desktop.
                let rows = ((full.height() - 64.0) / CELL.y).floor().max(1.0) as usize;
                for (index, item) in self.items.iter().enumerate() {
                    let (col, row) = (index / rows, index % rows);
                    let rect = Rect::from_min_size(
                        pos2(
                            full.left() + 6.0 + col as f32 * CELL.x,
                            full.top() + 8.0 + row as f32 * CELL.y,
                        ),
                        CELL,
                    );
                    let response =
                        ui.interact(rect, ui.id().with(("desktop-icon", index)), Sense::click());
                    let painter = ui.painter();
                    let selected = self.selected == Some(index);
                    if selected || response.hovered() {
                        let a = if selected { 70 } else { 35 };
                        painter.rect_filled(
                            rect.shrink(2.0),
                            CornerRadius::same(4),
                            Color32::from_rgba_unmultiplied(0xCC, 0xE8, 0xFF, a),
                        );
                    }
                    let icon_rect = Rect::from_center_size(
                        pos2(rect.center().x, rect.top() + 26.0),
                        vec2(42.0, 42.0),
                    );
                    wc_ui::paint_icon(painter, icon_rect, item.icon, Color32::WHITE);
                    let galley = painter.layout(
                        item.label.clone(),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                        CELL.x - 6.0,
                    );
                    let text_pos = pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 52.0);
                    // A shadow keeps labels readable on any wallpaper.
                    painter.galley(
                        text_pos + vec2(1.0, 1.0),
                        galley.clone(),
                        Color32::from_black_alpha(200),
                    );
                    painter.galley_with_override_text_color(text_pos, galley, Color32::WHITE);
                    if response.clicked() {
                        self.selected = Some(index);
                    }
                    if response.double_clicked() {
                        cmds.push(Self::open_cmd(item));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Open").clicked() {
                            cmds.push(Self::open_cmd(item));
                            ui.close();
                        }
                        if let Target::Path(path) = &item.target {
                            if path.starts_with(&self.folder) && ui.button("Delete").clicked() {
                                if let Err(e) = files::recycle(path) {
                                    cmds.push(Cmd::Message(format!("Cannot delete: {e}")));
                                }
                                ui.close();
                            }
                        }
                    });
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(item) = self.selected.and_then(|i| self.items.get(i)) {
                        cmds.push(Self::open_cmd(item));
                    }
                }
                ui.painter().text(
                    full.right_bottom() - vec2(12.0, 60.0),
                    Align2::RIGHT_BOTTOM,
                    "WinCraft OS · developer preview",
                    FontId::proportional(11.0),
                    Color32::from_white_alpha(140),
                );
                if let Some(reason) = &self.temporary {
                    let text = format!(
                        "Changes are kept in memory only and will be lost at shutdown: {reason}"
                    );
                    let galley = ui.painter().layout(
                        text,
                        FontId::proportional(12.5),
                        Color32::WHITE,
                        520.0,
                    );
                    let rect = Rect::from_min_size(
                        full.right_bottom() - vec2(548.0, 84.0) - vec2(0.0, galley.size().y),
                        galley.size() + vec2(16.0, 10.0),
                    );
                    ui.painter().rect_filled(
                        rect,
                        CornerRadius::same(6),
                        Color32::from_rgba_unmultiplied(0xC4, 0x2B, 0x1C, 220),
                    );
                    ui.painter()
                        .galley(rect.min + vec2(8.0, 5.0), galley, Color32::WHITE);
                }
            });
        for cmd in cmds {
            actions::execute_or_report(cmd, &self.ns);
        }
        if !self.announced {
            // Marker for the automated acceptance test (scripts/smoke-iso.ps1).
            println!("WINCRAFT_DESKTOP_READY");
            self.announced = true;
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
