//! File Explorer windows.
use crate::{
    actions::{self, Cmd, ThemeWatch},
    files::{self, Entry, SortKey},
    system,
};
use egui::{
    pos2, vec2, Align2, CornerRadius, FontId, Rect, Sense, ViewportBuilder, ViewportCommand,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use wc_paths::{KnownFolder, Namespace};
use wc_ui::{icons::icon_for_name, widgets, Icon, Theme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    ThisPc,
    RecycleBin,
    Dir(PathBuf),
}

const ROW: f32 = 26.0;
const COLUMNS: [(&str, SortKey, f32); 4] = [
    ("Name", SortKey::Name, 0.0),
    ("Date modified", SortKey::Modified, 0.46),
    ("Type", SortKey::Type, 0.66),
    ("Size", SortKey::Size, 0.86),
];

pub struct FolderWindow {
    location: Location,
    back: Vec<Location>,
    forward: Vec<Location>,
    address: String,
    search: String,
    entries: Vec<Entry>,
    error: Option<String>,
    selected: Option<PathBuf>,
    sort: (SortKey, bool),
    show_hidden: bool,
    properties: Option<PathBuf>,
    refreshed: Instant,
}

impl FolderWindow {
    pub fn new(location: Location, ns: &Namespace) -> Self {
        let mut w = Self {
            location: location.clone(),
            back: vec![],
            forward: vec![],
            address: String::new(),
            search: String::new(),
            entries: vec![],
            error: None,
            selected: None,
            sort: (SortKey::Name, true),
            show_hidden: false,
            properties: None,
            refreshed: Instant::now(),
        };
        w.load(ns);
        w
    }

    fn title(&self, ns: &Namespace) -> String {
        let name = match &self.location {
            Location::ThisPc => "This PC".to_owned(),
            Location::RecycleBin => "Recycle Bin".to_owned(),
            Location::Dir(p) => ns.display_name(p),
        };
        format!("{name} - File Explorer")
    }

    fn load(&mut self, ns: &Namespace) {
        self.error = None;
        self.address = match &self.location {
            Location::ThisPc => "This PC".into(),
            Location::RecycleBin => "Recycle Bin".into(),
            Location::Dir(p) => ns.to_windows(p),
        };
        let dir = match &self.location {
            Location::ThisPc => {
                self.entries.clear();
                self.refreshed = Instant::now();
                return;
            }
            Location::RecycleBin => files::recycle_bin_files(),
            Location::Dir(p) => p.clone(),
        };
        match files::list(&dir) {
            Ok(mut entries) => {
                files::sort(&mut entries, self.sort.0, self.sort.1);
                self.entries = entries;
            }
            Err(e)
                if self.location == Location::RecycleBin
                    && e.kind() == std::io::ErrorKind::NotFound =>
            {
                self.entries.clear();
            }
            Err(e) => {
                self.entries.clear();
                self.error = Some(format!("{} is not accessible.\n\n{e}", self.address));
            }
        }
        self.refreshed = Instant::now();
    }

    fn navigate(&mut self, location: Location, ns: &Namespace) {
        if location != self.location {
            self.back
                .push(std::mem::replace(&mut self.location, location));
            self.forward.clear();
        }
        self.selected = None;
        self.search.clear();
        self.load(ns);
    }

    fn go_back(&mut self, ns: &Namespace) {
        if let Some(previous) = self.back.pop() {
            self.forward
                .push(std::mem::replace(&mut self.location, previous));
            self.load(ns);
        }
    }

    fn go_forward(&mut self, ns: &Namespace) {
        if let Some(next) = self.forward.pop() {
            self.back.push(std::mem::replace(&mut self.location, next));
            self.load(ns);
        }
    }

    fn parent(&self, ns: &Namespace) -> Option<Location> {
        match &self.location {
            Location::ThisPc => None,
            Location::RecycleBin => Some(Location::ThisPc),
            Location::Dir(p) => {
                // Drive roots go up to This PC, as in Windows.
                if ns.drives().iter().any(|d| &d.root == p) {
                    Some(Location::ThisPc)
                } else {
                    p.parent().map(|p| Location::Dir(p.to_path_buf()))
                }
            }
        }
    }

    /// Double-click or Enter on an item.
    fn activate(&mut self, entry: &Entry, ns: &Namespace, cmds: &mut Vec<Cmd>) {
        if self.location == Location::RecycleBin {
            self.properties = Some(entry.path.clone());
        } else if entry.is_dir {
            self.navigate(Location::Dir(entry.path.clone()), ns);
        } else {
            cmds.push(Cmd::Open(entry.path.clone()));
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, ns: &Namespace, theme: &Theme) -> Vec<Cmd> {
        let mut cmds = vec![];
        if self.refreshed.elapsed() > Duration::from_secs(3) && self.location != Location::ThisPc {
            // Reflect changes made by other programs, as Windows does through notifications.
            let selected = self.selected.clone();
            self.load(ns);
            self.selected = selected;
        }
        self.toolbar(ctx, ns, theme, &mut cmds);
        self.navigation_pane(ctx, ns, theme);
        self.status_bar(ctx, theme);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme.palette.surface)
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ctx, |ui| {
                if let Some(error) = &self.error {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| ui.label(error.as_str()));
                    return;
                }
                match self.location {
                    Location::ThisPc => self.this_pc(ui, ns, theme),
                    _ => self.details(ui, ns, theme, &mut cmds),
                }
            });
        self.properties_window(ctx, ns, &mut cmds);
        self.keyboard(ctx, ns, &mut cmds);
        cmds
    }

    fn toolbar(&mut self, ctx: &egui::Context, ns: &Namespace, theme: &Theme, cmds: &mut Vec<Cmd>) {
        let p = theme.palette;
        egui::TopBottomPanel::top("nav")
            .frame(egui::Frame::NONE.fill(p.base).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if widgets::icon_button(ui, Icon::Back, "Back (Alt+Left)", !self.back.is_empty()).clicked() {
                        self.go_back(ns);
                    }
                    if widgets::icon_button(ui, Icon::Forward, "Forward (Alt+Right)", !self.forward.is_empty()).clicked() {
                        self.go_forward(ns);
                    }
                    let parent = self.parent(ns);
                    if widgets::icon_button(ui, Icon::Up, "Up to the parent folder", parent.is_some()).clicked() {
                        if let Some(parent) = parent {
                            self.navigate(parent, ns);
                        }
                    }
                    if widgets::icon_button(ui, Icon::Refresh, "Refresh (F5)", true).clicked() {
                        self.load(ns);
                    }
                    let search_width = 220.0;
                    let address = ui.add(
                        egui::TextEdit::singleline(&mut self.address)
                            .desired_width(ui.available_width() - search_width - 12.0)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                    if address.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let typed = self.address.trim().to_owned();
                        if typed.eq_ignore_ascii_case("This PC") {
                            self.navigate(Location::ThisPc, ns);
                        } else if typed.eq_ignore_ascii_case("Recycle Bin") {
                            self.navigate(Location::RecycleBin, ns);
                        } else {
                            match ns.to_unix(&typed) {
                                Some(path) if path.is_dir() => self.navigate(Location::Dir(path), ns),
                                Some(path) if path.exists() => cmds.push(Cmd::Open(path)),
                                _ => {
                                    cmds.push(Cmd::Message(format!("WinCraft can't find '{typed}'. Check the spelling and try again.")));
                                    self.load(ns);
                                }
                            }
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search")
                            .desired_width(search_width)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                });
            });
        egui::TopBottomPanel::top("commands")
            .frame(
                egui::Frame::NONE
                    .fill(p.base)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let dir = match &self.location {
                        Location::Dir(d) => Some(d.clone()),
                        _ => None,
                    };
                    if self.location == Location::RecycleBin {
                        if widgets::command_button(ui, Icon::Restart, "Restore").clicked() {
                            if let Some(path) = self.selected.clone() {
                                match files::restore(&path) {
                                    Ok(_) => self.load(ns),
                                    Err(e) => {
                                        cmds.push(Cmd::Message(format!("Cannot restore: {e}")))
                                    }
                                }
                            }
                        }
                        if widgets::command_button(ui, Icon::Delete, "Empty Recycle Bin").clicked()
                        {
                            match files::empty_recycle_bin() {
                                Ok(()) => self.load(ns),
                                Err(e) => cmds.push(Cmd::Message(format!(
                                    "Cannot empty the Recycle Bin: {e}"
                                ))),
                            }
                        }
                    }
                    if let Some(dir) = dir {
                        if widgets::command_button(ui, Icon::NewFolder, "New folder").clicked() {
                            match files::create_unique(&dir, "New folder", "", true) {
                                Ok(path) => {
                                    self.load(ns);
                                    self.selected = Some(path);
                                }
                                Err(e) => cmds.push(Cmd::Message(format!(
                                    "Cannot create a folder here: {e}"
                                ))),
                            }
                        }
                        if widgets::command_button(ui, Icon::TextFile, "New text document")
                            .clicked()
                        {
                            match files::create_unique(&dir, "New Text Document", ".txt", false) {
                                Ok(path) => {
                                    self.load(ns);
                                    self.selected = Some(path);
                                }
                                Err(e) => cmds
                                    .push(Cmd::Message(format!("Cannot create a file here: {e}"))),
                            }
                        }
                        let can_delete = self.selected.is_some();
                        if ui
                            .add_enabled_ui(can_delete, |ui| {
                                widgets::command_button(ui, Icon::Delete, "Delete")
                            })
                            .inner
                            .clicked()
                        {
                            self.delete_selected(ns, cmds);
                        }
                        if widgets::command_button(ui, Icon::Terminal, "Open in Terminal").clicked()
                        {
                            cmds.push(terminal_in(&dir));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.checkbox(&mut self.show_hidden, "Hidden items").changed() {
                            self.selected = None;
                        }
                    });
                });
            });
    }

    fn delete_selected(&mut self, ns: &Namespace, cmds: &mut Vec<Cmd>) {
        if let Some(path) = self.selected.take() {
            if let Err(e) = files::recycle(&path) {
                cmds.push(Cmd::Message(format!(
                    "Cannot move {} to the Recycle Bin: {e}",
                    ns.to_windows(&path)
                )));
            }
            self.load(ns);
        }
    }

    fn navigation_pane(&mut self, ctx: &egui::Context, ns: &Namespace, theme: &Theme) {
        egui::SidePanel::left("navigation")
            .resizable(false)
            .exact_width(206.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme.palette.base)
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    widgets::section_header(ui, "Quick access");
                    for folder in KnownFolder::USER_FOLDERS {
                        let path = ns.known_folder(folder);
                        let selected = self.location == Location::Dir(path.clone());
                        if widgets::nav_item(
                            ui,
                            icon_for_name(folder.dir_name(), true),
                            folder.dir_name(),
                            selected,
                        )
                        .clicked()
                        {
                            self.navigate(Location::Dir(path), ns);
                        }
                    }
                    widgets::section_header(ui, "This PC");
                    if widgets::nav_item(
                        ui,
                        Icon::ThisPc,
                        "This PC",
                        self.location == Location::ThisPc,
                    )
                    .clicked()
                    {
                        self.navigate(Location::ThisPc, ns);
                    }
                    for drive in ns.drives() {
                        let icon = if drive.letter == 'C' {
                            Icon::SystemDrive
                        } else {
                            Icon::Drive
                        };
                        let selected = self.location == Location::Dir(drive.root.clone());
                        if widgets::nav_item(ui, icon, &drive.display_name(), selected).clicked() {
                            self.navigate(Location::Dir(drive.root.clone()), ns);
                        }
                    }
                    ui.add_space(6.0);
                    if widgets::nav_item(
                        ui,
                        Icon::RecycleBin,
                        "Recycle Bin",
                        self.location == Location::RecycleBin,
                    )
                    .clicked()
                    {
                        self.navigate(Location::RecycleBin, ns);
                    }
                });
            });
    }

    fn visible_entries(&self) -> Vec<&Entry> {
        let query = self.search.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .filter(|e| query.is_empty() || e.name.to_lowercase().contains(&query))
            .collect()
    }

    fn status_bar(&mut self, ctx: &egui::Context, theme: &Theme) {
        let count = self.visible_entries().len();
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::NONE
                    .fill(theme.palette.base)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.location != Location::ThisPc {
                        ui.label(format!("{count} item{}", if count == 1 { "" } else { "s" }));
                    }
                    if let Some(selected) = &self.selected {
                        if let Some(entry) = self.entries.iter().find(|e| &e.path == selected) {
                            ui.label("|");
                            ui.label(if entry.is_dir {
                                "1 item selected".into()
                            } else {
                                format!("1 item selected  {}", widgets::human_size(entry.size))
                            });
                        }
                    }
                });
            });
    }

    fn this_pc(&mut self, ui: &mut egui::Ui, ns: &Namespace, theme: &Theme) {
        let p = theme.palette;
        egui::ScrollArea::vertical().show(ui, |ui| {
            widgets::section_header(ui, "Folders");
            ui.horizontal_wrapped(|ui| {
                for folder in KnownFolder::USER_FOLDERS {
                    if widgets::tile(
                        ui,
                        icon_for_name(folder.dir_name(), true),
                        folder.dir_name(),
                        vec2(110.0, 86.0),
                    )
                    .double_clicked()
                    {
                        self.navigate(Location::Dir(ns.known_folder(folder)), ns);
                    }
                }
            });
            ui.add_space(8.0);
            widgets::section_header(ui, "Devices and drives");
            ui.horizontal_wrapped(|ui| {
                for drive in ns.drives() {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(270.0, 66.0), Sense::click());
                    if response.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(6), p.hover);
                    }
                    let icon = if drive.letter == 'C' {
                        Icon::SystemDrive
                    } else {
                        Icon::Drive
                    };
                    wc_ui::paint_icon(
                        ui.painter(),
                        Rect::from_min_size(rect.min + vec2(8.0, 10.0), vec2(44.0, 44.0)),
                        icon,
                        p.text,
                    );
                    ui.painter().text(
                        rect.min + vec2(62.0, 14.0),
                        Align2::LEFT_CENTER,
                        drive.display_name(),
                        FontId::proportional(13.5),
                        p.text,
                    );
                    if let Some((free, total)) = system::disk_space(&drive.root) {
                        let used = 1.0 - free as f32 / total.max(1) as f32;
                        let bar =
                            Rect::from_min_size(rect.min + vec2(62.0, 28.0), vec2(190.0, 6.0));
                        ui.painter()
                            .rect_filled(bar, CornerRadius::same(3), p.border);
                        let mut fill = bar;
                        fill.set_width(bar.width() * used.clamp(0.0, 1.0));
                        ui.painter().rect_filled(
                            fill,
                            CornerRadius::same(3),
                            if used > 0.9 { p.danger } else { p.accent },
                        );
                        ui.painter().text(
                            rect.min + vec2(62.0, 48.0),
                            Align2::LEFT_CENTER,
                            format!(
                                "{} free of {}",
                                widgets::human_size(free),
                                widgets::human_size(total)
                            ),
                            FontId::proportional(11.5),
                            p.text_dim,
                        );
                    }
                    if response.double_clicked() {
                        self.navigate(Location::Dir(drive.root.clone()), ns);
                    }
                }
            });
        });
    }

    fn details(&mut self, ui: &mut egui::Ui, ns: &Namespace, theme: &Theme, cmds: &mut Vec<Cmd>) {
        let p = theme.palette;
        let width = ui.available_width();
        let left = ui.min_rect().left();
        // Column headers; clicking sorts, clicking again reverses.
        let (header, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
        for (label, key, at) in COLUMNS {
            let x = header.left() + width * at;
            let rect = Rect::from_min_max(
                pos2(x, header.top()),
                pos2(x + width * 0.2, header.bottom()),
            );
            let r = ui.interact(rect, ui.id().with(("col", label)), Sense::click());
            let arrow = if self.sort.0 == key {
                if self.sort.1 {
                    " ˄"
                } else {
                    " ˅"
                }
            } else {
                ""
            };
            ui.painter().text(
                pos2(x + 30.0 * (at == 0.0) as u8 as f32 + 4.0, header.center().y),
                Align2::LEFT_CENTER,
                format!("{label}{arrow}"),
                FontId::proportional(12.5),
                p.text_dim,
            );
            if r.clicked() {
                self.sort = if self.sort.0 == key {
                    (key, !self.sort.1)
                } else {
                    (key, true)
                };
                files::sort(&mut self.entries, self.sort.0, self.sort.1);
            }
        }
        widgets::separator(ui);
        let entries: Vec<Entry> = self.visible_entries().into_iter().cloned().collect();
        if entries.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(if self.location == Location::RecycleBin {
                        "The Recycle Bin is empty."
                    } else {
                        "This folder is empty."
                    })
                    .color(p.text_dim),
                );
            });
        }
        let mut activate = None;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            ROW,
            entries.len(),
            |ui, range| {
                for entry in &entries[range] {
                    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), Sense::click());
                    let selected = self.selected.as_ref() == Some(&entry.path);
                    if selected {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), p.selection);
                    } else if response.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), p.hover);
                    }
                    let painter = ui.painter().with_clip_rect(rect);
                    wc_ui::paint_icon(
                        &painter,
                        Rect::from_min_size(pos2(left + 4.0, rect.top() + 3.0), vec2(20.0, 20.0)),
                        icon_for_name(&entry.name, entry.is_dir),
                        p.text,
                    );
                    let text_color = if entry.hidden { p.text_dim } else { p.text };
                    let column = |at: f32| pos2(rect.left() + width * at + 4.0, rect.center().y);
                    painter
                        .with_clip_rect(Rect::from_min_max(
                            rect.min,
                            pos2(rect.left() + width * 0.45, rect.bottom()),
                        ))
                        .text(
                            column(0.0) + vec2(30.0, 0.0),
                            Align2::LEFT_CENTER,
                            &entry.name,
                            FontId::proportional(13.0),
                            text_color,
                        );
                    let date = if self.location == Location::RecycleBin {
                        files::original_location(&entry.path)
                            .map(|p| ns.to_windows(&p))
                            .unwrap_or_default()
                    } else {
                        files::format_time(entry.modified)
                    };
                    painter.text(
                        column(0.46),
                        Align2::LEFT_CENTER,
                        date,
                        FontId::proportional(12.5),
                        p.text_dim,
                    );
                    painter.text(
                        column(0.66),
                        Align2::LEFT_CENTER,
                        files::type_name(&entry.name, entry.is_dir),
                        FontId::proportional(12.5),
                        p.text_dim,
                    );
                    if !entry.is_dir {
                        painter.text(
                            pos2(rect.right() - 12.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            widgets::human_size(entry.size),
                            FontId::proportional(12.5),
                            p.text_dim,
                        );
                    }
                    if response.clicked() || response.secondary_clicked() {
                        self.selected = Some(entry.path.clone());
                    }
                    if response.double_clicked() {
                        activate = Some(entry.clone());
                    }
                    response.context_menu(|ui| {
                        if self.location == Location::RecycleBin {
                            if ui.button("Restore").clicked() {
                                if let Err(e) = files::restore(&entry.path) {
                                    cmds.push(Cmd::Message(format!("Cannot restore: {e}")));
                                }
                                ui.close();
                            }
                        } else {
                            if ui.button(egui::RichText::new("Open").strong()).clicked() {
                                activate = Some(entry.clone());
                                ui.close();
                            }
                            let zip = !entry.is_dir
                                && entry
                                    .path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
                            if zip && ui.button("Extract All…").clicked() {
                                cmds.push(Cmd::Spawn(
                                    "explorer",
                                    vec!["--extract".into(), entry.path.display().to_string()],
                                ));
                                ui.close();
                            }
                            if !entry.is_dir && ui.button("Edit with Notepad").clicked() {
                                cmds.push(Cmd::Spawn(
                                    "notepad",
                                    vec![entry.path.display().to_string()],
                                ));
                                ui.close();
                            }
                            if entry.is_dir && ui.button("Open in new window").clicked() {
                                cmds.push(Cmd::OpenLocation(Location::Dir(entry.path.clone())));
                                ui.close();
                            }
                            if ui.button("Copy as path").clicked() {
                                ui.ctx()
                                    .copy_text(format!("\"{}\"", ns.to_windows(&entry.path)));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Delete").clicked() {
                                self.selected = Some(entry.path.clone());
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Properties").clicked() {
                            self.properties = Some(entry.path.clone());
                            ui.close();
                        }
                    });
                }
            },
        );
        if let Some(entry) = activate {
            self.activate(&entry, ns, cmds);
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context, ns: &Namespace, cmds: &mut Vec<Cmd>) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (enter, backspace, delete, f5, alt_left, alt_right, up, down) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::F5),
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if alt_left || backspace {
            if backspace {
                if let Some(parent) = self.parent(ns) {
                    self.navigate(parent, ns);
                }
            } else {
                self.go_back(ns);
            }
        }
        if alt_right {
            self.go_forward(ns);
        }
        if f5 {
            self.load(ns);
        }
        if up || down {
            let visible: Vec<PathBuf> = self
                .visible_entries()
                .iter()
                .map(|e| e.path.clone())
                .collect();
            let current = self
                .selected
                .as_ref()
                .and_then(|s| visible.iter().position(|p| p == s));
            let next = match (current, down) {
                (None, _) => 0,
                (Some(i), true) => (i + 1).min(visible.len().saturating_sub(1)),
                (Some(i), false) => i.saturating_sub(1),
            };
            self.selected = visible.get(next).cloned();
        }
        if enter {
            if let Some(entry) = self
                .selected
                .as_ref()
                .and_then(|s| self.entries.iter().find(|e| &e.path == s))
                .cloned()
            {
                self.activate(&entry, ns, cmds);
            }
        }
        if delete
            && self.location != Location::RecycleBin
            && matches!(self.location, Location::Dir(_))
        {
            self.delete_selected(ns, cmds);
        }
    }

    fn properties_window(&mut self, ctx: &egui::Context, ns: &Namespace, cmds: &mut Vec<Cmd>) {
        let Some(path) = self.properties.clone() else {
            return;
        };
        let mut open = true;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        egui::Window::new(format!("{name} Properties"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let meta = std::fs::metadata(&path).ok();
                let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                ui.horizontal(|ui| {
                    wc_ui::icons::icon(ui, icon_for_name(&name, is_dir), 36.0);
                    ui.strong(&name);
                });
                ui.separator();
                let format = if is_dir {
                    None
                } else {
                    wc_formats::classify_file(&path).ok()
                };
                let kind = match &format {
                    Some(f) if *f != wc_formats::Format::Unknown => f.description().to_owned(),
                    _ => files::type_name(&name, is_dir),
                };
                egui::Grid::new("props")
                    .num_columns(2)
                    .spacing(vec2(18.0, 6.0))
                    .show(ui, |ui| {
                        let mut row = |k: &str, v: String| {
                            ui.label(k);
                            ui.label(v);
                            ui.end_row();
                        };
                        row("Type:", kind);
                        let location = files::original_location(&path).unwrap_or_else(|| {
                            path.parent().map(Path::to_path_buf).unwrap_or_default()
                        });
                        row("Location:", ns.to_windows(&location));
                        if let Some(m) = &meta {
                            if !is_dir {
                                row(
                                    "Size:",
                                    format!("{} ({} bytes)", widgets::human_size(m.len()), m.len()),
                                );
                            }
                            row("Modified:", files::format_time(m.modified().ok()));
                        }
                        if let Some(wc_formats::Format::Pe {
                            arch,
                            subsystem,
                            dotnet,
                            ..
                        }) = &format
                        {
                            row(
                                "Platform:",
                                format!("{arch:?}{}", if *dotnet { ", .NET" } else { "" }),
                            );
                            row("Subsystem:", format!("{subsystem:?}"));
                            row("Runs with:", "Win32 subsystem (Wine)".into());
                        }
                        row("Unix path:", path.display().to_string());
                    });
                ui.add_space(6.0);
                if !is_dir && ui.button("Open").clicked() {
                    cmds.push(Cmd::Open(path.clone()));
                }
            });
        if !open {
            self.properties = None;
        }
    }
}

fn terminal_in(dir: &Path) -> Cmd {
    actions::terminal_in(dir)
}

/// `explorer --folder <target>`: one File Explorer window per process.
pub fn run(target: String) -> Result<(), String> {
    let ns = Namespace::from_env();
    let location = match target.to_ascii_lowercase().as_str() {
        "shell:thispc" | "shell:mycomputerfolder" => Location::ThisPc,
        "shell:recyclebinfolder" => Location::RecycleBin,
        _ => Location::Dir(
            ns.to_unix(&target)
                .unwrap_or_else(|| PathBuf::from(&target)),
        ),
    };
    let window = FolderWindow::new(location, &ns);
    let title = window.title(&ns);
    let viewport = ViewportBuilder::default()
        .with_title(title.clone())
        .with_inner_size([920.0, 580.0])
        .with_min_inner_size([520.0, 320.0]);
    actions::run_window(viewport, move |_| FolderApp {
        ns,
        theme: ThemeWatch::new(),
        window,
        messages: vec![],
        title,
    })
}

struct FolderApp {
    ns: Namespace,
    theme: ThemeWatch,
    window: FolderWindow,
    messages: Vec<String>,
    title: String,
}

impl eframe::App for FolderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.tick(ctx);
        actions::debug_input(ctx, "folder");
        let theme = self.theme.theme;
        for cmd in self.window.ui(ctx, &self.ns, &theme) {
            if let Err(text) = actions::execute(cmd, &self.ns) {
                self.messages.push(text);
            }
        }
        let title = self.window.title(&self.ns);
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if let Some(text) = self.messages.first().cloned() {
            egui::Modal::new(egui::Id::new("message")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(text);
                ui.add_space(8.0);
                if ui.button("   OK   ").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    self.messages.remove(0);
                }
            });
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
