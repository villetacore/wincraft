//! Settings: personalization, power and system information.
use crate::{
    actions::{self, Cmd, ThemeWatch},
    system::{self, Power},
};
use egui::{vec2, ViewportBuilder};
use wc_paths::Namespace;
use wc_ui::{widgets, Icon, Theme};

/// `explorer --settings-window`.
pub fn run() -> Result<(), String> {
    let viewport = ViewportBuilder::default()
        .with_title("Settings")
        .with_inner_size([820.0, 540.0])
        .with_min_inner_size([560.0, 380.0]);
    actions::run_window(viewport, |_| SettingsApp {
        settings: Settings::default(),
        theme: ThemeWatch::new(),
        ns: Namespace::from_env(),
        error: None,
    })
}

struct SettingsApp {
    settings: Settings,
    theme: ThemeWatch,
    ns: Namespace,
    error: Option<String>,
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.tick(ctx);
        for cmd in self.settings.ui(ctx, &self.theme.theme) {
            if let Cmd::SetTheme(theme) = &cmd {
                // Apply here at once; other shell windows follow the saved file.
                self.theme.theme = *theme;
            }
            if let Err(e) = actions::execute(cmd, &self.ns) {
                self.error = Some(e);
            }
        }
        if let Some(error) = self.error.clone() {
            egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
                ui.label(error);
                if ui.button("OK").clicked() {
                    self.error = None;
                }
            });
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Personalization,
    Power,
    About,
}

pub struct Settings {
    page: Page,
    info: Option<system::SystemInfo>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            page: Page::Personalization,
            info: None,
        }
    }
}

impl Settings {
    /// Draws the window; returns requests for the shell.
    pub fn ui(&mut self, ctx: &egui::Context, theme: &Theme) -> Vec<Cmd> {
        let mut cmds = vec![];
        {
            let p = theme.palette;
            egui::SidePanel::left("settings-nav")
                .resizable(false)
                .exact_width(230.0)
                .frame(
                    egui::Frame::NONE
                        .fill(p.base)
                        .inner_margin(egui::Margin::same(12)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        wc_ui::icons::icon(ui, Icon::User, 40.0);
                        ui.vertical(|ui| {
                            ui.strong(std::env::var("USER").unwrap_or_else(|_| "user".into()));
                            ui.label(
                                egui::RichText::new("Local account")
                                    .color(p.text_dim)
                                    .size(12.0),
                            );
                        });
                    });
                    ui.add_space(14.0);
                    for (page, icon, label) in [
                        (Page::Personalization, Icon::Settings, "Personalization"),
                        (Page::Power, Icon::Power, "Power"),
                        (Page::About, Icon::ThisPc, "About"),
                    ] {
                        if widgets::nav_item(ui, icon, label, self.page == page).clicked() {
                            self.page = page;
                        }
                    }
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(p.base).inner_margin(egui::Margin::symmetric(28, 20)))
                .show(ctx, |ui| match self.page {
                    Page::Personalization => {
                        ui.heading("Personalization");
                        ui.add_space(16.0);
                        card(ui, theme, |ui| {
                            ui.strong("Choose your mode");
                            ui.label(egui::RichText::new("Changes the colors of the shell and WinCraft apps.").color(p.text_dim));
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                for (dark, label) in [(false, "Light"), (true, "Dark")] {
                                    if ui.selectable_label(theme.dark == dark, label).clicked() && theme.dark != dark {
                                        cmds.push(Cmd::SetTheme(if dark { Theme::dark() } else { Theme::light() }));
                                    }
                                }
                            });
                        });
                        ui.add_space(10.0);
                        card(ui, theme, |ui| {
                            ui.strong("Window behaviour");
                            ui.label("Alt+F4 closes, Alt+Tab switches, Win+D shows the desktop, Win+E opens File Explorer,");
                            ui.label("Win+R opens Run, Win+Left/Right snaps, Ctrl+Shift+Esc opens Task Manager.");
                        });
                    }
                    Page::Power => {
                        ui.heading("Power");
                        ui.add_space(16.0);
                        card(ui, theme, |ui| {
                            ui.label("Unsaved work in open programs is lost when the session ends.");
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                for (label, action) in [("Sign out", Power::SignOut), ("Restart", Power::Restart), ("Shut down", Power::ShutDown)] {
                                    if ui.button(label).clicked() {
                                        cmds.push(Cmd::Power(action));
                                    }
                                }
                            });
                        });
                    }
                    Page::About => {
                        ui.heading("About");
                        ui.add_space(16.0);
                        let info = self.info.get_or_insert_with(system::system_info);
                        card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                wc_ui::icons::icon(ui, Icon::Logo, 48.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&info.os).size(18.0).strong());
                                    ui.label(egui::RichText::new("Windows-like desktop on a vanilla Linux kernel").color(p.text_dim));
                                });
                            });
                            ui.add_space(10.0);
                            egui::Grid::new("about").num_columns(2).spacing(vec2(24.0, 6.0)).show(ui, |ui| {
                                for (k, v) in [
                                    ("Kernel", &info.kernel),
                                    ("Processor", &info.cpu),
                                    ("Installed RAM", &info.memory),
                                    ("Win32 subsystem", &info.wine),
                                ] {
                                    ui.label(egui::RichText::new(k).color(p.text_dim));
                                    ui.label(v);
                                    ui.end_row();
                                }
                            });
                        });
                    }
                });
        }
        cmds
    }
}

fn card(ui: &mut egui::Ui, theme: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(theme.palette.surface)
        .stroke(egui::Stroke::new(1.0, theme.palette.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}
