//! Widgets with the WinCraft look, built on egui primitives.
use crate::{icons::paint_icon, Icon, Palette};
use egui::{vec2, Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, Ui};

fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

/// Square toolbar button with a monochrome glyph (Back, Up, Refresh...).
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(
        vec2(32.0, 30.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && response.hovered() {
        let fill = if response.is_pointer_button_down_on() {
            p.pressed
        } else {
            p.hover
        };
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    }
    let ink = if enabled {
        p.text
    } else {
        p.text_dim.gamma_multiply(0.5)
    };
    paint_icon(ui.painter(), rect.shrink(8.0), icon, ink);
    response.on_hover_text(tooltip)
}

/// Command bar button: colour icon and label (Win11 Explorer "New", "Delete"...).
pub fn command_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let p = palette(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), p.text);
    let size = vec2(34.0 + galley.size().x, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.hover);
    }
    paint_icon(
        ui.painter(),
        egui::Rect::from_min_size(rect.min + vec2(7.0, 6.0), vec2(18.0, 18.0)),
        icon,
        p.text,
    );
    ui.painter().galley(
        rect.min + vec2(30.0, (rect.height() - galley.size().y) / 2.0),
        galley,
        p.text,
    );
    response
}

/// Navigation pane row: icon + label, selected rows get the accent bar.
pub fn nav_item(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(4), p.selection);
        let bar = egui::Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, 14.0));
        painter.rect_filled(bar, CornerRadius::same(2), p.accent);
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(4), p.hover);
    }
    paint_icon(
        painter,
        egui::Rect::from_min_size(rect.min + vec2(10.0, 6.0), vec2(18.0, 18.0)),
        icon,
        p.text,
    );
    painter.text(
        rect.min + vec2(36.0, 15.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        p.text,
    );
    response
}

pub fn section_header(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .size(12.5)
            .strong()
            .color(p.text_dim),
    );
    ui.add_space(2.0);
}

/// Large tile with an icon above a label (Start menu "Pinned", This PC folders).
pub fn tile(ui: &mut Ui, icon: Icon, label: &str, size: egui::Vec2) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(6), p.hover);
    }
    let icon_size = (size.y * 0.45).min(36.0);
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 8.0 + icon_size / 2.0),
        vec2(icon_size, icon_size),
    );
    paint_icon(painter, icon_rect, icon, p.text);
    let galley = painter.layout(
        label.into(),
        FontId::proportional(12.0),
        p.text,
        size.x - 8.0,
    );
    let text_pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        icon_rect.bottom() + 6.0,
    );
    painter.galley(text_pos, galley, p.text);
    response
}

/// Thin horizontal usage meter (drive free space).
pub fn meter(ui: &mut Ui, fraction: f32, width: f32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), p.border);
    let mut filled = rect;
    filled.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    let color = if fraction > 0.9 { p.danger } else { p.accent };
    ui.painter()
        .rect_filled(filled, CornerRadius::same(3), color);
}

/// 1 px separator line in the theme's border colour.
pub fn separator(ui: &mut Ui) {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, p.border),
    );
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit]).replace(".0 ", " ")
}

/// Transparent colour helper for overlays.
pub fn alpha(color: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}
