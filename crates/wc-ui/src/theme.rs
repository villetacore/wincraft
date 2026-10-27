use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Window background ("mica").
    pub base: Color32,
    /// Cards, lists, text fields.
    pub surface: Color32,
    /// Buttons at rest, alternate rows.
    pub surface_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selection: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub taskbar: Color32,
    pub danger: Color32,
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        base: Color32::from_rgb(0xF3, 0xF3, 0xF3),
        surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        surface_alt: Color32::from_rgb(0xFB, 0xFB, 0xFB),
        border: Color32::from_rgb(0xE0, 0xE0, 0xE0),
        text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
        text_dim: Color32::from_rgb(0x5E, 0x5E, 0x5E),
        hover: Color32::from_rgb(0xEA, 0xEA, 0xEA),
        pressed: Color32::from_rgb(0xDD, 0xDD, 0xDD),
        selection: Color32::from_rgb(0xCC, 0xE4, 0xF7),
        accent: Color32::from_rgb(0x00, 0x67, 0xC0),
        accent_text: Color32::WHITE,
        taskbar: Color32::from_rgb(0xEE, 0xF1, 0xF5),
        danger: Color32::from_rgb(0xC4, 0x2B, 0x1C),
    };
    pub const DARK: Palette = Palette {
        base: Color32::from_rgb(0x20, 0x20, 0x20),
        surface: Color32::from_rgb(0x2B, 0x2B, 0x2B),
        surface_alt: Color32::from_rgb(0x32, 0x32, 0x32),
        border: Color32::from_rgb(0x3D, 0x3D, 0x3D),
        text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        text_dim: Color32::from_rgb(0xC5, 0xC5, 0xC5),
        hover: Color32::from_rgb(0x3A, 0x3A, 0x3A),
        pressed: Color32::from_rgb(0x45, 0x45, 0x45),
        selection: Color32::from_rgb(0x0F, 0x3F, 0x66),
        accent: Color32::from_rgb(0x4C, 0xC2, 0xFF),
        accent_text: Color32::BLACK,
        taskbar: Color32::from_rgb(0x1C, 0x1C, 0x1C),
        danger: Color32::from_rgb(0xFF, 0x99, 0xA4),
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub palette: Palette,
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            palette: Palette::LIGHT,
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            palette: Palette::DARK,
        }
    }

    /// The user's choice, stored by Settings in `~/.config/wincraft/theme`.
    pub fn load() -> Self {
        let dark = std::env::var_os("HOME")
            .map(|h| std::path::Path::new(&h).join(".config/wincraft/theme"))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|s| s.trim() == "dark");
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let home = std::env::var_os("HOME").unwrap_or_default();
        let dir = std::path::Path::new(&home).join(".config/wincraft");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("theme"), if self.dark { "dark" } else { "light" })
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let p = self.palette;
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = p.base;
        visuals.window_fill = p.surface;
        visuals.extreme_bg_color = p.surface;
        visuals.faint_bg_color = p.surface_alt;
        visuals.code_bg_color = p.surface_alt;
        visuals.window_stroke = Stroke::new(1.0, p.border);
        visuals.window_corner_radius = CornerRadius::same(8);
        visuals.menu_corner_radius = CornerRadius::same(6);
        visuals.selection.bg_fill = p.selection;
        visuals.selection.stroke = Stroke::new(1.0, p.accent);
        visuals.hyperlink_color = p.accent;
        visuals.override_text_color = Some(p.text);
        let w = &mut visuals.widgets;
        let radius = CornerRadius::same(4);
        for (state, fill) in [
            (&mut w.inactive, p.surface_alt),
            (&mut w.hovered, p.hover),
            (&mut w.active, p.pressed),
            (&mut w.open, p.hover),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.corner_radius = radius;
            state.fg_stroke = Stroke::new(1.0, p.text);
            state.bg_stroke = Stroke::new(1.0, p.border);
            state.expansion = 0.0;
        }
        w.noninteractive.bg_fill = p.base;
        w.noninteractive.weak_bg_fill = p.base;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        w.noninteractive.corner_radius = radius;
        w.hovered.bg_stroke = Stroke::new(1.0, p.border);
        ctx.set_visuals(visuals);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(12.0, 5.0);
            s.spacing.interact_size.y = 28.0;
            s.spacing.window_margin = egui::Margin::same(12);
            s.text_styles = [
                (TextStyle::Small, FontId::proportional(11.5)),
                (TextStyle::Body, FontId::proportional(13.5)),
                (TextStyle::Button, FontId::proportional(13.5)),
                (TextStyle::Heading, FontId::proportional(20.0)),
                (TextStyle::Monospace, FontId::monospace(13.5)),
            ]
            .into();
        });
    }
}

/// System fonts (DejaVu: Latin + Cyrillic) take priority over egui's built-in fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, family, candidates) in [
        (
            "sans",
            FontFamily::Proportional,
            [
                "/usr/share/fonts/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
        ),
        (
            "mono",
            FontFamily::Monospace,
            [
                "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ],
        ),
    ] {
        if let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
            fonts
                .font_data
                .insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);
}
