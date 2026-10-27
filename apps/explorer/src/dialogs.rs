//! Small shell dialogs: Run (Win+R) and the message box.
use crate::actions::{self, ThemeWatch};
use egui::{vec2, ViewportBuilder, ViewportCommand};
use std::process::Command;
use wc_paths::Namespace;

/// `explorer --run-dialog`. The compositor puts it in the bottom-left corner, as Windows does.
pub fn run_dialog() -> Result<(), String> {
    let viewport = ViewportBuilder::default()
        .with_title("Run")
        .with_inner_size([440.0, 175.0])
        .with_resizable(false);
    actions::run_window(viewport, |_| RunDialog {
        ns: Namespace::from_env(),
        theme: ThemeWatch::new(),
        text: String::new(),
        error: None,
        focused: false,
    })
}

struct RunDialog {
    ns: Namespace,
    theme: ThemeWatch,
    text: String,
    error: Option<String>,
    focused: bool,
}

impl RunDialog {
    /// Built-in names (notepad, cmd, regedit...), Windows/Unix paths, then programs on PATH.
    fn run(&self, input: &str) -> Result<(), String> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(());
        }
        if let Some(mut command) = wc_launch::run_command(input) {
            return command
                .spawn()
                .map(actions::detach)
                .map_err(|e| e.to_string());
        }
        if let Some(path) = self.ns.to_unix(input).filter(|p| p.exists()) {
            return actions::execute(actions::Cmd::Open(path), &self.ns);
        }
        let mut words = input.split_whitespace();
        let program = words.next().unwrap_or_default();
        let on_path = std::env::var_os("PATH")
            .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()));
        if on_path {
            return Command::new(program)
                .args(words)
                .spawn()
                .map(actions::detach)
                .map_err(|e| e.to_string());
        }
        Err(format!("WinCraft cannot find '{input}'. Make sure you typed the name correctly, and then try again."))
    }
}

impl eframe::App for RunDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.tick(ctx);
        let mut submit = false;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                wc_ui::icons::icon(ui, wc_ui::Icon::Run, 32.0);
                // Wrapped: in a horizontal row egui would otherwise widen the layout past the window.
                ui.add(egui::Label::new("Type the name of a program, folder, document, or Windows path, and WinCraft will open it for you.").wrap());
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Open:");
                let edit = ui.add(egui::TextEdit::singleline(&mut self.text).desired_width(f32::INFINITY));
                if !self.focused {
                    edit.request_focus();
                    self.focused = true;
                }
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
            if let Some(error) = &self.error {
                ui.colored_label(self.theme.theme.palette.danger, error);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                close |= ui.button("Cancel").clicked();
                submit |= ui.button("   OK   ").clicked();
            });
        });
        if submit {
            match self.run(&self.text.clone()) {
                Ok(()) => close = true,
                Err(e) => self.error = Some(e),
            }
        }
        if close {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

/// `explorer --message <text>`: an information box for errors from windowless actions.
pub fn message(text: String) -> Result<(), String> {
    // Centred on the screen by the compositor's window rule.
    let viewport = ViewportBuilder::default()
        .with_title("WinCraft")
        .with_inner_size([420.0, 170.0]);
    actions::run_window(viewport, move |_| MessageBox {
        text,
        theme: ThemeWatch::new(),
    })
}

struct MessageBox {
    text: String,
    theme: ThemeWatch,
}

impl eframe::App for MessageBox {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.tick(ctx);
        let accent = self.theme.theme.palette.accent;
        let mut close =
            ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
        egui::TopBottomPanel::bottom("buttons").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close |= ui.add(egui::Button::new("   OK   ").fill(accent)).clicked();
            });
            ui.add_space(6.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 15.0, accent);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "i",
                    egui::FontId::proportional(20.0),
                    egui::Color32::WHITE,
                );
                ui.add(egui::Label::new(&self.text).wrap());
            });
        });
        if close {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}
