// Archived first Windows-only prototype (not compiled).
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Align, Area, Button, Color32, Context, CornerRadius, FontId, Frame, Id, Layout, Margin,
    Pos2, RichText, ScrollArea, Stroke, TextEdit, Vec2, ViewportCommand,
};

#[derive(Clone)]
struct RecentApp {
    name: String,
    path: PathBuf,
    started: String,
}

struct WinCraft {
    start_open: bool,
    launcher_open: bool,
    about_open: bool,
    exe_path: String,
    search: String,
    recent: Vec<RecentApp>,
    status: String,
    clock: String,
    last_clock: Instant,
}

impl Default for WinCraft {
    fn default() -> Self {
        Self {
            start_open: false,
            launcher_open: false,
            about_open: false,
            exe_path: String::new(),
            search: String::new(),
            recent: Vec::new(),
            status: "Готово".into(),
            clock: String::new(),
            last_clock: Instant::now(),
        }
    }
}

impl WinCraft {
    fn launch_exe(&mut self, path: PathBuf) {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Приложение")
            .to_string();
        match Command::new(&path).spawn() {
            Ok(_) => {
                self.status = format!("Запущено: {name}");
                self.recent.retain(|app| app.path != path);
                self.recent.insert(
                    0,
                    RecentApp {
                        name,
                        path,
                        started: "только что".into(),
                    },
                );
                self.start_open = false;
                self.launcher_open = false;
            }
            Err(error) => self.status = format!("Не удалось запустить: {error}"),
        }
    }

    fn pick_exe(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows applications", &["exe"])
            .set_title("Выберите EXE-файл")
            .pick_file()
        {
            self.exe_path = path.display().to_string();
            self.launch_exe(path);
        }
    }

    fn top_bar(&mut self, ctx: &Context) {
        Area::new(Id::new("top_bar"))
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(Color32::from_rgb(18, 28, 48))
                    .inner_margin(Margin::symmetric(20, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("◈  WinCraft")
                                    .font(FontId::proportional(19.0))
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.add_space(18.0);
                            if ui
                                .add(
                                    Button::new(RichText::new("⊞  Пуск").size(15.0))
                                        .corner_radius(8.0),
                                )
                                .clicked()
                            {
                                self.start_open = !self.start_open;
                            }
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(&self.status).color(Color32::from_rgb(178, 197, 224)),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(Button::new("⏻").corner_radius(8.0)).clicked() {
                                    ctx.send_viewport_cmd(ViewportCommand::Close);
                                }
                                ui.label(RichText::new(&self.clock).color(Color32::WHITE));
                            });
                        });
                    });
            });
    }

    fn desktop(&mut self, ctx: &Context) {
        Area::new(Id::new("desktop"))
            .fixed_pos(Pos2::new(0.0, 60.0))
            .show(ctx, |ui| {
                ui.set_min_size(ctx.screen_rect().size() - Vec2::new(0.0, 120.0));
                ui.vertical(|ui| {
                    ui.add_space(26.0);
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        if ui
                            .add(
                                Button::new(RichText::new("▣\nЭтот компьютер").size(14.0))
                                    .min_size(Vec2::new(120.0, 70.0)),
                            )
                            .clicked()
                        {
                            self.status = "Файловый менеджер пока доступен через выбор EXE".into();
                        }
                        if ui
                            .add(
                                Button::new(RichText::new("◎\nКорзина").size(14.0))
                                    .min_size(Vec2::new(100.0, 70.0)),
                            )
                            .clicked()
                        {
                            self.status = "Корзина пуста".into();
                        }
                    });
                });
            });
    }

    fn taskbar(&mut self, ctx: &Context) {
        Area::new(Id::new("taskbar"))
            .anchor(egui::Align2::LEFT_BOTTOM, Vec2::ZERO)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(Color32::from_rgb(24, 36, 61))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(62, 83, 117)))
                    .inner_margin(Margin::symmetric(18, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(Button::new(RichText::new("⊞").size(22.0)).corner_radius(8.0))
                                .clicked()
                            {
                                self.start_open = !self.start_open;
                            }
                            if ui
                                .add(Button::new("▶  Запустить EXE").corner_radius(8.0))
                                .clicked()
                            {
                                self.pick_exe();
                            }
                            if ui.add(Button::new("⌕  Поиск").corner_radius(8.0)).clicked() {
                                self.launcher_open = true;
                            }
                            if ui.add(Button::new("ⓘ").corner_radius(8.0)).clicked() {
                                self.about_open = true;
                            }
                        });
                    });
            });
    }

    fn start_menu(&mut self, ctx: &Context) {
        if !self.start_open {
            return;
        }
        Area::new(Id::new("start_menu"))
            .fixed_pos(Pos2::new(24.0, 70.0))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(Color32::from_rgb(31, 45, 74))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(91, 120, 166)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(18))
                    .show(ui, |ui| {
                        ui.set_width(390.0);
                        ui.label(
                            RichText::new("Добро пожаловать")
                                .size(23.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new("Приложения и недавние файлы")
                                .color(Color32::from_rgb(179, 199, 225)),
                        );
                        ui.add_space(12.0);
                        if ui
                            .add(
                                Button::new("▶  Запустить EXE-файл")
                                    .min_size(Vec2::new(350.0, 38.0)),
                            )
                            .clicked()
                        {
                            self.pick_exe();
                        }
                        if ui
                            .add(Button::new("▤  Открыть лаунчер").min_size(Vec2::new(350.0, 38.0)))
                            .clicked()
                        {
                            self.launcher_open = true;
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Недавние приложения")
                                .strong()
                                .color(Color32::WHITE),
                        );
                        if self.recent.is_empty() {
                            ui.label("Пока ничего не запускалось");
                        } else {
                            let recent = self.recent.clone();
                            ScrollArea::vertical().max_height(145.0).show(ui, |ui| {
                                for app in recent {
                                    if ui
                                        .add(
                                            Button::new(format!("{}\n{}", app.name, app.started))
                                                .min_size(Vec2::new(350.0, 42.0)),
                                        )
                                        .clicked()
                                    {
                                        self.launch_exe(app.path);
                                    }
                                }
                            });
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("WinCraft 0.1 • Rust desktop shell")
                                .small()
                                .color(Color32::from_rgb(150, 172, 205)),
                        );
                    });
            });
    }

    fn launcher(&mut self, ctx: &Context) {
        if !self.launcher_open {
            return;
        }
        Area::new(Id::new("launcher"))
            .fixed_pos(Pos2::new(180.0, 130.0))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(Color32::from_rgb(27, 40, 66))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(91, 120, 166)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(18))
                    .show(ui, |ui| {
                        ui.set_width(520.0);
                        ui.horizontal(|ui| {
                            ui.label("⌕");
                            ui.add(
                                TextEdit::singleline(&mut self.search)
                                    .hint_text("Найти приложение или EXE...")
                                    .desired_width(450.0),
                            );
                        });
                        ui.add_space(14.0);
                        if ui
                            .add(
                                Button::new("▶  Выбрать EXE через проводник")
                                    .min_size(Vec2::new(480.0, 40.0)),
                            )
                            .clicked()
                        {
                            self.pick_exe();
                        }
                        if !self.exe_path.is_empty() {
                            ui.label(
                                RichText::new(&self.exe_path)
                                    .small()
                                    .color(Color32::from_rgb(174, 197, 226)),
                            );
                        }
                        ui.add_space(8.0);
                        if ui.button("Закрыть").clicked() {
                            self.launcher_open = false;
                        }
                    });
            });
    }

    fn about(&mut self, ctx: &Context) {
        if !self.about_open {
            return;
        }
        egui::Window::new("О WinCraft")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new("WinCraft").size(25.0).strong());
                ui.label("Современная графическая оболочка на Rust.");
                ui.label("Запуск EXE выполняется средствами Windows через std::process::Command.");
                if ui.button("Закрыть").clicked() {
                    self.about_open = false;
                }
            });
    }
}

impl eframe::App for WinCraft {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals {
            dark_mode: true,
            panel_fill: Color32::from_rgb(13, 22, 40),
            ..Default::default()
        });
        if self.last_clock.elapsed() > Duration::from_secs(1) {
            self.clock = chrono_like_time();
            self.last_clock = Instant::now();
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(13, 22, 40)))
            .show(ctx, |_| {});
        self.desktop(ctx);
        self.top_bar(ctx);
        self.taskbar(ctx);
        self.start_menu(ctx);
        self.launcher(ctx);
        self.about(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn chrono_like_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86_400;
    format!(
        "{:02}:{:02}",
        (seconds / 3600 + 3) % 24,
        (seconds / 60) % 60
    )
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("WinCraft")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "WinCraft",
        options,
        Box::new(|_cc| Ok(Box::new(WinCraft::default()))),
    )
}
