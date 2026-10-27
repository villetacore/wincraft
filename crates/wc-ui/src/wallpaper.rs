//! Procedural default wallpaper: deep blue gradient with a soft "bloom" of light.
use egui::{epaint::Vertex, pos2, Color32, Mesh, Painter, Rect, Shape};

fn gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.vertices.push(Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    painter.add(Shape::mesh(mesh));
}

pub fn paint(painter: &Painter, rect: Rect, dark: bool) {
    let (top, bottom) = if dark {
        (
            Color32::from_rgb(0x06, 0x12, 0x2B),
            Color32::from_rgb(0x0C, 0x2E, 0x5C),
        )
    } else {
        (
            Color32::from_rgb(0x0A, 0x3A, 0x7A),
            Color32::from_rgb(0x5B, 0x9B, 0xD8),
        )
    };
    gradient(painter, rect, top, bottom);
    // Bloom: layered translucent petals around a bright core, right of centre.
    let center = pos2(
        rect.left() + rect.width() * 0.62,
        rect.top() + rect.height() * 0.55,
    );
    let unit = rect.height() * 0.42;
    for i in 0..6 {
        let angle = -1.1 + i as f32 * 0.42;
        let offset = egui::vec2(angle.cos(), angle.sin()) * unit * 0.35;
        for k in 0..10 {
            let r = unit * (0.75 - k as f32 * 0.06);
            let alpha = 6 + k * 2;
            painter.circle_filled(
                center + offset * (1.0 - k as f32 * 0.05),
                r,
                Color32::from_rgba_unmultiplied(0x9F, 0xD8, 0xFF, alpha as u8),
            );
        }
    }
    for k in 0..12 {
        painter.circle_filled(
            center,
            unit * (0.34 - k as f32 * 0.025),
            Color32::from_rgba_unmultiplied(0xE6, 0xF5, 0xFF, (10 + k * 3) as u8),
        );
    }
}
