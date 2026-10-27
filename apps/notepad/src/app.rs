use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
use std::path::{Path, PathBuf};
use wc_paths::{KnownFolder, Namespace};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DialogKind {
    Open,
    SaveAs,
}

/// Minimal file dialog: a Windows-style path field plus the folder's contents.
struct FileDialog {
    kind: DialogKind,
    dir: PathBuf,
    name: String,
    address: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    Exit,
}

pub struct Notepad {
    ns: Namespace,
    path: Option<PathBuf>,
    text: String,
    saved_text: String,
    crlf: bool,
    word_wrap: bool,
    status_bar: bool,
    zoom: f32,
    cursor: (usize, usize),
    dialog: Option<FileDialog>,
    confirm: Option<Pending>,
    error: Option<String>,
    allow_close: bool,
}

impl Notepad {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut n = Self {
            ns: Namespace::from_env(),
            path: None,
            text: String::new(),
            saved_text: String::new(),
            crlf: false,
            word_wrap: true,
            status_bar: true,
            zoom: 1.0,
            cursor: (1, 1),
            dialog: None,
            confirm: None,
            error: None,
            allow_close: false,
        };
        if let Some(path) = path {
            n.load(&path);
        }
        n
    }

    fn dirty(&self) -> bool {
        self.text != self.saved_text
    }

    fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    fn load(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes).unwrap_or_else(|e| {
                    self.error = Some(
                        "The file is not valid UTF-8; unreadable characters were replaced.".into(),
                    );
                    String::from_utf8_lossy(e.as_bytes()).into_owned()
                });
                self.crlf = text.contains("\r\n");
                self.text = text.replace("\r\n", "\n");
                self.saved_text = self.text.clone();
                self.path = Some(path.to_path_buf());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Like Windows Notepad: a new file is created on first save.
                self.path = Some(path.to_path_buf());
                self.text.clear();
                self.saved_text.clear();
            }
            Err(e) => self.error = Some(format!("Cannot open {}: {e}", self.ns.to_windows(path))),
        }
    }

    fn save_to(&mut self, path: &Path) -> bool {
        let data = if self.crlf {
            self.text.replace('\n', "\r\n")
        } else {
            self.text.clone()
        };
        // Atomic: a failed save (full disk, no permission) keeps the previous file intact.
        match wc_paths::write_atomic(path, data.as_bytes()) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.saved_text = self.text.clone();
                true
            }
            Err(e) => {
                self.error = Some(format!("Cannot save {}: {e}", self.ns.to_windows(path)));
                false
            }
        }
    }

    /// Save; asks for a name for untitled documents. Returns true when saved.
    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(path) => self.save_to(&path),
            None => {
                self.open_dialog(DialogKind::SaveAs);
                false
            }
        }
    }

    fn open_dialog(&mut self, kind: DialogKind) {
        let dir = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.ns.known_folder(KnownFolder::Documents));
        let name = if kind == DialogKind::SaveAs {
            format!("{}.txt", self.file_name().trim_end_matches(".txt"))
        } else {
            String::new()
        };
        self.dialog = Some(FileDialog {
            kind,
            address: self.ns.to_windows(&dir),
            dir,
            name,
        });
    }

    fn run_pending(&mut self, pending: Pending, ctx: &egui::Context) {
        match pending {
            Pending::New => {
                self.path = None;
                self.text.clear();
                self.saved_text.clear();
                self.crlf = false;
            }
            Pending::Open => self.open_dialog(DialogKind::Open),
            Pending::Exit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// Runs `pending` now, or first asks to save unsaved changes.
    fn guarded(&mut self, pending: Pending, ctx: &egui::Context) {
        if self.dirty() {
            self.confirm = Some(pending)
        } else {
            self.run_pending(pending, ctx)
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(egui::Button::new("New").shortcut_text("Ctrl+N"))
                    .clicked()
                {
                    self.guarded(Pending::New, ctx);
                }
                if ui
                    .add(egui::Button::new("Open...").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    self.guarded(Pending::Open, ctx);
                }
                if ui
                    .add(egui::Button::new("Save").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save();
                }
                if ui.button("Save as...").clicked() {
                    self.open_dialog(DialogKind::SaveAs);
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    self.guarded(Pending::Exit, ctx);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add(egui::Button::new("Time/Date").shortcut_text("F5"))
                    .clicked()
                {
                    self.insert_time();
                    ui.close();
                }
            });
            ui.menu_button("Format", |ui| {
                ui.checkbox(&mut self.word_wrap, "Word wrap");
            });
            ui.menu_button("View", |ui| {
                if ui
                    .add(egui::Button::new("Zoom in").shortcut_text("Ctrl+Plus"))
                    .clicked()
                {
                    self.zoom = (self.zoom + 0.1).min(3.0);
                }
                if ui
                    .add(egui::Button::new("Zoom out").shortcut_text("Ctrl+Minus"))
                    .clicked()
                {
                    self.zoom = (self.zoom - 0.1).max(0.5);
                }
                if ui.button("Restore default zoom").clicked() {
                    self.zoom = 1.0;
                }
                ui.checkbox(&mut self.status_bar, "Status bar");
            });
        });
    }

    fn insert_time(&mut self) {
        let now = std::process::Command::new("date")
            .arg("+%H:%M %d.%m.%Y")
            .output();
        if let Ok(out) = now {
            self.text
                .push_str(String::from_utf8_lossy(&out.stdout).trim());
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |ctx: &egui::Context, key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key)))
        };
        if pressed(ctx, Key::S) {
            self.save();
        }
        if pressed(ctx, Key::O) {
            self.guarded(Pending::Open, ctx);
        }
        if pressed(ctx, Key::N) {
            self.guarded(Pending::New, ctx);
        }
        if pressed(ctx, Key::Plus) || pressed(ctx, Key::Equals) {
            self.zoom = (self.zoom + 0.1).min(3.0);
        }
        if pressed(ctx, Key::Minus) {
            self.zoom = (self.zoom - 0.1).max(0.5);
        }
        if ctx.input(|i| i.key_pressed(Key::F5)) {
            self.insert_time();
        }
    }

    fn file_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let mut chosen: Option<PathBuf> = None;
        let title = if dialog.kind == DialogKind::Open {
            "Open"
        } else {
            "Save as"
        };
        egui::Modal::new(egui::Id::new("file-dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(title);
            ui.horizontal(|ui| {
                ui.label("Folder:");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut dialog.address).desired_width(f32::INFINITY),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(dir) = self.ns.to_unix(&dialog.address).filter(|d| d.is_dir()) {
                        dialog.dir = dir;
                    }
                    dialog.address = self.ns.to_windows(&dialog.dir);
                }
            });
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .auto_shrink(false)
                .show(ui, |ui| {
                    if let Some(parent) = dialog.dir.parent() {
                        if ui.selectable_label(false, "📁 ..").double_clicked() {
                            dialog.dir = parent.to_path_buf();
                            dialog.address = self.ns.to_windows(&dialog.dir);
                        }
                    }
                    let mut entries: Vec<_> = std::fs::read_dir(&dialog.dir)
                        .map(|d| d.flatten().map(|e| (e.path(), e.path().is_dir())).collect())
                        .unwrap_or_default();
                    entries.retain(|(p, _)| {
                        !p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                    });
                    entries.sort_by_key(|(p, d)| {
                        (
                            !*d,
                            p.file_name().map(|n| n.to_string_lossy().to_lowercase()),
                        )
                    });
                    for (path, is_dir) in entries {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        let r = ui.selectable_label(
                            dialog.name == name,
                            format!("{} {name}", if is_dir { "📁" } else { "📄" }),
                        );
                        if r.clicked() && !is_dir {
                            dialog.name = name.clone();
                        }
                        if r.double_clicked() {
                            if is_dir {
                                dialog.dir = path.clone();
                                dialog.address = self.ns.to_windows(&dialog.dir);
                            } else {
                                chosen = Some(path.clone());
                            }
                        }
                    }
                });
            ui.horizontal(|ui| {
                ui.label("File name:");
                ui.add(egui::TextEdit::singleline(&mut dialog.name).desired_width(300.0));
            });
            ui.horizontal(|ui| {
                if ui.button(title).clicked() && !dialog.name.trim().is_empty() {
                    chosen = Some(dialog.dir.join(dialog.name.trim()));
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
        });
        if let Some(path) = chosen {
            keep = false;
            match dialog.kind {
                DialogKind::Open => self.load(&path),
                DialogKind::SaveAs => {
                    self.save_to(&path);
                }
            }
        }
        if keep {
            self.dialog = Some(dialog);
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.confirm else { return };
        let name = self
            .path
            .as_ref()
            .map(|p| self.ns.to_windows(p))
            .unwrap_or_else(|| "Untitled".into());
        egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
            ui.heading("Notepad");
            ui.label(format!("Do you want to save changes to {name}?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.confirm = None;
                    if self.save() {
                        self.run_pending(pending, ctx);
                    }
                }
                if ui.button("Don't save").clicked() {
                    self.confirm = None;
                    self.saved_text = self.text.clone();
                    self.run_pending(pending, ctx);
                }
                if ui.button("Cancel").clicked() {
                    self.confirm = None;
                }
            });
        });
    }
}

impl eframe::App for Notepad {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let title = format!(
            "{}{} - Notepad",
            if self.dirty() { "*" } else { "" },
            self.file_name()
        );
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Exit);
        }
        self.shortcuts(ctx);
        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu(ui, ctx));
        if self.status_bar {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Ln {}, Col {}", self.cursor.0, self.cursor.1));
                    ui.separator();
                    ui.label(format!("{}%", (self.zoom * 100.0).round()));
                    ui.separator();
                    ui.label(if self.crlf {
                        "Windows (CRLF)"
                    } else {
                        "Unix (LF)"
                    });
                    ui.separator();
                    ui.label("UTF-8");
                    if let Some(error) = &self.error {
                        ui.separator();
                        ui.colored_label(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C), error);
                    }
                });
            });
        }
        let paper = ctx.style().visuals.extreme_bg_color;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(paper))
            .show(ctx, |ui| {
                let font = egui::FontId::monospace(14.0 * self.zoom);
                let mut scroll = egui::ScrollArea::vertical();
                if !self.word_wrap {
                    scroll = egui::ScrollArea::both();
                }
                scroll.auto_shrink(false).show(ui, |ui| {
                    let mut edit = egui::TextEdit::multiline(&mut self.text)
                        .font(font)
                        .frame(false)
                        .margin(egui::Margin::same(8))
                        .desired_width(if self.word_wrap {
                            ui.available_width()
                        } else {
                            f32::INFINITY
                        })
                        .desired_rows(30);
                    if !self.word_wrap {
                        edit = edit.code_editor();
                    }
                    let output = edit.show(ui);
                    if let Some(range) = output.cursor_range {
                        let index = range.primary.index;
                        let before: String = self.text.chars().take(index).collect();
                        let line = before.matches('\n').count() + 1;
                        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
                        self.cursor = (line, col);
                    }
                });
            });
        self.file_dialog(ctx);
        self.confirm_dialog(ctx);
    }
}
