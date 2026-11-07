use eframe::egui::{
    self, pos2, vec2, Align, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Pos2,
    Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(24, 24, 27);
pub const RAIL: Color32 = Color32::from_rgb(28, 28, 32);
pub const TOP: Color32 = Color32::from_rgb(31, 31, 35);
pub const PANEL: Color32 = Color32::from_rgb(35, 35, 40);
pub const PANEL_2: Color32 = Color32::from_rgb(41, 41, 46);
pub const PANEL_3: Color32 = Color32::from_rgb(48, 48, 54);
pub const LINE: Color32 = Color32::from_rgb(56, 56, 63);
pub const LINE_SOFT: Color32 = Color32::from_rgb(44, 44, 50);
pub const TEXT: Color32 = Color32::from_rgb(232, 232, 236);
pub const MUTED: Color32 = Color32::from_rgb(154, 154, 164);
pub const DIM: Color32 = Color32::from_rgb(108, 108, 118);
pub const ACCENT: Color32 = Color32::from_rgb(10, 147, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(10, 90, 150);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(18, 54, 82);
pub const GREEN: Color32 = Color32::from_rgb(48, 209, 88);
pub const AMBER: Color32 = Color32::from_rgb(255, 159, 10);
pub const RED: Color32 = Color32::from_rgb(255, 69, 58);

pub fn setup(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    use egui::{FontFamily::Proportional, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(26.0, Proportional)),
        (TextStyle::Body, FontId::new(14.0, Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
        (TextStyle::Button, FontId::new(14.0, Proportional)),
        (TextStyle::Small, FontId::new(11.0, Proportional)),
    ]
    .into();
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 7.0);
    style.spacing.interact_size = vec2(32.0, 28.0);
    style.visuals.dark_mode = true;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 25);
    style.visuals.faint_bg_color = PANEL_2;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
    style.visuals.widgets.inactive.bg_fill = PANEL_2;
    style.visuals.widgets.inactive.weak_bg_fill = PANEL_2;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.hovered.bg_fill = PANEL_3;
    style.visuals.widgets.hovered.weak_bg_fill = PANEL_3;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(78, 78, 88));
    style.visuals.widgets.active.bg_fill = ACCENT_DARK;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.selection.bg_fill = ACCENT_SOFT;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals.menu_corner_radius = CornerRadius::same(10);
    style.visuals.window_stroke = Stroke::new(1.0, LINE);
    style.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

pub fn heading(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(25.0).strong().color(TEXT));
            if !subtitle.is_empty() {
                ui.add_space(1.0);
                ui.label(RichText::new(subtitle).size(13.0).color(MUTED));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {});
    });
}

pub fn card() -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE_SOFT))
        .corner_radius(12)
        .inner_margin(Margin::same(18))
}

pub fn card_pad(pad: i8) -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE_SOFT))
        .corner_radius(12)
        .inner_margin(Margin::same(pad))
}

pub fn inner_card() -> Frame {
    Frame::new()
        .fill(PANEL_2)
        .stroke(Stroke::new(1.0, LINE_SOFT))
        .corner_radius(9)
        .inner_margin(Margin::same(13))
}

pub fn section(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .strong()
            .color(DIM),
    );
}

pub fn small(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(MUTED));
}

pub fn divider(ui: &mut Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0u8, LINE_SOFT);
}

pub fn hspace(ui: &mut Ui, h: f32) {
    ui.add_space(h);
}

pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(11.0, FontFamily::Proportional),
        color,
    );
    let size = vec2(galley.size().x + 18.0, 22.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28);
    ui.painter().rect_filled(rect, 11, bg);
    ui.painter().rect_stroke(
        rect,
        11,
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70),
        ),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(11.0, FontFamily::Proportional),
        color,
    );
}

pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(12.5, FontFamily::Proportional),
        if selected { TEXT } else { MUTED },
    );
    let size = vec2(galley.size().x + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = resp.hovered();
    let bg = if selected {
        ACCENT_DARK
    } else if hover {
        PANEL_3
    } else {
        PANEL_2
    };
    ui.painter().rect_filled(rect, 8, bg);
    let stroke = if selected { ACCENT } else { LINE_SOFT };
    ui.painter()
        .rect_stroke(rect, 8, Stroke::new(1.0, stroke), StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(12.5, FontFamily::Proportional),
        if selected { Color32::WHITE } else { MUTED },
    );
    resp
}

pub fn accent_button(ui: &mut Ui, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.5, FontFamily::Proportional),
        Color32::WHITE,
    );
    let w = galley.size().x + 34.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 38.0), Sense::click());
    let hover = resp.hovered();
    let fill = if hover {
        Color32::from_rgb(28, 160, 255)
    } else {
        ACCENT
    };
    ui.painter().rect_filled(rect, 9, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(13.5, FontFamily::Proportional),
        Color32::WHITE,
    );
    resp
}

pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, FontFamily::Proportional),
        MUTED,
    );
    let w = galley.size().x + 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    let hover = resp.hovered();
    ui.painter()
        .rect_filled(rect, 9, if hover { PANEL_3 } else { Color32::TRANSPARENT });
    ui.painter().rect_stroke(
        rect,
        9,
        Stroke::new(1.0, if hover { LINE } else { LINE_SOFT }),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(13.0, FontFamily::Proportional),
        if hover { TEXT } else { MUTED },
    );
    resp
}

pub fn icon_text_button(ui: &mut Ui, icon: &str, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(12.5, FontFamily::Proportional),
        MUTED,
    );
    let w = galley.size().x + 42.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let hover = resp.hovered();
    ui.painter()
        .rect_filled(rect, 8, if hover { PANEL_3 } else { Color32::TRANSPARENT });
    ui.painter().text(
        pos2(rect.left() + 14.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        icon,
        FontId::new(15.0, FontFamily::Proportional),
        if hover { TEXT } else { MUTED },
    );
    ui.painter().text(
        pos2(rect.left() + 26.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::new(12.5, FontFamily::Proportional),
        if hover { TEXT } else { MUTED },
    );
    resp
}

pub fn toggle(ui: &mut Ui, value: &mut bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *value = !*value;
    }
    let t = if *value { 1.0 } else { 0.0 };
    let bg = if *value {
        ACCENT
    } else {
        Color32::from_rgb(60, 60, 68)
    };
    ui.painter().rect_filled(rect, 11, bg);
    let cx = rect.left() + 11.0 + t * 18.0;
    ui.painter()
        .circle_filled(pos2(cx, rect.center().y), 8.0, Color32::WHITE);
    resp
}

pub fn slider(
    ui: &mut Ui,
    value: &mut u8,
    range: std::ops::RangeInclusive<u8>,
    width: f32,
) -> Response {
    let mut v = *value as f32;
    let resp = ui.add_sized(
        [width, 20.0],
        egui::Slider::new(&mut v, *range.start() as f32..=*range.end() as f32)
            .show_value(false)
            .trailing_fill(true),
    );
    if resp.changed() {
        *value = v.round() as u8;
    }
    resp
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavIcon {
    Sliders,
    Keyboard,
    Bulb,
    Macro,
    Music,
    People,
    Device,
    Log,
    Book,
}

pub fn nav_button(ui: &mut Ui, icon: NavIcon, selected: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::click());
    let hover = resp.hovered();
    if selected {
        ui.painter()
            .rect_filled(rect, 10, Color32::from_rgb(32, 55, 74));
        let bar =
            egui::Rect::from_min_size(pos2(rect.left() - 8.0, rect.top() + 11.0), vec2(3.0, 24.0));
        ui.painter().rect_filled(bar, 2, ACCENT);
    } else if hover {
        ui.painter().rect_filled(rect, 10, PANEL_2);
    }
    let color = if selected {
        ACCENT
    } else if hover {
        TEXT
    } else {
        Color32::from_rgb(139, 139, 150)
    };
    draw_nav_icon(ui, rect.center(), icon, color);
    if resp.hovered() {
        resp.clone().on_hover_text(tip);
    }
    resp
}

fn draw_nav_icon(ui: &Ui, c: Pos2, icon: NavIcon, color: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.6, color);
    match icon {
        NavIcon::Sliders => {
            for (i, y) in [-6.0f32, 0.0, 6.0].iter().enumerate() {
                p.line_segment([pos2(c.x - 8.0, c.y + y), pos2(c.x + 8.0, c.y + y)], s);
                let x = c.x + [-4.0, 3.0, -1.0][i];
                p.circle_filled(pos2(x, c.y + y), 2.4, color);
            }
        }
        NavIcon::Keyboard => {
            let r = egui::Rect::from_center_size(c, vec2(17.0, 12.0));
            p.rect_stroke(r, 2, s, StrokeKind::Inside);
            for i in 0..3 {
                for j in 0..4 {
                    p.rect_filled(
                        egui::Rect::from_center_size(
                            pos2(c.x - 5.7 + j as f32 * 3.8, c.y - 3.6 + i as f32 * 3.4),
                            vec2(2.0, 1.8),
                        ),
                        0.5,
                        color,
                    );
                }
            }
        }
        NavIcon::Bulb => {
            p.circle_stroke(pos2(c.x, c.y - 1.0), 5.2, s);
            p.line_segment([pos2(c.x - 2.2, c.y + 5.2), pos2(c.x + 2.2, c.y + 5.2)], s);
            p.line_segment([pos2(c.x - 1.6, c.y + 8.0), pos2(c.x + 1.6, c.y + 8.0)], s);
            for a in [0.0f32, 0.8, 1.6, 2.4, 3.2, 4.0, 4.8, 5.6] {
                let dir = vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        pos2(c.x, c.y - 1.0) + dir * 8.0,
                        pos2(c.x, c.y - 1.0) + dir * 10.0,
                    ],
                    Stroke::new(1.2, color),
                );
            }
        }
        NavIcon::Macro => {
            let r = egui::Rect::from_center_size(c, vec2(17.0, 13.0));
            p.rect_stroke(r, 3, s, StrokeKind::Inside);
            p.line_segment(
                [pos2(c.x - 4.0, c.y - 2.5), pos2(c.x - 1.0, c.y)],
                Stroke::new(1.5, color),
            );
            p.line_segment(
                [pos2(c.x - 1.0, c.y), pos2(c.x - 4.0, c.y + 2.5)],
                Stroke::new(1.5, color),
            );
            p.line_segment(
                [pos2(c.x + 4.0, c.y - 2.5), pos2(c.x + 1.0, c.y)],
                Stroke::new(1.5, color),
            );
            p.line_segment(
                [pos2(c.x + 1.0, c.y), pos2(c.x + 4.0, c.y + 2.5)],
                Stroke::new(1.5, color),
            );
        }
        NavIcon::Music => {
            p.line_segment(
                [pos2(c.x + 4.0, c.y - 7.0), pos2(c.x + 4.0, c.y + 4.0)],
                Stroke::new(1.6, color),
            );
            p.line_segment(
                [pos2(c.x + 4.0, c.y - 7.0), pos2(c.x - 3.0, c.y - 8.5)],
                Stroke::new(1.6, color),
            );
            p.circle_filled(pos2(c.x + 1.2, c.y + 5.5), 3.2, color);
        }
        NavIcon::People => {
            p.circle_stroke(pos2(c.x, c.y - 5.0), 3.6, s);
            p.line_segment([pos2(c.x - 7.0, c.y + 8.0), pos2(c.x - 5.0, c.y + 2.0)], s);
            p.line_segment([pos2(c.x + 7.0, c.y + 8.0), pos2(c.x + 5.0, c.y + 2.0)], s);
            p.line_segment([pos2(c.x - 5.0, c.y + 2.0), pos2(c.x + 5.0, c.y + 2.0)], s);
        }
        NavIcon::Device => {
            let r = egui::Rect::from_center_size(c, vec2(15.0, 15.0));
            p.rect_stroke(r, 2, s, StrokeKind::Inside);
            p.rect_filled(egui::Rect::from_center_size(c, vec2(5.0, 5.0)), 1, color);
            for i in 0..3 {
                let off = -5.0 + i as f32 * 5.0;
                p.line_segment([pos2(c.x + off, c.y - 7.5), pos2(c.x + off, c.y - 10.0)], s);
                p.line_segment([pos2(c.x + off, c.y + 7.5), pos2(c.x + off, c.y + 10.0)], s);
            }
        }
        NavIcon::Log => {
            let r = egui::Rect::from_center_size(c, vec2(15.0, 18.0));
            p.rect_stroke(r, 2, s, StrokeKind::Inside);
            for (i, w) in [10.0f32, 8.0, 10.0, 6.0].iter().enumerate() {
                let y = c.y - 5.6 + i as f32 * 3.7;
                p.line_segment(
                    [pos2(c.x - w / 2.0, y), pos2(c.x + w / 2.0, y)],
                    Stroke::new(1.2, color),
                );
            }
        }
        NavIcon::Book => {
            let r = egui::Rect::from_center_size(c, vec2(15.0, 18.0));
            p.rect_stroke(r, 2, s, StrokeKind::Inside);
            p.line_segment(
                [pos2(c.x, c.y - 8.0), pos2(c.x, c.y + 8.0)],
                Stroke::new(1.2, color),
            );
            for y in [-4.5f32, 0.0, 4.5] {
                p.line_segment(
                    [pos2(c.x - 5.0, c.y + y), pos2(c.x - 2.0, c.y + y)],
                    Stroke::new(1.0, color),
                );
                p.line_segment(
                    [pos2(c.x + 2.0, c.y + y), pos2(c.x + 5.0, c.y + y)],
                    Stroke::new(1.0, color),
                );
            }
        }
    }
}

pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 9, ACCENT_DARK);
    p.rect_filled(
        egui::Rect::from_min_size(
            rect.min + vec2(1.0, 1.0),
            vec2(size - 2.0, (size - 2.0) * 0.55),
        ),
        9,
        Color32::from_rgba_unmultiplied(10, 147, 255, 60),
    );
    p.rect_stroke(rect, 9, Stroke::new(1.0, ACCENT_SOFT), StrokeKind::Inside);
    let r = egui::Rect::from_center_size(rect.center(), vec2(size * 0.5, size * 0.34));
    p.rect_stroke(r, 3, Stroke::new(1.4, Color32::WHITE), StrokeKind::Inside);
    for i in 0..3 {
        p.rect_filled(
            egui::Rect::from_center_size(
                pos2(
                    r.left() + size * 0.13 + i as f32 * size * 0.12,
                    r.center().y,
                ),
                vec2(2.4, 2.0),
            ),
            0.5,
            Color32::WHITE,
        );
    }
}

pub fn dot(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size * 0.5, color);
}

pub fn keycap_label(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    ui.painter().rect_filled(rect, 4, PANEL_3);
    ui.painter()
        .rect_stroke(rect, 4, Stroke::new(1.0, LINE), StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(10.5, FontFamily::Proportional),
        TEXT,
    );
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

pub fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let h = h.rem_euclid(1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

pub fn spacer(ui: &mut Ui, w: f32) {
    ui.add_space(w);
}

pub fn panel_title(ui: &mut Ui, title: &str, right: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(15.0).strong().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if !right.is_empty() {
                ui.label(RichText::new(right).size(11.0).color(DIM));
            }
        });
    });
    ui.add_space(6.0);
}

pub fn underline_tab(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, FontFamily::Proportional),
        if selected { TEXT } else { MUTED },
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 8.0, 34.0), Sense::click());
    ui.painter().text(
        pos2(rect.center().x, rect.center().y - 1.0),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(13.0, FontFamily::Proportional),
        if selected {
            TEXT
        } else if resp.hovered() {
            TEXT
        } else {
            MUTED
        },
    );
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                pos2(rect.left() + 2.0, rect.bottom() - 2.0),
                vec2(rect.width() - 4.0, 2.0),
            ),
            1,
            ACCENT,
        );
    }
    resp
}

pub fn draw_battery(ui: &mut Ui, pct: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(30.0, 14.0), Sense::hover());
    let p = ui.painter();
    let body = egui::Rect::from_min_size(rect.min, vec2(25.0, 12.0));
    p.rect_stroke(body, 2, Stroke::new(1.2, MUTED), StrokeKind::Inside);
    p.rect_filled(
        egui::Rect::from_min_size(pos2(body.right(), body.center().y - 3.0), vec2(2.5, 6.0)),
        1,
        MUTED,
    );
    let fill = (body.width() - 4.0) * pct;
    p.rect_filled(
        egui::Rect::from_min_size(body.min + Vec2::splat(2.0), vec2(fill, body.height() - 4.0)),
        1,
        GREEN,
    );
}
