//! The Start menu window: search, pinned tiles, all apps, user and power.
//! A short-lived process: it closes when it loses focus, on Esc, or after a choice.
use crate::{
    actions::{self, Cmd, ThemeWatch},
    catalog::{self, Action, App},
    system::Power,
};
use egui::{vec2, Align2, CornerRadius, FontId, Sense, ViewportBuilder, ViewportCommand};
use std::time::{Duration, Instant};
use wc_paths::Namespace;
use wc_ui::{widgets, Icon};

const SIZE: egui::Vec2 = egui::vec2(640.0, 580.0);

/// An ordinary window, so it takes the keyboard focus and closes when it loses it. The
/// compositor's window rule for the title "Start" puts it above the Start button, without
/// a frame, a taskbar button or a place in Alt+Tab.
pub fn run() -> Result<(), String> {
    let viewport = ViewportBuilder::default()
        .with_title("Start")
        .with_decorations(false)
        .with_resizable(false)
        .with_inner_size(SIZE);
    actions::run_window(viewport, |_| StartMenu::new())
}

pub struct StartMenu {
    ns: Namespace,
    theme: ThemeWatch,
    search: String,
    all_apps: bool,
    pinned: Vec<App>,
    apps: Vec<App>,
    was_focused: bool,
    opened: Instant,
    focus_search: bool,
}

impl StartMenu {
    fn new() -> Self {
        let ns = Namespace::from_env();
        Self {
            // Scanned on every opening: newly installed Windows programs appear at once.
            pinned: catalog::pinned(&ns),
            apps: catalog::all_apps(&ns),
            ns,
            theme: ThemeWatch::new(),
            search: String::new(),
            all_apps: false,
            was_focused: false,
            opened: Instant::now(),
            focus_search: true,
        }
    }
}

fn app_row(ui: &mut egui::Ui, app: &App, p: wc_ui::Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.hover);
    }
    wc_ui::paint_icon(
        ui.painter(),
        egui::Rect::from_min_size(rect.min + vec2(8.0, 6.0), vec2(24.0, 24.0)),
        app.icon,
        p.text,
    );
    ui.painter().text(
        rect.min + vec2(44.0, 18.0),
        Align2::LEFT_CENTER,
        &app.name,
        FontId::proportional(13.5),
        p.text,
    );
    if let Action::Launch(_) = app.action {
        ui.painter().text(
            rect.right_center() - vec2(8.0, 0.0),
            Align2::RIGHT_CENTER,
            "Windows program",
            FontId::proportional(11.0),
            p.text_dim,
        );
    }
    response
}

impl eframe::App for StartMenu {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.tick(ctx);
        actions::debug_input(ctx, "start");
        let p = self.theme.theme.palette;
        let mut chosen: Option<Cmd> = None;
        let mut close = false;
        // Clicking anywhere else closes the menu, like Windows.
        match ctx.input(|i| i.viewport().focused) {
            Some(true) => self.was_focused = true,
            Some(false) if self.was_focused => close = true,
            _ if self.opened.elapsed() < Duration::from_secs(1) => {
                ctx.send_viewport_cmd(ViewportCommand::Focus)
            }
            _ => {}
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        egui::TopBottomPanel::bottom("start-footer")
            .frame(
                egui::Frame::NONE
                    .fill(p.surface_alt)
                    .inner_margin(egui::Margin::symmetric(24, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    wc_ui::icons::icon(ui, Icon::User, 28.0);
                    let user = ui.add(
                        egui::Button::new(egui::RichText::new(&self.ns.user).size(13.5))
                            .frame(false),
                    );
                    if user.clicked() {
                        chosen = Some(Cmd::Open(self.ns.home.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let power = widgets::icon_button(ui, Icon::Power, "Power", true);
                        egui::Popup::menu(&power).show(|ui| {
                            for (label, action) in [
                                ("Sign out", Power::SignOut),
                                ("Restart", Power::Restart),
                                ("Shut down", Power::ShutDown),
                            ] {
                                if ui.button(label).clicked() {
                                    chosen = Some(Cmd::Power(action));
                                }
                            }
                        });
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.base).inner_margin(egui::Margin::symmetric(24, 18)).stroke(egui::Stroke::new(1.0, p.border)))
            .show(ctx, |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Type here to search apps")
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(12, 8)),
                );
                if self.focus_search {
                    search.request_focus();
                    self.focus_search = false;
                }
                ui.add_space(14.0);
                let query = self.search.trim().to_lowercase();
                if !query.is_empty() {
                    let matches: Vec<&App> = self.apps.iter().filter(|a| a.name.to_lowercase().contains(&query)).collect();
                    widgets::section_header(ui, "Best match");
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    for (i, app) in matches.iter().take(12).enumerate() {
                        if app_row(ui, app, p).clicked() || (enter && i == 0) {
                            chosen = Some(Cmd::Catalog(app.action.clone()));
                        }
                    }
                    if matches.is_empty() {
                        ui.label(format!("No apps match \"{}\".", self.search.trim()));
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(if self.all_apps { "All apps" } else { "Pinned" }).size(15.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(if self.all_apps { "‹  Back" } else { "All apps  ›" }).clicked() {
                            self.all_apps = !self.all_apps;
                        }
                    });
                });
                ui.add_space(8.0);
                if self.all_apps {
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        let mut letter = None;
                        for app in &self.apps {
                            let first = app.name.chars().next().map(|c| c.to_ascii_uppercase());
                            if first != letter {
                                letter = first;
                                widgets::section_header(ui, &first.unwrap_or('#').to_string());
                            }
                            if app_row(ui, app, p).clicked() {
                                chosen = Some(Cmd::Catalog(app.action.clone()));
                            }
                        }
                    });
                } else {
                    let tile = vec2((ui.available_width() / 4.0).floor() - 2.0, 84.0);
                    egui::Grid::new("pinned").spacing(vec2(2.0, 4.0)).show(ui, |ui| {
                        for (i, app) in self.pinned.iter().enumerate() {
                            if widgets::tile(ui, app.icon, &app.name, tile).clicked() {
                                chosen = Some(Cmd::Catalog(app.action.clone()));
                            }
                            if i % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("Recommended").size(15.0).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Windows programs you install appear in All apps. Portable programs go on drive D:.").color(p.text_dim));
                }
            });
        if let Some(cmd) = chosen {
            actions::execute_or_report(cmd, &self.ns);
            close = true;
        }
        if close {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}
