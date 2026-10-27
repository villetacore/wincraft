//! Vector icons on a 32×32 design grid, scaled to any rectangle.
use egui::{epaint::PathShape, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Logo,
    Folder,
    FolderDesktop,
    FolderDocuments,
    FolderDownloads,
    FolderMusic,
    FolderPictures,
    FolderVideos,
    File,
    TextFile,
    Application,
    Installer,
    Shortcut,
    Drive,
    SystemDrive,
    ThisPc,
    RecycleBin,
    Terminal,
    Notepad,
    TaskManager,
    Settings,
    Power,
    Restart,
    User,
    Search,
    Back,
    Forward,
    Up,
    Refresh,
    NewFolder,
    Delete,
    Run,
    Wine,
    Browser,
    Archive,
}

const FOLDER_BACK: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x1E);
const FOLDER_FRONT: Color32 = Color32::from_rgb(0xFF, 0xCB, 0x45);
const FOLDER_SHINE: Color32 = Color32::from_rgb(0xFF, 0xDA, 0x73);
const BLUE: Color32 = Color32::from_rgb(0x00, 0x67, 0xC0);
const BLUE_LIGHT: Color32 = Color32::from_rgb(0x4C, 0xC2, 0xFF);
const BLUE_DARK: Color32 = Color32::from_rgb(0x00, 0x3E, 0x92);
const PAPER: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const PAPER_EDGE: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
const GRAY: Color32 = Color32::from_rgb(0x7A, 0x7A, 0x7A);
const DARK: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const GREEN: Color32 = Color32::from_rgb(0x10, 0x9F, 0x4F);

struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    s: f32,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x * self.s, y * self.s)
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: Color32) {
        let rect = Rect::from_min_size(self.p(x, y), vec2(w * self.s, h * self.s));
        self.painter
            .rect_filled(rect, CornerRadius::same((r * self.s).round() as u8), fill);
    }
    fn outline(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color32, width: f32) {
        let rect = Rect::from_min_size(self.p(x, y), vec2(w * self.s, h * self.s));
        self.painter.rect_stroke(
            rect,
            CornerRadius::same((r * self.s).round() as u8),
            Stroke::new(width * self.s, color),
            egui::StrokeKind::Inside,
        );
    }
    fn line(&self, points: &[(f32, f32)], color: Color32, width: f32) {
        let pts = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::line(pts, Stroke::new(width * self.s, color)));
    }
    fn poly(&self, points: &[(f32, f32)], fill: Color32, stroke: Option<Color32>) {
        let pts = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        let stroke = stroke.map_or(Stroke::NONE, |c| Stroke::new(self.s, c));
        self.painter
            .add(PathShape::convex_polygon(pts, fill, stroke));
    }
    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.painter.circle_filled(self.p(x, y), r * self.s, fill);
    }
    fn ring(&self, x: f32, y: f32, r: f32, color: Color32, width: f32) {
        self.painter
            .circle_stroke(self.p(x, y), r * self.s, Stroke::new(width * self.s, color));
    }
}

/// Paints `icon` centred in `rect`. `ink` colours monochrome glyphs (toolbar icons).
pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, ink: Color32) {
    let side = rect.width().min(rect.height());
    let origin = rect.center() - vec2(side, side) / 2.0;
    let pen = Pen {
        painter,
        origin,
        s: side / 32.0,
    };
    match icon {
        Icon::Logo => logo(&pen),
        Icon::Folder => folder(&pen),
        Icon::FolderDesktop => {
            folder(&pen);
            pen.rect(10.0, 15.0, 12.0, 8.0, 1.0, BLUE);
            pen.rect(14.0, 23.0, 4.0, 2.0, 0.0, BLUE_DARK);
        }
        Icon::FolderDocuments => {
            folder(&pen);
            pen.rect(11.0, 13.0, 10.0, 12.0, 1.0, PAPER);
            for y in [16.0, 19.0, 22.0] {
                pen.line(&[(13.0, y), (19.0, y)], BLUE, 1.2);
            }
        }
        Icon::FolderDownloads => {
            folder(&pen);
            pen.line(&[(16.0, 13.0), (16.0, 22.0)], BLUE, 2.4);
            pen.line(&[(12.0, 18.5), (16.0, 22.5), (20.0, 18.5)], BLUE, 2.4);
        }
        Icon::FolderMusic => {
            folder(&pen);
            pen.line(
                &[(14.0, 22.0), (14.0, 14.0), (21.0, 13.0), (21.0, 21.0)],
                BLUE,
                1.8,
            );
            pen.circle(12.8, 22.0, 2.0, BLUE);
            pen.circle(19.8, 21.0, 2.0, BLUE);
        }
        Icon::FolderPictures => {
            folder(&pen);
            pen.rect(10.0, 13.0, 12.0, 10.0, 1.0, PAPER);
            pen.poly(
                &[(11.0, 22.0), (15.0, 17.0), (18.0, 20.0), (21.0, 22.0)],
                GREEN,
                None,
            );
            pen.circle(19.0, 16.0, 1.4, FOLDER_BACK);
        }
        Icon::FolderVideos => {
            folder(&pen);
            pen.rect(10.0, 14.0, 12.0, 9.0, 1.5, BLUE);
            pen.poly(&[(14.5, 16.0), (19.0, 18.5), (14.5, 21.0)], PAPER, None);
        }
        Icon::File => page(&pen),
        Icon::TextFile => {
            page(&pen);
            for y in [13.0, 16.5, 20.0, 23.5] {
                pen.line(&[(10.0, y), (22.0, y)], GRAY, 1.1);
            }
        }
        Icon::Application => app_window(&pen, BLUE),
        Icon::Wine => {
            app_window(&pen, Color32::from_rgb(0x8E, 0x1B, 0x3B));
            pen.circle(16.0, 18.0, 3.5, Color32::from_rgb(0x8E, 0x1B, 0x3B));
        }
        Icon::Browser => {
            // A globe: the web.
            pen.circle(16.0, 16.0, 12.0, BLUE);
            pen.ring(16.0, 16.0, 12.0, BLUE_LIGHT, 1.2);
            pen.line(&[(4.5, 16.0), (27.5, 16.0)], BLUE_LIGHT, 1.4);
            pen.line(&[(6.5, 10.0), (25.5, 10.0)], BLUE_LIGHT, 1.1);
            pen.line(&[(6.5, 22.0), (25.5, 22.0)], BLUE_LIGHT, 1.1);
            pen.line(
                &[
                    (16.0, 4.0),
                    (12.0, 10.0),
                    (11.0, 16.0),
                    (12.0, 22.0),
                    (16.0, 28.0),
                ],
                BLUE_LIGHT,
                1.4,
            );
            pen.line(
                &[
                    (16.0, 4.0),
                    (20.0, 10.0),
                    (21.0, 16.0),
                    (20.0, 22.0),
                    (16.0, 28.0),
                ],
                BLUE_LIGHT,
                1.4,
            );
        }
        Icon::Archive => {
            pen.rect(
                6.0,
                5.0,
                20.0,
                22.0,
                2.0,
                Color32::from_rgb(0xE8, 0xB9, 0x3A),
            );
            for y in [7.0, 11.0, 15.0] {
                pen.rect(14.0, y, 4.0, 2.0, 0.0, Color32::from_rgb(0x5A, 0x4A, 0x2A));
            }
            pen.rect(
                13.0,
                18.0,
                6.0,
                6.0,
                1.0,
                Color32::from_rgb(0x5A, 0x4A, 0x2A),
            );
        }
        Icon::Installer => {
            pen.rect(
                5.0,
                9.0,
                22.0,
                17.0,
                2.0,
                Color32::from_rgb(0xC9, 0x8B, 0x3C),
            );
            pen.rect(
                5.0,
                9.0,
                22.0,
                5.0,
                2.0,
                Color32::from_rgb(0xA8, 0x6C, 0x26),
            );
            pen.line(&[(16.0, 4.0), (16.0, 20.0)], BLUE, 2.4);
            pen.line(&[(11.5, 15.5), (16.0, 20.0), (20.5, 15.5)], BLUE, 2.4);
        }
        Icon::Shortcut => {
            app_window(&pen, BLUE);
            pen.rect(4.0, 19.0, 9.0, 9.0, 1.0, PAPER);
            pen.outline(4.0, 19.0, 9.0, 9.0, 1.0, PAPER_EDGE, 0.8);
            pen.line(&[(6.5, 25.5), (10.5, 21.5)], BLUE, 1.5);
            pen.line(&[(7.5, 21.5), (10.5, 21.5), (10.5, 24.5)], BLUE, 1.5);
        }
        Icon::Drive | Icon::SystemDrive => {
            pen.rect(
                3.0,
                12.0,
                26.0,
                11.0,
                2.5,
                Color32::from_rgb(0xD5, 0xD9, 0xDE),
            );
            pen.outline(3.0, 12.0, 26.0, 11.0, 2.5, GRAY, 1.0);
            pen.rect(
                5.0,
                19.0,
                22.0,
                1.2,
                0.0,
                Color32::from_rgb(0xA8, 0xAE, 0xB5),
            );
            pen.circle(24.5, 16.0, 1.3, GREEN);
            if icon == Icon::SystemDrive {
                logo_at(&pen, 6.0, 13.0, 0.25);
            }
        }
        Icon::ThisPc => {
            pen.rect(3.0, 5.0, 26.0, 17.0, 2.0, DARK);
            pen.rect(5.0, 7.0, 22.0, 13.0, 1.0, BLUE);
            pen.poly(
                &[(5.0, 20.0), (5.0, 14.0), (27.0, 8.0), (27.0, 20.0)],
                BLUE_LIGHT,
                None,
            );
            pen.rect(13.0, 22.0, 6.0, 3.0, 0.0, GRAY);
            pen.rect(9.0, 25.0, 14.0, 2.0, 1.0, GRAY);
        }
        Icon::RecycleBin => {
            pen.poly(
                &[(8.0, 9.0), (24.0, 9.0), (22.0, 28.0), (10.0, 28.0)],
                Color32::from_rgb(0xDC, 0xE6, 0xEE),
                Some(GRAY),
            );
            pen.rect(6.0, 6.0, 20.0, 3.0, 1.0, GRAY);
            pen.rect(13.0, 4.0, 6.0, 2.0, 1.0, GRAY);
            for x in [12.5, 16.0, 19.5] {
                pen.line(&[(x, 12.0), (x, 25.0)], GRAY, 1.0);
            }
        }
        Icon::Terminal => {
            pen.rect(
                3.0,
                6.0,
                26.0,
                20.0,
                3.0,
                Color32::from_rgb(0x1E, 0x1E, 0x1E),
            );
            pen.outline(
                3.0,
                6.0,
                26.0,
                20.0,
                3.0,
                Color32::from_rgb(0x55, 0x55, 0x55),
                1.0,
            );
            pen.line(&[(8.0, 12.0), (12.0, 16.0), (8.0, 20.0)], PAPER, 1.8);
            pen.line(&[(14.0, 21.0), (21.0, 21.0)], PAPER, 1.8);
        }
        Icon::Notepad => {
            pen.rect(7.0, 4.0, 19.0, 25.0, 2.0, PAPER);
            pen.outline(7.0, 4.0, 19.0, 25.0, 2.0, PAPER_EDGE, 1.0);
            pen.rect(7.0, 4.0, 19.0, 5.0, 2.0, BLUE);
            for y in [13.0, 17.0, 21.0, 25.0] {
                pen.line(&[(10.0, y), (23.0, y)], BLUE_LIGHT, 1.1);
            }
        }
        Icon::TaskManager => {
            app_window(&pen, Color32::from_rgb(0x2D, 0x7D, 0x46));
            pen.line(
                &[
                    (7.0, 22.0),
                    (11.0, 17.0),
                    (14.0, 20.0),
                    (19.0, 13.0),
                    (25.0, 18.0),
                ],
                GREEN,
                1.8,
            );
        }
        Icon::Settings => {
            // Teeth around a thick ring; the hole stays transparent.
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                pen.circle(16.0 + 10.0 * a.cos(), 16.0 + 10.0 * a.sin(), 2.6, ink);
            }
            pen.ring(16.0, 16.0, 6.5, ink, 5.0);
        }
        Icon::Power => {
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2
                        + 0.6
                        + i as f32 * (std::f32::consts::TAU - 1.2) / 24.0;
                    (16.0 + 9.5 * a.cos(), 17.0 + 9.5 * a.sin())
                })
                .collect();
            pen.line(&pts, ink, 2.2);
            pen.line(&[(16.0, 5.0), (16.0, 15.0)], ink, 2.4);
        }
        Icon::Restart => {
            pen.ring(16.0, 16.0, 9.0, ink, 2.2);
            pen.poly(&[(16.0, 3.0), (22.0, 7.0), (16.0, 11.0)], ink, None);
        }
        Icon::User => {
            pen.circle(16.0, 16.0, 14.0, Color32::from_rgb(0xC7, 0xD4, 0xE2));
            pen.circle(16.0, 12.5, 5.0, PAPER);
            pen.poly(
                &[
                    (7.5, 26.0),
                    (10.0, 20.0),
                    (22.0, 20.0),
                    (24.5, 26.0),
                    (16.0, 29.0),
                ],
                PAPER,
                None,
            );
        }
        Icon::Search => {
            pen.ring(13.5, 13.5, 7.0, ink, 2.2);
            pen.line(&[(18.5, 18.5), (26.0, 26.0)], ink, 2.6);
        }
        Icon::Back => arrow(
            &pen,
            &[(26.0, 16.0), (7.0, 16.0)],
            &[(14.0, 9.0), (7.0, 16.0), (14.0, 23.0)],
            ink,
        ),
        Icon::Forward => arrow(
            &pen,
            &[(6.0, 16.0), (25.0, 16.0)],
            &[(18.0, 9.0), (25.0, 16.0), (18.0, 23.0)],
            ink,
        ),
        Icon::Up => arrow(
            &pen,
            &[(16.0, 26.0), (16.0, 7.0)],
            &[(9.0, 14.0), (16.0, 7.0), (23.0, 14.0)],
            ink,
        ),
        Icon::Refresh => {
            let pts: Vec<(f32, f32)> = (0..=20)
                .map(|i| {
                    let a = -0.6 + i as f32 * 0.25;
                    (16.0 + 9.0 * a.cos(), 16.0 + 9.0 * a.sin())
                })
                .collect();
            pen.line(&pts, ink, 2.0);
            pen.poly(&[(23.5, 4.5), (25.5, 12.0), (18.0, 10.5)], ink, None);
        }
        Icon::NewFolder => {
            folder(&pen);
            pen.circle(24.0, 22.0, 6.0, GREEN);
            pen.line(&[(24.0, 18.5), (24.0, 25.5)], PAPER, 1.8);
            pen.line(&[(20.5, 22.0), (27.5, 22.0)], PAPER, 1.8);
        }
        Icon::Delete => {
            pen.line(&[(8.0, 8.0), (24.0, 24.0)], ink, 2.4);
            pen.line(&[(24.0, 8.0), (8.0, 24.0)], ink, 2.4);
        }
        Icon::Run => {
            app_window(&pen, BLUE);
            pen.poly(&[(13.0, 14.0), (21.0, 18.5), (13.0, 23.0)], BLUE, None);
        }
    }
}

fn arrow(pen: &Pen<'_>, shaft: &[(f32, f32)], head: &[(f32, f32)], ink: Color32) {
    pen.line(shaft, ink, 2.0);
    pen.line(head, ink, 2.0);
}

fn folder(pen: &Pen<'_>) {
    pen.rect(3.0, 6.0, 12.0, 6.0, 1.5, FOLDER_BACK);
    pen.rect(3.0, 8.0, 26.0, 19.0, 2.0, FOLDER_BACK);
    pen.rect(3.0, 11.0, 26.0, 16.0, 2.0, FOLDER_FRONT);
    pen.rect(3.0, 11.0, 26.0, 2.0, 1.0, FOLDER_SHINE);
}

fn page(pen: &Pen<'_>) {
    pen.poly(
        &[
            (7.0, 3.0),
            (20.0, 3.0),
            (26.0, 9.0),
            (26.0, 29.0),
            (7.0, 29.0),
        ],
        PAPER,
        Some(PAPER_EDGE),
    );
    pen.poly(
        &[(20.0, 3.0), (26.0, 9.0), (20.0, 9.0)],
        Color32::from_rgb(0xDD, 0xDD, 0xDD),
        Some(PAPER_EDGE),
    );
}

fn app_window(pen: &Pen<'_>, title: Color32) {
    pen.rect(3.0, 6.0, 26.0, 21.0, 2.5, PAPER);
    pen.outline(3.0, 6.0, 26.0, 21.0, 2.5, PAPER_EDGE, 1.0);
    pen.rect(3.0, 6.0, 26.0, 5.0, 2.5, title);
    pen.rect(3.0, 9.0, 26.0, 2.0, 0.0, title);
}

/// The WinCraft mark: an isometric "craft" block (deliberately not a four-pane window).
fn logo(pen: &Pen<'_>) {
    logo_at(pen, 0.0, 0.0, 1.0);
}

fn logo_at(pen: &Pen<'_>, ox: f32, oy: f32, k: f32) {
    let t = |x: f32, y: f32| (ox + x * k, oy + y * k);
    pen.poly(
        &[t(16.0, 3.0), t(28.0, 9.5), t(16.0, 16.0), t(4.0, 9.5)],
        BLUE_LIGHT,
        None,
    );
    pen.poly(
        &[t(4.0, 9.5), t(16.0, 16.0), t(16.0, 29.0), t(4.0, 22.5)],
        BLUE,
        None,
    );
    pen.poly(
        &[t(16.0, 16.0), t(28.0, 9.5), t(28.0, 22.5), t(16.0, 29.0)],
        BLUE_DARK,
        None,
    );
}

/// Picks the icon Explorer shows for a directory entry.
pub fn icon_for_name(name: &str, is_dir: bool) -> Icon {
    if is_dir {
        return match name {
            "Desktop" => Icon::FolderDesktop,
            "Documents" => Icon::FolderDocuments,
            "Downloads" => Icon::FolderDownloads,
            "Music" => Icon::FolderMusic,
            "Pictures" => Icon::FolderPictures,
            "Videos" => Icon::FolderVideos,
            _ => Icon::Folder,
        };
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "exe" | "com" | "scr" => Icon::Application,
        "msi" => Icon::Installer,
        "zip" => Icon::Archive,
        "html" | "htm" | "xhtml" | "url" => Icon::Browser,
        "lnk" | "desktop" => Icon::Shortcut,
        "txt" | "log" | "ini" | "md" | "cfg" | "conf" | "csv" | "json" | "xml" | "reg" | "inf" => {
            Icon::TextFile
        }
        "bat" | "cmd" | "sh" => Icon::Terminal,
        _ => Icon::File,
    }
}

/// Convenience: allocate a square and paint an icon in it.
pub fn icon(ui: &mut egui::Ui, icon: Icon, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    let ink = ui.visuals().text_color();
    paint_icon(ui.painter(), rect, icon, ink);
    response
}
