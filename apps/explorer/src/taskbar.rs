//! The taskbar: Start, pinned apps, one button per application window, clock.
//!
//! A layer surface along the bottom edge that reserves its height, so maximized windows
//! stop above it. The window list comes from the compositor (wlr-foreign-toplevel), so
//! Windows programs under XWayland appear like native windows.
use crate::{
    actions::{self, Cmd, ThemeWatch},
    catalog::Action,
    system,
};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Sense};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::{Duration, SystemTime},
};
use wc_paths::Namespace;
use wc_ui::Icon;
use wc_wayland::{
    layer::{Anchor, KeyboardInteractivity, Layer, Options},
    WindowInfo, WindowList,
};

pub const TASKBAR_HEIGHT: f32 = 48.0;

enum WmCmd {
    Activate(u32),
    Minimize(u32),
    Close(u32),
    ShowDesktop,
}

#[derive(Default)]
struct WindowState {
    windows: Vec<WindowInfo>,
    active: Option<u32>,
}

pub fn run() -> Result<(), String> {
    let options = Options {
        namespace: "wincraft-taskbar",
        layer: Layer::Top,
        anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        size: (0, TASKBAR_HEIGHT as u32),
        exclusive_zone: TASKBAR_HEIGHT as i32,
        keyboard: KeyboardInteractivity::None,
    };
    actions::run_layer(options, |ctx| Taskbar::start(ctx.clone()))
}

pub struct Taskbar {
    ns: Namespace,
    theme: ThemeWatch,
    state: Arc<Mutex<WindowState>>,
    commands: mpsc::Sender<WmCmd>,
}

impl Taskbar {
    /// Watches the window list in a background thread and repaints on changes.
    fn start(ctx: egui::Context) -> Self {
        let state = Arc::new(Mutex::new(WindowState::default()));
        let (commands, receiver) = mpsc::channel::<WmCmd>();
        let shared = state.clone();
        std::thread::spawn(move || loop {
            // Reconnects if the list fails (for example while the compositor restarts).
            let Ok(mut list) = WindowList::connect() else {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            };
            loop {
                let result = match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(WmCmd::Activate(w)) => list.activate(w),
                    Ok(WmCmd::Minimize(w)) => list.minimize(w),
                    Ok(WmCmd::Close(w)) => list.close(w),
                    Ok(WmCmd::ShowDesktop) => list.toggle_desktop(),
                    Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                };
                if result.and_then(|_| list.refresh()).is_err() {
                    break;
                }
                let windows = list.windows();
                let active = list.active();
                let changed = {
                    let mut s = shared.lock().unwrap();
                    let changed = s.windows != windows || s.active != active;
                    if changed {
                        s.windows = windows;
                        s.active = active;
                    }
                    changed
                };
                // Repaint outside the lock: the UI thread takes the same lock while drawing.
                if changed {
                    ctx.request_repaint();
                }
            }
        });
        Self {
            ns: Namespace::from_env(),
            theme: ThemeWatch::new(),
            state,
            commands,
        }
    }
}

fn window_icon(window: &WindowInfo) -> Icon {
    match window.app_id.to_lowercase().as_str() {
        "explorer" if window.title == "Settings" => Icon::Settings,
        "explorer" if window.title == "Run" => Icon::Run,
        "explorer" => Icon::Folder,
        "notepad" => Icon::Notepad,
        "taskmgr" => Icon::TaskManager,
        "foot" | "xterm" => Icon::Terminal,
        "org.wincraft.browser" => Icon::Browser,
        _ => Icon::Application,
    }
}

impl wc_wayland::layer::ShellApp for Taskbar {
    fn update(&mut self, ctx: &egui::Context) {
        self.theme.tick(ctx);
        actions::debug_input(ctx, "taskbar");
        let p = self.theme.theme.palette;
        let mut cmds = vec![];
        let windows = {
            let s = self.state.lock().unwrap();
            (s.windows.clone(), s.active)
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.taskbar))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.painter().line_segment(
                    [full.left_top(), full.right_top()],
                    egui::Stroke::new(1.0, p.border),
                );
                let mut x = full.left() + 4.0;
                let button = |ui: &mut egui::Ui, rect: Rect, id: &str, icon: Icon, inset: f32| {
                    let r = ui.interact(rect, ui.id().with(id), Sense::click());
                    if r.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(4), p.hover);
                    }
                    wc_ui::paint_icon(ui.painter(), rect.shrink(inset), icon, p.text);
                    r
                };
                let start = button(
                    ui,
                    Rect::from_min_size(pos2(x, full.top() + 4.0), vec2(44.0, 40.0)),
                    "start",
                    Icon::Logo,
                    9.0,
                );
                if start.clicked() {
                    cmds.push(Cmd::ToggleStart);
                } else if ui.input(|i| i.pointer.any_pressed()) && !start.hovered() {
                    // Any other taskbar click closes the Start menu, as in Windows.
                    let _ = crate::ipc::forward(&crate::ipc::Request::CloseStart);
                }
                start.on_hover_text("Start");
                x += 50.0;
                for (icon, tip, action) in [
                    (
                        Icon::Browser,
                        "WinCraft Browser",
                        Action::Command("browser", &[]),
                    ),
                    (Icon::Folder, "File Explorer", Action::ThisPc),
                    (
                        Icon::Terminal,
                        "Terminal",
                        Action::Command("foot", &["--title", "Terminal"]),
                    ),
                    (Icon::Notepad, "Notepad", Action::Command("notepad", &[])),
                ] {
                    let r = button(
                        ui,
                        Rect::from_min_size(pos2(x, full.top() + 4.0), vec2(40.0, 40.0)),
                        tip,
                        icon,
                        10.0,
                    );
                    if r.clicked() {
                        cmds.push(Cmd::Catalog(action));
                    }
                    r.on_hover_text(tip);
                    x += 42.0;
                }
                ui.painter().line_segment(
                    [
                        pos2(x + 2.0, full.top() + 12.0),
                        pos2(x + 2.0, full.bottom() - 12.0),
                    ],
                    egui::Stroke::new(1.0, p.border),
                );
                x += 8.0;

                // "Show desktop" sliver and clock on the right.
                let peek =
                    Rect::from_min_max(pos2(full.right() - 6.0, full.top()), full.right_bottom());
                let peek_r = ui.interact(peek, ui.id().with("peek"), Sense::click());
                if peek_r.hovered() {
                    ui.painter().rect_filled(peek, CornerRadius::ZERO, p.hover);
                }
                if peek_r.clicked() {
                    let _ = self.commands.send(WmCmd::ShowDesktop);
                }
                peek_r.on_hover_text("Show desktop");
                let now = system::local_time(SystemTime::now());
                let clock = Rect::from_min_size(
                    pos2(full.right() - 104.0, full.top() + 4.0),
                    vec2(92.0, 40.0),
                );
                ui.painter().text(
                    clock.center() - vec2(0.0, 8.0),
                    Align2::CENTER_CENTER,
                    now.time(),
                    FontId::proportional(12.5),
                    p.text,
                );
                ui.painter().text(
                    clock.center() + vec2(0.0, 9.0),
                    Align2::CENTER_CENTER,
                    now.date(),
                    FontId::proportional(12.0),
                    p.text,
                );

                // Window buttons share the space between pinned apps and the clock.
                let (list, active_id) = &windows;
                let available = (clock.left() - 8.0 - x).max(0.0);
                let width = (available / list.len().max(1) as f32).clamp(44.0, 200.0);
                for window in list {
                    if x + width > clock.left() - 4.0 {
                        break;
                    }
                    let rect =
                        Rect::from_min_size(pos2(x, full.top() + 4.0), vec2(width - 4.0, 40.0));
                    let r = ui.interact(rect, ui.id().with(("win", window.id)), Sense::click());
                    let active =
                        (*active_id == Some(window.id) || window.active) && !window.minimized;
                    let fill = if active {
                        p.selection
                    } else if r.hovered() {
                        p.hover
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
                    let bar = Rect::from_center_size(
                        pos2(rect.center().x, rect.bottom() - 2.0),
                        vec2(if active { 22.0 } else { 8.0 }, 3.0),
                    );
                    ui.painter().rect_filled(
                        bar,
                        CornerRadius::same(2),
                        if active { p.accent } else { p.text_dim },
                    );
                    wc_ui::paint_icon(
                        ui.painter(),
                        Rect::from_min_size(rect.min + vec2(8.0, 10.0), vec2(20.0, 20.0)),
                        window_icon(window),
                        p.text,
                    );
                    if width > 70.0 {
                        let title = if window.title.is_empty() {
                            window.app_id.clone()
                        } else {
                            window.title.clone()
                        };
                        let text_rect = Rect::from_min_max(
                            rect.min + vec2(34.0, 0.0),
                            rect.max - vec2(6.0, 0.0),
                        );
                        ui.painter().with_clip_rect(text_rect).text(
                            pos2(text_rect.left(), rect.center().y),
                            Align2::LEFT_CENTER,
                            title,
                            FontId::proportional(12.5),
                            if window.minimized { p.text_dim } else { p.text },
                        );
                    }
                    if r.clicked() {
                        let _ = self.commands.send(if active {
                            WmCmd::Minimize(window.id)
                        } else {
                            WmCmd::Activate(window.id)
                        });
                    }
                    let id = window.id;
                    r.on_hover_text(window.title.clone()).context_menu(|ui| {
                        if ui.button("Close window").clicked() {
                            let _ = self.commands.send(WmCmd::Close(id));
                            ui.close();
                        }
                    });
                    x += width;
                }
            });
        for cmd in cmds {
            actions::execute_or_report(cmd, &self.ns);
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
