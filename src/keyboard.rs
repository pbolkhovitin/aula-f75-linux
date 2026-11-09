use crate::model::{AppState, KeyDef, KEYS};
use crate::theme::*;
use eframe::egui::{
    self, pos2, vec2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

pub struct KeyHit {
    pub hovered: Option<usize>,
    pub clicked: Option<usize>,
    pub secondary: Option<usize>,
}

const X0: f32 = 27.0;
const Y0: f32 = 27.0;
const KB_W: f32 = 610.0;
const KB_H: f32 = 242.0;

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E3779B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EBCA6B);
    x ^= x >> 13;
    (x & 0xFFFF) as f32 / 65535.0
}

/// Colour of one LED at this moment for the selected effect. This is a visual
/// re-creation; the hardware effect IDs live in `Effect::hw`.
pub fn led_color(state: &AppState, key: &KeyDef, time: f32) -> Color32 {
    let l = &state.lighting;
    let e = crate::model::effect_by_ui(l.effect);
    if l.effect == 20 {
        return Color32::TRANSPARENT;
    }
    let base = rgb(l.color);
    let bri = (l.brightness as f32 + 1.0) / 10.0;
    let spd = 0.35 + l.speed as f32 * 0.22;
    let t = time * spd;
    let nx = key.x / 600.0;
    let ny = key.y / 233.0;
    let led = key.led as f32;
    let mut c = base;
    let mut v = 1.0f32;
    match l.effect {
        1 => {
            c = base;
            v = 1.0;
        }
        2 => {
            v = 0.35 + 0.65 * (0.5 + 0.5 * (t * 1.8).sin());
        }
        3 => {
            c = hsl(nx * 0.8 + t * 0.06, 0.85, 0.55);
        }
        4 => {
            let sweep = (t * 0.55).rem_euclid(1.3) - 0.15;
            let d = (nx - sweep).abs();
            v = if d < 0.12 { 1.0 } else { 0.18 };
        }
        5 => {
            let h = hash(key.index as u32 * 7 + (t * 0.9) as u32);
            v = if h > 0.72 { 0.55 + h * 0.45 } else { 0.12 };
        }
        6 => {
            let ang = ((nx - 0.5) * 2.0).atan2(ny - 0.5);
            c = hsl(ang / std::f32::consts::TAU + t * 0.12, 0.9, 0.55);
        }
        7 => {
            let d = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
            v = 0.18 + 0.82 * (0.5 + 0.5 * (d * 18.0 - t * 2.4).sin());
        }
        8 => {
            let h = hash(key.index as u32 * 13 + (t * 0.55) as u32);
            v = if h > 0.8 { 0.95 } else { 0.1 };
            c = if h > 0.93 {
                base
            } else {
                mix(base, Color32::WHITE, 0.35)
            };
        }
        9 => {
            let sweep = (t * 0.5).rem_euclid(1.2);
            let d = (nx - sweep).abs();
            v = if d < 0.2 { 1.0 - d * 4.0 } else { 0.14 };
        }
        10 => {
            // Retro snake follows the LED index order in KB.ini.
            let head = (t * 24.0) % 96.0;
            let d = (led - head).rem_euclid(96.0);
            v = if d < 6.0 { 1.0 - d / 7.0 } else { 0.1 };
            c = mix(base, Color32::WHITE, 0.15);
        }
        11 => {
            c = hsl((nx * 0.55 + ny * 0.8 + t * 0.16).rem_euclid(1.0), 0.9, 0.58);
            v = 0.55 + 0.45 * (0.5 + 0.5 * (nx * 10.0 + ny * 5.0 - t * 2.0).sin());
        }
        12 => {
            let h = hash(key.index as u32 * 31 + (t * 1.2) as u32);
            v = if h > 0.55 { 0.35 + h * 0.65 } else { 0.1 };
        }
        13 => {
            let w = 0.5 + 0.5 * (nx * 9.0 - t * 2.6).sin();
            v = 0.2 + 0.8 * w;
            c = hsl((nx * 0.35 + t * 0.05).rem_euclid(1.0), 0.85, 0.55);
        }
        14 => {
            let sweep = (t * 0.32).rem_euclid(1.4) - 0.2;
            let d = (nx - sweep).abs();
            v = if d < 0.05 {
                1.0
            } else if d < 0.35 {
                1.0 - d * 2.4
            } else {
                0.12
            };
        }
        15 => {
            let ang = ((nx - 0.5) * 2.0).atan2(ny - 0.5);
            let a = (ang * 3.0 + t * 0.8).rem_euclid(std::f32::consts::TAU);
            v = if a < 1.2 {
                0.35 + 0.65 * (1.0 - a / 1.2)
            } else {
                0.12
            };
            c = hsl(
                (ang / std::f32::consts::TAU + 0.5).rem_euclid(1.0),
                0.9,
                0.55,
            );
        }
        16 => {
            let flow = (ny + t * 0.2).rem_euclid(1.0);
            c = hsl((flow * 0.9 + nx * 0.1).rem_euclid(1.0), 0.9, 0.55);
            v = 0.45 + 0.55 * (0.5 + 0.5 * (ny * 14.0 - t * 2.2).sin());
        }
        17 => {
            let d = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
            v = 0.15 + 0.85 * (0.5 + 0.5 * (d * 26.0 - t * 1.8).sin());
            c = mix(base, hsl(0.92, 0.7, 0.62), (d * 2.0).min(0.7));
        }
        18 => {
            let ang = ((nx - 0.5) * 2.0).atan2(ny - 0.5);
            let a = (ang * 4.0 + t * 1.6).rem_euclid(std::f32::consts::TAU);
            v = if a < 0.9 { 0.9 } else { 0.12 };
            c = hsl(
                (ang / std::f32::consts::TAU + t * 0.06).rem_euclid(1.0),
                0.9,
                0.55,
            );
        }
        19 => {
            c = rgb(l.per_key.get(key.index).copied().unwrap_or(l.color));
            v = 1.0;
        }
        _ => {}
    }
    if l.random_color && e.random {
        c = hsl(hash(key.index as u32 * 5 + 11), 0.88, 0.58);
    }
    let v = (v * (0.42 + bri * 0.72)).clamp(0.0, 1.25);
    let mut c = Color32::from_rgb(
        (c.r() as f32 * v).min(255.0) as u8,
        (c.g() as f32 * v).min(255.0) as u8,
        (c.b() as f32 * v).min(255.0) as u8,
    );
    if l.effect == 1 || l.effect == 19 {
        c = Color32::from_rgb(
            (c.r() as f32 * bri).min(255.0) as u8,
            (c.g() as f32 * bri).min(255.0) as u8,
            (c.b() as f32 * bri).min(255.0) as u8,
        );
    }
    c
}

pub fn keyboard(
    ui: &mut Ui,
    state: &AppState,
    desired: Vec2,
    time: f32,
    interactive: bool,
    show_badges: bool,
) -> KeyHit {
    let (area, resp) = ui.allocate_exact_size(
        desired,
        if interactive {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    let scale = ((desired.x - 18.0) / KB_W)
        .min((desired.y - 18.0) / KB_H)
        .max(0.2);
    let origin = pos2(
        area.center().x - KB_W * scale * 0.5,
        area.center().y - KB_H * scale * 0.5,
    );
    let map_rect = |k: &KeyDef| {
        Rect::from_min_size(
            pos2(origin.x + (k.x - X0) * scale, origin.y + (k.y - Y0) * scale),
            vec2(k.w * scale, k.h * scale),
        )
    };

    // Case.
    let case = Rect::from_min_size(
        origin + vec2(-12.0, -12.0) * scale,
        vec2((KB_W + 24.0) * scale, (KB_H + 24.0) * scale),
    );
    p.rect_filled(case.expand(6.0), 14, Color32::from_rgb(15, 15, 17));
    p.rect_stroke(
        case.expand(6.0),
        14,
        Stroke::new(1.0, Color32::from_rgb(50, 50, 56)),
        StrokeKind::Inside,
    );
    p.rect_filled(case, 10, Color32::from_rgb(23, 23, 26));
    p.rect_stroke(
        case,
        10,
        Stroke::new(1.0, Color32::from_rgb(39, 39, 44)),
        StrokeKind::Inside,
    );

    // Side underglow strip.
    if state.lighting.side_led && state.lighting.effect != 20 {
        let glow = led_color(state, &KEYS[35], time + 0.4);
        let r = Rect::from_min_max(
            pos2(case.left() + 18.0 * scale, case.bottom() - 4.0),
            pos2(case.right() - 18.0 * scale, case.bottom() - 1.0),
        );
        p.rect_filled(r, 2, with_alpha(glow, 150));
        p.rect_filled(r.expand(2.0), 3, with_alpha(glow, 28));
    }

    // Ambient glow layer: multiple translucent outlines give a soft bloom.
    for k in KEYS.iter() {
        let color = led_color(state, k, time);
        if color.a() == 0 {
            continue;
        }
        let r = map_rect(k);
        p.rect_filled(
            r.expand(4.0),
            6,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 12),
        );
        p.rect_filled(
            r.expand(1.5),
            5,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 24),
        );
    }

    let mut hit = KeyHit {
        hovered: None,
        clicked: None,
        secondary: None,
    };
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let clicked = interactive && resp.clicked();
    let right_clicked = interactive && resp.secondary_clicked();
    for k in KEYS.iter() {
        let r = map_rect(k);
        let color = led_color(state, k, time);
        let lit = color.a() > 0 && state.lighting.effect != 20;
        if let Some(pos) = pointer {
            if r.contains(pos) {
                hit.hovered = Some(k.index);
                if clicked {
                    hit.clicked = Some(k.index);
                }
                if right_clicked {
                    hit.secondary = Some(k.index);
                }
            }
        }
        let is_sel = interactive && state.active_key == k.index;
        let is_fn_remap = show_badges && state.is_remapped(k.index);
        let body = if lit {
            mix(
                Color32::from_rgb(43, 43, 49),
                color,
                0.30 + 0.18 * state.lighting.brightness as f32 / 10.0,
            )
        } else {
            Color32::from_rgb(43, 43, 49)
        };
        let body = if hit.hovered == Some(k.index) && interactive {
            mix(body, Color32::WHITE, 0.06)
        } else {
            body
        };
        let edge = if is_sel {
            Color32::WHITE
        } else if lit {
            mix(Color32::from_rgb(62, 62, 70), color, 0.55)
        } else {
            Color32::from_rgb(34, 34, 39)
        };
        p.rect_filled(
            r.translate(vec2(0.0, 1.0)),
            4,
            Color32::from_rgb(18, 18, 21),
        );
        p.rect_filled(r, 4, body);
        p.rect_stroke(
            r,
            4,
            Stroke::new(if is_sel { 1.6 } else { 1.0 }, edge),
            StrokeKind::Inside,
        );
        // tiny top highlight
        p.line_segment(
            [
                pos2(r.left() + 3.0, r.top() + 1.2),
                pos2(r.right() - 3.0, r.top() + 1.2),
            ],
            Stroke::new(0.8, Color32::from_white_alpha(if lit { 24 } else { 10 })),
        );
        if lit {
            p.rect_filled(
                Rect::from_min_size(
                    pos2(r.left() + 2.0, r.bottom() - 2.5),
                    vec2(r.width() - 4.0, 1.5),
                ),
                1,
                with_alpha(color, 180),
            );
        }
        let label = match k.label {
            _ if k.label.len() > 4 => k.label.to_uppercase(),
            _ => k.label.to_string(),
        };
        let fs: f32 = if scale < 0.9 {
            6.5
        } else if label.len() > 3 {
            7.6
        } else if label.len() > 1 {
            8.4
        } else {
            9.5
        };
        let txt = if lit {
            mix(Color32::from_rgb(236, 236, 240), color, 0.35)
        } else {
            Color32::from_rgb(196, 196, 205)
        };
        let arrow = match k.label {
            "Left" => Some(0u8),
            "Right" => Some(1),
            "Up" => Some(2),
            "Down" => Some(3),
            _ => None,
        };
        if let Some(dir) = arrow {
            let c = r.center();
            let s = (r.width().min(r.height()) * 0.22).clamp(2.5, 5.0);
            let pts: Vec<Pos2> = match dir {
                0 => vec![
                    pos2(c.x + s, c.y - s),
                    pos2(c.x - s, c.y),
                    pos2(c.x + s, c.y + s),
                ],
                1 => vec![
                    pos2(c.x - s, c.y - s),
                    pos2(c.x + s, c.y),
                    pos2(c.x - s, c.y + s),
                ],
                2 => vec![
                    pos2(c.x - s, c.y + s),
                    pos2(c.x, c.y - s),
                    pos2(c.x + s, c.y + s),
                ],
                _ => vec![
                    pos2(c.x - s, c.y - s),
                    pos2(c.x, c.y + s),
                    pos2(c.x + s, c.y - s),
                ],
            };
            p.add(egui::Shape::convex_polygon(pts, txt, Stroke::NONE));
        } else {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::new(fs.max(6.0) * scale.max(0.8), FontFamily::Proportional),
                txt,
            );
        }
        if is_fn_remap {
            let dot = pos2(r.right() - 4.5, r.top() + 4.5);
            p.circle_filled(dot, 2.2, ACCENT);
        }
        if is_sel {
            let ring = r.expand(2.0);
            p.rect_stroke(ring, 6, Stroke::new(2.0, ACCENT), StrokeKind::Outside);
        }
    }

    // Small wheel hint on the right of the case.
    let wheel = Rect::from_center_size(
        pos2(case.right() - 12.0 * scale, area.center().y),
        vec2(10.0 * scale, 34.0 * scale),
    );
    p.rect_filled(wheel, 4, Color32::from_rgb(38, 38, 43));
    p.rect_stroke(
        wheel,
        4,
        Stroke::new(1.0, Color32::from_rgb(58, 58, 64)),
        StrokeKind::Inside,
    );

    if interactive {
        if let Some(h) = hit.hovered {
            let k = &KEYS[h];
            let extra = state
                .assignment(h)
                .map(|a| format!(" · {}", a.action))
                .unwrap_or_default();
            resp.clone().on_hover_text(format!(
                "{}  ·  VK 0x{:02X}  ·  LED {}{}",
                k.label, k.vk, k.led, extra
            ));
        }
    }
    hit
}

pub fn key_label_for(index: usize) -> &'static str {
    KEYS.get(index).map(|k| k.label).unwrap_or("Key")
}

pub fn keyboard_bounds_hint() -> Vec2 {
    vec2(KB_W, KB_H)
}

pub fn selected_rect_color() -> Color32 {
    ACCENT
}

pub fn draw_light_legend(ui: &mut Ui, state: &AppState) {
    let p = ui.painter();
    let _ = p;
    ui.horizontal(|ui| {
        let color = rgb(state.lighting.color);
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 5.0, color);
        ui.label(
            egui::RichText::new(format!(
                "{} · {}%",
                crate::model::effect_by_ui(state.lighting.effect).name,
                (state.lighting.brightness as u32 + 1) * 10
            ))
            .size(11.5)
            .color(MUTED),
        );
    });
}

pub fn draw_arrow(ui: &mut Ui, dir: u8) {
    let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    let p = ui.painter();
    let c = r.center();
    let pts: Vec<Pos2> = match dir {
        0 => vec![
            pos2(c.x + 4.0, c.y - 5.0),
            pos2(c.x - 3.0, c.y),
            pos2(c.x + 4.0, c.y + 5.0),
        ],
        1 => vec![
            pos2(c.x - 4.0, c.y - 5.0),
            pos2(c.x + 3.0, c.y),
            pos2(c.x - 4.0, c.y + 5.0),
        ],
        2 => vec![
            pos2(c.x - 5.0, c.y + 4.0),
            pos2(c.x, c.y - 3.0),
            pos2(c.x + 5.0, c.y + 4.0),
        ],
        _ => vec![
            pos2(c.x - 5.0, c.y - 4.0),
            pos2(c.x, c.y + 3.0),
            pos2(c.x + 5.0, c.y - 4.0),
        ],
    };
    p.add(egui::Shape::convex_polygon(pts, ACCENT, Stroke::NONE));
}

pub fn draw_keyboard_icon(ui: &mut Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let r = Rect::from_center_size(rect.center(), vec2(size * 0.9, size * 0.62));
    p.rect_stroke(r, 3, Stroke::new(1.2, color), StrokeKind::Inside);
    for i in 0..3 {
        for j in 0..5 {
            p.rect_filled(
                Rect::from_center_size(
                    pos2(
                        r.left() + size * 0.15 + j as f32 * size * 0.15,
                        r.top() + size * 0.14 + i as f32 * size * 0.15,
                    ),
                    vec2(size * 0.08, size * 0.06),
                ),
                0.5,
                color,
            );
        }
    }
}
