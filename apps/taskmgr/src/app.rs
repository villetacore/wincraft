use crate::procfs::{Process, Sampler};
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Rect, Sense, Stroke};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use wc_ui::{widgets, Icon, Palette};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Processes,
    Performance,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Name,
    Cpu,
    Memory,
}

pub struct TaskManager {
    sampler: Sampler,
    processes: Vec<Process>,
    cpu_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
    memory: (u64, u64),
    last: Instant,
    tab: Tab,
    sort: Sort,
    selected: Option<u32>,
    error: Option<String>,
}

impl Default for TaskManager {
    fn default() -> Self {
        let mut sampler = Sampler::default();
        let first = sampler.sample();
        Self {
            sampler,
            processes: first.processes,
            cpu_history: VecDeque::from(vec![0.0; 60]),
            memory_history: VecDeque::from(vec![0.0; 60]),
            memory: (first.memory_used, first.memory_total),
            last: Instant::now(),
            tab: Tab::Processes,
            sort: Sort::Cpu,
            selected: None,
            error: None,
        }
    }
}

impl TaskManager {
    fn refresh(&mut self) {
        let snapshot = self.sampler.sample();
        self.processes = snapshot.processes;
        self.memory = (snapshot.memory_used, snapshot.memory_total);
        for (history, value) in [
            (&mut self.cpu_history, snapshot.cpu),
            (
                &mut self.memory_history,
                snapshot.memory_used as f32 * 100.0 / snapshot.memory_total.max(1) as f32,
            ),
        ] {
            history.push_back(value);
            if history.len() > 60 {
                history.pop_front();
            }
        }
        self.last = Instant::now();
    }

    fn end_task(&mut self, pid: u32) {
        // SAFETY: plain signal delivery; the kernel enforces permissions.
        if unsafe { libc::kill(pid as i32, libc::SIGTERM) } != 0 {
            let e = std::io::Error::last_os_error();
            self.error = Some(format!(
                "Unable to end process {pid}: {e} (system processes are protected)"
            ));
        } else {
            self.error = None;
        }
    }

    fn processes_tab(&mut self, ui: &mut egui::Ui, p: Palette) {
        let mut list = self.processes.clone();
        match self.sort {
            Sort::Name => list.sort_by_key(|p| p.name.to_lowercase()),
            Sort::Cpu => list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            Sort::Memory => list.sort_by(|a, b| b.memory.cmp(&a.memory)),
        }
        let width = ui.available_width();
        let cols = [
            ("Name", 0.0, Some(Sort::Name)),
            ("PID", 0.42, None),
            ("User", 0.52, None),
            ("CPU", 0.70, Some(Sort::Cpu)),
            ("Memory", 0.84, Some(Sort::Memory)),
        ];
        let (header, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
        for (label, at, sort) in cols {
            let rect = Rect::from_min_size(
                pos2(header.left() + width * at, header.top()),
                vec2(width * 0.14, 26.0),
            );
            let r = ui.interact(rect, ui.id().with(label), Sense::click());
            let active = sort == Some(self.sort);
            ui.painter().text(
                rect.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.5),
                if active { p.accent } else { p.text_dim },
            );
            if let (true, Some(sort)) = (r.clicked(), sort) {
                self.sort = sort;
            }
        }
        widgets::separator(ui);
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            24.0,
            list.len(),
            |ui, range| {
                for process in &list[range] {
                    let (rect, r) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
                    if self.selected == Some(process.pid) {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), p.selection);
                    } else if r.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), p.hover);
                    }
                    // CPU heat, like Windows tints busy cells.
                    let heat = (process.cpu / 50.0).clamp(0.0, 1.0);
                    if heat > 0.02 {
                        let cell = Rect::from_min_size(
                            pos2(rect.left() + width * 0.70, rect.top()),
                            vec2(width * 0.14, 24.0),
                        );
                        ui.painter().rect_filled(
                            cell,
                            CornerRadius::ZERO,
                            widgets::alpha(
                                egui::Color32::from_rgb(0xFF, 0xB9, 0x00),
                                (heat * 120.0) as u8,
                            ),
                        );
                    }
                    let icon = if process.windows {
                        Icon::Application
                    } else {
                        Icon::Terminal
                    };
                    wc_ui::paint_icon(
                        ui.painter(),
                        Rect::from_min_size(rect.min + vec2(4.0, 3.0), vec2(18.0, 18.0)),
                        icon,
                        p.text,
                    );
                    let at = |f: f32| rect.left() + width * f + 4.0;
                    let painter = ui.painter();
                    painter
                        .with_clip_rect(Rect::from_min_max(rect.min, pos2(at(0.41), rect.bottom())))
                        .text(
                            pos2(rect.left() + 28.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &process.name,
                            FontId::proportional(13.0),
                            p.text,
                        );
                    painter.text(
                        pos2(at(0.42), rect.center().y),
                        Align2::LEFT_CENTER,
                        process.pid.to_string(),
                        FontId::proportional(12.5),
                        p.text_dim,
                    );
                    painter.text(
                        pos2(at(0.52), rect.center().y),
                        Align2::LEFT_CENTER,
                        &process.user,
                        FontId::proportional(12.5),
                        p.text_dim,
                    );
                    painter.text(
                        pos2(at(0.70), rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:.1}%", process.cpu),
                        FontId::proportional(12.5),
                        p.text,
                    );
                    painter.text(
                        pos2(at(0.84), rect.center().y),
                        Align2::LEFT_CENTER,
                        widgets::human_size(process.memory),
                        FontId::proportional(12.5),
                        p.text,
                    );
                    if r.clicked() {
                        self.selected = Some(process.pid);
                    }
                    let pid = process.pid;
                    r.context_menu(|ui| {
                        if ui.button("End task").clicked() {
                            self.end_task(pid);
                            ui.close();
                        }
                    });
                }
            },
        );
    }

    fn graph(
        ui: &mut egui::Ui,
        p: Palette,
        title: &str,
        detail: String,
        history: &VecDeque<f32>,
        color: egui::Color32,
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).size(18.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(detail)
            });
        });
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(4), p.surface);
        for i in 1..10 {
            let x = rect.left() + rect.width() * i as f32 / 10.0;
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(1.0, p.border),
            );
        }
        let points: Vec<egui::Pos2> = history
            .iter()
            .enumerate()
            .map(|(i, v)| {
                pos2(
                    rect.left() + rect.width() * i as f32 / (history.len().max(2) - 1) as f32,
                    rect.bottom() - rect.height() * (v / 100.0).clamp(0.0, 1.0),
                )
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(2.0, color)));
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        ui.add_space(12.0);
    }
}

impl eframe::App for TaskManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last.elapsed() >= Duration::from_secs(1) {
            self.refresh();
        }
        let p = if ctx.style().visuals.dark_mode {
            Palette::DARK
        } else {
            Palette::LIGHT
        };
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (Tab::Processes, "Processes"),
                    (Tab::Performance, "Performance"),
                ] {
                    if ui.selectable_label(self.tab == tab, label).clicked() {
                        self.tab = tab;
                    }
                }
            });
        });
        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} processes", self.processes.len()));
                if let Some(error) = &self.error {
                    ui.colored_label(p.danger, error);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("End task"))
                        .clicked()
                    {
                        if let Some(pid) = self.selected {
                            self.end_task(pid);
                        }
                    }
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Processes => self.processes_tab(ui, p),
            Tab::Performance => {
                let cpu = self.cpu_history.back().copied().unwrap_or(0.0);
                Self::graph(
                    ui,
                    p,
                    "CPU",
                    format!("{cpu:.0}%"),
                    &self.cpu_history,
                    p.accent,
                );
                let (used, total) = self.memory;
                Self::graph(
                    ui,
                    p,
                    "Memory",
                    format!(
                        "{} / {}",
                        widgets::human_size(used),
                        widgets::human_size(total)
                    ),
                    &self.memory_history,
                    egui::Color32::from_rgb(0x8B, 0x5C, 0xF6),
                );
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
