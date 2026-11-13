use crate::device::{AccessCheck, DeviceInfo, JobOutcome};
use crate::keyboard::{self, key_label_for};
use crate::log;
use crate::model::*;
use crate::theme::*;
use eframe::egui::{
    self, pos2, vec2, Align, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui,
};
use std::sync::mpsc::{Receiver, TryRecvError};

/// Two-column pages were designed at 1200 px. niri tiles windows to the
/// column width, so the same page can be narrower (half a screen) or much
/// wider (a full 4K output). Keep the design widths when they fit, otherwise
/// shrink both columns proportionally, with a floor that keeps controls
/// usable, and let the caller centre the whole block on wide screens.
fn columns(ui: &Ui, left_design: f32, right_design: f32) -> (f32, f32) {
    const GAP: f32 = 12.0;
    let avail = ui.available_width();
    if avail >= left_design + right_design + GAP {
        return (left_design, right_design);
    }
    let scale = ((avail - GAP) / (left_design + right_design)).clamp(0.55, 1.0);
    let min_left = left_design.min(380.0);
    // The right column holds the effect grid and the colour swatches. Below
    // about 330 px the ten swatches wrap and push Apply out of the card, so
    // keep that floor and give the left column what is left.
    let min_right = right_design.min(342.0);
    let right = (right_design * scale).max(min_right).min(avail - GAP - 240.0);
    let left = (avail - GAP - right).max(min_left.min(avail - GAP - right));
    (left, right)
}

/// A background device task. The kernel can spend five seconds inside one
/// feature-report write; running it on the UI thread made the whole window
/// look frozen, which is half of why "it does not work" felt like a crash.
pub struct Job {
    pub label: String,
    pub started: f64,
    rx: Receiver<JobOutcome>,
}

pub struct UiState {
    pub toast: Option<(String, f64)>,
    pub rename: Option<usize>,
    pub rename_buf: String,
    pub macro_play: Option<f64>,
    pub record_started: f64,
    pub devices: Vec<DeviceInfo>,
    pub confirm_reset: bool,
    pub export_name: String,
    pub import_name: String,
    pub file_dialog: Option<bool>, // Some(true)=export, Some(false)=import
    pub close_dialog: bool,
    pub last_apply: Option<(String, bool)>,
    pub access: Vec<AccessCheck>,
    pub access_checked: bool,
    pub job: Option<Job>,
    pub install_result: Option<(String, bool)>,
    pub probe: Vec<crate::device::ProbeReport>,
    pub probe_checked: bool,
    pub log_filter: u8, // 0 all, 1 errors, 2 warnings and up, 3 info and up
    pub log_search: String,
    pub log_confirm_clear: bool,
    pub log_notice: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            toast: None,
            rename: None,
            rename_buf: String::new(),
            macro_play: None,
            record_started: 0.0,
            devices: crate::device::detect(),
            confirm_reset: false,
            export_name: "my-f75-profile".into(),
            import_name: String::new(),
            file_dialog: None,
            close_dialog: false,
            last_apply: None,
            access: Vec::new(),
            access_checked: false,
            job: None,
            install_result: None,
            probe: Vec::new(),
            probe_checked: false,
            log_filter: 0,
            log_search: String::new(),
            log_confirm_clear: false,
            log_notice: None,
        }
    }
}

impl UiState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
}

/// Write the current lighting to the keyboard over the vendor feature report.
/// This is deliberately explicit: the vendor app also only sends on its Save
/// button, and the keyboard must be awake and in Windows mode for it to stick.
pub fn apply_to_keyboard(s: &mut AppState, v: &mut UiState, now: f64) {
    save_current_profile(s);
    v.devices = crate::device::detect();
    v.access = crate::device::check_access();
    v.access_checked = true;
    let russian = s.prefs.lang == Lang::Ru;

    if v.devices.is_empty() {
        let message = if russian {
            "Клавиатура F75 не найдена. Подключите её по USB, нажмите любую клавишу и повторите."
        } else {
            "No F75 found. Connect it by USB, press any key and try again."
        };
        log::warn("apply", "refused: no F75 hidraw node found");
        v.last_apply = Some((message.to_string(), false));
        toast(v, message, now);
        return;
    }
    if !access_ready(v) {
        s.page = 6;
        let message = if russian {
            "У программы нет прав на клавиатуру. Открыл страницу «Устройство» — нажмите «Дать доступ сейчас»."
        } else {
            "The app has no permission for the keyboard. Opened Device — press “Allow access now”."
        };
        log::warn(
            "apply",
            format!(
                "refused: no write permission for {}",
                v.devices
                    .iter()
                    .map(|d| d.path.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        v.last_apply = Some((message.to_string(), false));
        toast(v, message, now);
        return;
    }
    if v.job.is_some() {
        log::warn("apply", "refused: another device task is still running");
        toast(
            v,
            t(
                s,
                "A device task is already running",
                "Устройство уже выполняет задачу",
            ),
            now,
        );
        return;
    }
    v.last_apply = None;
    log::info(
        "apply",
        format!(
            "queued: {} device node(s), profile \"{}\", effect {}, {} key assignment(s), {} macro(s)",
            v.devices.len(),
            s.profiles
                .get(s.active_profile)
                .map(|p| p.name.as_str())
                .unwrap_or("Default"),
            s.lighting.effect,
            s.keys.base.iter().filter(|a| a.is_some()).count()
                + s.keys.func.iter().filter(|a| a.is_some()).count(),
            s.macros.len()
        ),
    );
    v.job = Some(Job {
        label: if russian {
            "Отправляю настройки на клавиатуру".into()
        } else {
            "Sending settings to the keyboard".into()
        },
        started: now,
        rx: crate::device::spawn_apply(
            s.lighting.clone(),
            s.keys.clone(),
            s.macros.clone(),
            russian,
        ),
    });
    toast(
        v,
        if russian {
            "Отправляю настройки. Это может занять до 15 секунд — окно остаётся отзывчивым."
        } else {
            "Sending settings. This can take up to 15 seconds; the window stays responsive."
        },
        now,
    );
}

/// Collect finished background work. Called once per frame from the app.
pub fn poll_jobs(s: &mut AppState, v: &mut UiState, now: f64) {
    let Some(job) = v.job.take() else {
        return;
    };
    let russian = s.prefs.lang == Lang::Ru;
    match job.rx.try_recv() {
        Err(TryRecvError::Empty) => {
            v.job = Some(job);
        }
        Err(TryRecvError::Disconnected) => {
            log::error(
                "apply",
                format!("worker for \"{}\" ended without a result", job.label),
            );
            let message = if russian {
                "Фоновая задача неожиданно завершилась. Попробуйте ещё раз."
            } else {
                "The background task ended unexpectedly. Try again."
            };
            v.last_apply = Some((message.to_string(), false));
            toast(v, message, now);
        }
        Ok(JobOutcome::Applied(result)) => match result {
            Ok(message) => {
                log::info("apply", &message);
                v.last_apply = Some((message.clone(), true));
                toast(v, message, now);
            }
            Err(error) => {
                log::error("apply", &error);
                v.last_apply = Some((error.clone(), false));
                toast(v, error, now);
            }
        },
        Ok(JobOutcome::Installed(result)) => {
            match result {
                Ok(message) => {
                    log::info("access", &message);
                    v.install_result = Some((message.clone(), true));
                    refresh_access(v);
                    toast(
                        v,
                        if access_ready(v) {
                            if russian {
                                "Доступ выдан. Проверьте связь и применяйте подсветку."
                            } else {
                                "Access granted. Check the connection and apply lighting."
                            }
                        } else if russian {
                            "Правило записано. Если доступа всё ещё нет, переподключите клавиатуру и нажмите «Проверить доступ»."
                        } else {
                            "The rule is written. If access is still missing, replug the keyboard and press Check access."
                        },
                        now,
                    );
                }
                Err(error) => {
                    log::error("access", format!("install failed: {error}"));
                    v.install_result = Some((error.clone(), false));
                    toast(
                        v,
                        if russian {
                            format!("Система не выдала доступ: {error}")
                        } else {
                            format!("The system did not grant access: {error}")
                        },
                        now,
                    );
                }
            }
            v.probe.clear();
            v.probe_checked = false;
        }
        Ok(JobOutcome::Probed(reports)) => {
            for report in &reports {
                if report.profile_ok {
                    log::info(
                        "probe",
                        format!(
                            "{}: profile read ok in {} ms",
                            report.path, report.request_ms
                        ),
                    );
                } else {
                    log::warn(
                        "probe",
                        format!(
                            "{}: access={} read={} request={} profile={}",
                            report.path,
                            report.access_ok,
                            report.read_ok,
                            report.request_ok,
                            report.profile_error
                        ),
                    );
                }
            }
            let verdict = crate::device::probe_verdict(&reports, russian);
            let ok = reports.iter().any(|r| r.profile_ok);
            v.probe = reports;
            v.probe_checked = true;
            v.last_apply = Some((verdict.clone(), ok));
            toast(v, verdict, now);
        }
    }
}

pub fn toast(v: &mut UiState, text: impl Into<String>, now: f64) {
    let text = text.into();
    // Long explanations (permissions, refused writes) need time to read.
    let extra = (text.chars().count() as f64 / 45.0).min(8.0);
    v.toast = Some((text, now + 2.8 + extra));
}

fn t(s: &AppState, en: &'static str, ru: &'static str) -> &'static str {
    tr(s.prefs.lang, en, ru)
}

fn page_header(
    ui: &mut Ui,
    s: &AppState,
    title_en: &'static str,
    title_ru: &'static str,
    sub_en: &'static str,
    sub_ru: &'static str,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(t(s, title_en, title_ru))
                    .size(25.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(2.0);
            ui.label(RichText::new(t(s, sub_en, sub_ru)).size(12.5).color(MUTED));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            pill(ui, t(s, "LOCAL PROFILE", "ЛОКАЛЬНЫЙ ПРОФИЛЬ"), GREEN);
            ui.add_space(6.0);
            let d = if s.active_profile < s.profiles.len() {
                s.profiles[s.active_profile].name.as_str()
            } else {
                "Default"
            };
            ui.label(RichText::new(d).size(12.0).color(MUTED));
        });
    });
}

pub fn lighting(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    // niri gives the window the full output height; a 680 px tile is the
    // narrow case. Trim the effect list there so Apply stays in view.
    let short = ui.ctx().viewport_rect().height() < 720.0;
    page_header(
        ui,
        s,
        "Lighting",
        "Подсветка",
        "Per-key RGB, effects and side light",
        "Подсветка каждой клавиши, эффекты и боковая лента",
    );
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 710.0, 342.0);
        ui.allocate_ui_with_layout(vec2(w_left, 520.0), Layout::top_down(Align::Min), |ui| {
            card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Keyboard preview", "Предпросмотр клавиатуры")).size(15.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let live = format!("{}  ·  {}%", effect_by_ui(s.lighting.effect).name, (s.lighting.brightness as u32 + 1) * 10);
                        ui.label(RichText::new(live).size(11.5).color(ACCENT));
                        let (r, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, ACCENT);
                    });
                });
                ui.add_space(6.0);
                let hit = keyboard::keyboard(ui, s, vec2((w_left - 38.0).max(320.0), 300.0), time, true, true);
                if let Some(k) = hit.clicked {
                    if s.lighting.paint {
                        if let Some(c) = s.lighting.per_key.get_mut(k) {
                            *c = s.lighting.color;
                        }
                        s.lighting.effect = 19;
                        toast(v, format!("{} painted", key_label_for(k)), time as f64);
                    } else {
                        s.active_key = k;
                        toast(v, format!("{} selected", key_label_for(k)), time as f64);
                    }
                }
                if let Some(k) = hit.secondary {
                    if let Some(c) = s.lighting.per_key.get_mut(k) {
                        *c = s.lighting.color;
                    }
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if s.lighting.paint { t(s, "Paint mode: click a key", "Режим кисти: нажмите клавишу") } else { t(s, "Click a key to inspect it", "Нажмите клавишу для выбора") }).size(11.0).color(DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} keys · {} LEDs · FN layer", KEY_COUNT, LED_COUNT)).size(10.5).color(DIM));
                    });
                });
            });
            ui.add_space(12.0);
            card_pad(14).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut paint = s.lighting.paint;
                    if toggle(ui, &mut paint).clicked() {
                        s.lighting.paint = paint;
                        if paint {
                            s.lighting.effect = 19;
                        }
                    }
                    ui.label(RichText::new(t(s, "Per-key painting", "Рисование по клавишам")).size(12.5));
                    ui.add_space(10.0);
                    if ghost_button(ui, t(s, "Fill all", "Залить всё")).clicked() {
                        for c in s.lighting.per_key.iter_mut() {
                            *c = s.lighting.color;
                        }
                        s.lighting.effect = 19;
                        toast(v, t(s, "All keys painted", "Все клавиши залиты"), time as f64);
                    }
                    if ghost_button(ui, t(s, "Clear", "Очистить")).clicked() {
                        for c in s.lighting.per_key.iter_mut() {
                            *c = [0, 0, 0];
                        }
                        s.lighting.effect = 19;
                        toast(v, t(s, "Per-key colours cleared", "Цвета клавиш очищены"), time as f64);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut side = s.lighting.side_led;
                        if toggle(ui, &mut side).clicked() {
                            s.lighting.side_led = side;
                        }
                        ui.label(RichText::new(t(s, "Side light", "Боковая лента")).size(12.0).color(MUTED));
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Brightness", "Яркость")).size(12.0).color(MUTED));
                    ui.add_space(4.0);
                    let mut b = s.lighting.brightness;
                    if slider(ui, &mut b, 0..=9, 420.0).changed() {
                        s.lighting.brightness = b;
                    }
                    ui.label(RichText::new(format!("{}%", (s.lighting.brightness as u32 + 1) * 10)).size(12.0).color(TEXT));
                });
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 520.0), Layout::top_down(Align::Min), |ui| {
            // On a short tile the effect card is taller than the page area;
            // scroll it instead of cutting Apply and Save off.
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            card_pad(16).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Effect", "Эффект")).size(15.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("HW {}", effect_by_ui(s.lighting.effect).hw)).size(10.5).color(DIM));
                    });
                });
                ui.add_space(8.0);
                ScrollArea::vertical().max_height(if short { 124.0 } else { 190.0 }).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = if short { 4.0 } else { 5.0 };
                    let chip_w = ((ui.available_width() - 8.0) / 2.0).max(104.0);
                    let mut i = 0;
                    while i < EFFECTS.len() {
                        ui.horizontal(|ui| {
                            for j in 0..2 {
                                if i + j < EFFECTS.len() {
                                    let e = &EFFECTS[i + j];
                                    if effect_chip(ui, e, s.lighting.effect == e.ui, chip_w).clicked() {
                                        s.lighting.effect = e.ui;
                                        if e.ui == 19 {
                                            s.lighting.paint = true;
                                        }
                                    }
                                }
                            }
                        });
                        i += 2;
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Speed", "Скорость")).size(11.5).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut speed = s.lighting.speed;
                        if level_pips(ui, &mut speed, SPEED_LEVELS).clicked() {
                            s.lighting.speed = speed;
                        }
                    });
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Direction", "Направление")).size(11.5).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        for d in [3u8, 2, 1, 0] {
                            if dir_chip(ui, d, s.lighting.direction == d).clicked() {
                                s.lighting.direction = d;
                            }
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Colour", "Цвет")).size(11.5).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut c = s.lighting.color;
                        if ui.color_edit_button_srgb(&mut c).changed() {
                            s.lighting.color = c;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for p in PALETTE.iter() {
                        if swatch(ui, *p, s.lighting.color == *p).clicked() {
                            s.lighting.color = *p;
                            s.lighting.random_color = false;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let mut rnd = s.lighting.random_color;
                    if toggle(ui, &mut rnd).clicked() {
                        s.lighting.random_color = rnd;
                    }
                    ui.label(RichText::new(t(s, "Random colour per key", "Случайный цвет клавиш")).size(11.5).color(MUTED));
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if accent_button(ui, t(s, "Apply to keyboard", "Применить к клавиатуре")).clicked() {
                        apply_to_keyboard(s, v, time as f64);
                    }
                    if ghost_button(ui, t(s, "Save profile", "Сохранить профиль")).clicked() {
                        save_current_profile(s);
                        toast(v, t(s, "Lighting saved to the active profile", "Подсветка сохранена в активный профиль"), time as f64);
                    }
                });
                ui.add_space(4.0);
                let status = if v.devices.is_empty() {
                    t(s, "No keyboard found — connect USB and press a key", "Клавиатура не найдена — подключите USB и нажмите клавишу")
                } else {
                    t(s, "Ready to write over the HID feature report", "Готово к записи через HID feature-отчёт")
                };
                ui.label(RichText::new(status).size(10.5).color(DIM));
                ui.label(RichText::new(t(s, "Effect, per-key colours, brightness, speed, direction, random colour, key remaps and macro assignments are written.", "Эффект, цвета клавиш, яркость, скорость, направление, случайный цвет, переназначения и макросы записываются.")).size(9.5).color(DIM));
            });
            });
        });
    });
}

fn effect_chip(ui: &mut Ui, e: &Effect, selected: bool, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 27.0), Sense::click());
    let hover = resp.hovered();
    let fill = if selected {
        ACCENT_DARK
    } else if hover {
        PANEL_3
    } else {
        PANEL_2
    };
    ui.painter().rect_filled(rect, 7, fill);
    ui.painter().rect_stroke(
        rect,
        7,
        Stroke::new(1.0, if selected { ACCENT } else { LINE_SOFT }),
        StrokeKind::Inside,
    );
    if selected {
        ui.painter()
            .circle_filled(pos2(rect.left() + 10.0, rect.center().y), 3.0, ACCENT);
    }
    ui.painter().text(
        pos2(
            rect.left() + if selected { 18.0 } else { 9.0 },
            rect.center().y,
        ),
        egui::Align2::LEFT_CENTER,
        e.name,
        FontId::new(10.5, FontFamily::Proportional),
        if selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(178, 178, 188)
        },
    );
    resp
}

fn level_pips(ui: &mut Ui, value: &mut u8, count: u8) -> egui::Response {
    let mut any = ui.allocate_response(egui::Vec2::ZERO, Sense::hover());
    ui.horizontal(|ui| {
        for i in 0..count {
            let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 16.0), Sense::click());
            let on = i <= *value;
            let c = if on {
                ACCENT
            } else {
                Color32::from_rgb(58, 58, 65)
            };
            let h = 5.0 + i as f32 * 2.2;
            let bar = egui::Rect::from_center_size(rect.center(), vec2(13.0, h));
            ui.painter().rect_filled(bar, 2, c);
            if resp.clicked() {
                *value = i;
                any = resp;
            }
        }
    });
    any
}

fn dir_chip(ui: &mut Ui, dir: u8, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    let fill = if selected {
        ACCENT_DARK
    } else if resp.hovered() {
        PANEL_3
    } else {
        PANEL_2
    };
    ui.painter().rect_filled(rect, 6, fill);
    ui.painter().rect_stroke(
        rect,
        6,
        Stroke::new(1.0, if selected { ACCENT } else { LINE_SOFT }),
        StrokeKind::Inside,
    );
    let c = rect.center();
    let pts: Vec<egui::Pos2> = match dir {
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
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        if selected { Color32::WHITE } else { MUTED },
        Stroke::NONE,
    ));
    resp
}

fn swatch(ui: &mut Ui, c: [u8; 3], selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(23.0, 23.0), Sense::click());
    ui.painter().circle_filled(rect.center(), 9.5, rgb(c));
    if selected {
        ui.painter()
            .circle_stroke(rect.center(), 11.5, Stroke::new(2.0, Color32::WHITE));
    } else if resp.hovered() {
        ui.painter().circle_stroke(
            rect.center(),
            11.0,
            Stroke::new(1.0, Color32::from_white_alpha(90)),
        );
    }
    resp
}

pub fn save_current_profile(s: &mut AppState) {
    if s.active_profile < s.profiles.len() {
        s.profiles[s.active_profile].data = ProfileData {
            lighting: s.lighting.clone(),
            keys: s.keys.clone(),
        };
        s.profiles[s.active_profile].macros = s.macros.len();
    }
}

pub fn keys_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Key assignment",
        "Назначение клавиш",
        "Remap keys, media controls and FN layer",
        "Переназначение клавиш, мультимедиа и слоя FN",
    );
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 700.0, 352.0);
        ui.allocate_ui_with_layout(vec2(w_left, 520.0), Layout::top_down(Align::Min), |ui| {
            card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(s, "Select a key", "Выберите клавишу"))
                            .size(15.0)
                            .strong(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut fnl = s.fn_layer;
                        if toggle(ui, &mut fnl).clicked() {
                            s.fn_layer = fnl;
                        }
                        ui.label(
                            RichText::new(t(s, "FN layer", "Слой FN"))
                                .size(11.5)
                                .color(if s.fn_layer { ACCENT } else { MUTED }),
                        );
                    });
                });
                ui.add_space(4.0);
                let hit = keyboard::keyboard(ui, s, vec2((w_left - 40.0).max(320.0), 300.0), time, true, true);
                if let Some(k) = hit.clicked {
                    s.active_key = k;
                }
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(
                            s,
                            "Blue dot = key has a custom assignment on this layer",
                            "Синяя точка = у клавиши своё назначение на этом слое",
                        ))
                        .size(10.5)
                        .color(DIM),
                    );
                });
            });
            ui.add_space(12.0);
            card_pad(14).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(s, "Remapped on this layer", "Изменено на этом слое"))
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let count = if s.fn_layer {
                            s.keys.func.iter().filter(|x| x.is_some()).count()
                        } else {
                            s.keys.base.iter().filter(|x| x.is_some()).count()
                        };
                        ui.label(
                            RichText::new(format!("{}", count))
                                .size(13.0)
                                .strong()
                                .color(ACCENT),
                        );
                    });
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let map = if s.fn_layer {
                        &s.keys.func
                    } else {
                        &s.keys.base
                    };
                    let mut shown = 0;
                    for (i, a) in map.iter().enumerate() {
                        if let Some(a) = a {
                            if shown >= 10 {
                                break;
                            }
                            shown += 1;
                            let text = format!("{} -> {}", key_label_for(i), a.action);
                            pill(ui, &text, ACCENT);
                        }
                    }
                    if shown == 0 {
                        ui.label(
                            RichText::new(t(s, "Nothing remapped yet", "Пока ничего не изменено"))
                                .size(11.5)
                                .color(DIM),
                        );
                    }
                });
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 520.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                let key = s.active_key.min(KEY_COUNT - 1);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::hover());
                    ui.painter().rect_filled(rect, 8, PANEL_3);
                    ui.painter()
                        .rect_stroke(rect, 8, Stroke::new(1.0, LINE), StrokeKind::Inside);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        key_label_for(key),
                        FontId::new(
                            if key_label_for(key).len() > 2 {
                                10.0
                            } else {
                                16.0
                            },
                            FontFamily::Proportional,
                        ),
                        TEXT,
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(key_label_for(key)).size(17.0).strong());
                        let layer = if s.fn_layer { "FN layer" } else { "Base layer" };
                        ui.label(
                            RichText::new(format!(
                                "VK 0x{:02X}  ·  LED {}  ·  {}",
                                KEYS[key].vk, KEYS[key].led, layer
                            ))
                            .size(10.5)
                            .color(MUTED),
                        );
                    });
                });
                ui.add_space(8.0);
                Frame::new()
                    .fill(Color32::from_rgb(28, 28, 32))
                    .corner_radius(8)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(t(s, "CURRENT ACTION", "ТЕКУЩЕЕ ДЕЙСТВИЕ"))
                                .size(9.5)
                                .strong()
                                .color(DIM),
                        );
                        let current = s.assignment(key).map(|a| a.action.clone());
                        ui.label(
                            RichText::new(
                                current.unwrap_or_else(|| {
                                    format!("Keyboard · {}", key_label_for(key))
                                }),
                            )
                            .size(13.0)
                            .color(TEXT),
                        );
                    });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, _cat) in KEY_CATEGORIES.iter().enumerate() {
                        if chip(ui, s.action_category == i, &short_cat(i)).clicked() {
                            s.action_category = i;
                            s.action_choice = 0;
                        }
                    }
                });
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .max_height(196.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let acts = actions_for(s.action_category);
                        for (i, a) in acts.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 27.0),
                                Sense::click(),
                            );
                            let sel = s.action_choice == i;
                            if sel {
                                ui.painter().rect_filled(rect, 6, ACCENT_DARK);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, 6, PANEL_2);
                            }
                            ui.painter().text(
                                pos2(rect.left() + 9.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                *a,
                                FontId::new(11.5, FontFamily::Proportional),
                                if sel {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(183, 183, 192)
                                },
                            );
                            if sel {
                                ui.painter().circle_filled(
                                    pos2(rect.right() - 10.0, rect.center().y),
                                    3.0,
                                    ACCENT,
                                );
                            }
                            if resp.clicked() {
                                s.action_choice = i;
                            }
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if accent_button(ui, t(s, "Assign", "Назначить")).clicked() {
                        let action = actions_for(s.action_category)[s
                            .action_choice
                            .min(actions_for(s.action_category).len() - 1)];
                        if s.action_category == 5 {
                            s.set_assignment(key, None);
                            toast(v, format!("{} disabled", key_label_for(key)), time as f64);
                        } else {
                            s.set_assignment(
                                key,
                                Some(Assignment::new(s.action_category as u8, action)),
                            );
                            toast(
                                v,
                                format!("{} -> {}", key_label_for(key), action),
                                time as f64,
                            );
                        }
                    }
                    if ghost_button(ui, t(s, "Reset key", "Сбросить")).clicked() {
                        s.set_assignment(key, None);
                        toast(v, format!("{} restored", key_label_for(key)), time as f64);
                    }
                });
            });
        });
    });
}

fn short_cat(i: usize) -> String {
    match i {
        0 => "Keys".into(),
        1 => "Mouse".into(),
        2 => "Media".into(),
        3 => "Commands".into(),
        4 => "Combo".into(),
        _ => "Disable".into(),
    }
}

pub fn macros_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Macros",
        "Макросы",
        "Record key sequences and replay them from the keyboard",
        "Запись последовательностей клавиш и их воспроизведение",
    );
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 280.0, 772.0);
        ui.allocate_ui_with_layout(vec2(w_left, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(12).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Library", "Библиотека")).size(14.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{}", s.macros.len())).size(11.0).color(DIM));
                    });
                });
                ui.add_space(6.0);
                ScrollArea::vertical().max_height(330.0).auto_shrink([false, false]).show(ui, |ui| {
                    for i in 0..s.macros.len() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
                        let sel = s.macro_selected == i;
                        if sel {
                            ui.painter().rect_filled(rect, 8, ACCENT_DARK);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 8, PANEL_2);
                        }
                        ui.painter().text(pos2(rect.left() + 10.0, rect.top() + 15.0), egui::Align2::LEFT_CENTER, &s.macros[i].name, FontId::new(12.5, FontFamily::Proportional), TEXT);
                        let info = format!("{} steps · {}", s.macros[i].steps.len(), s.macros[i].trigger);
                        ui.painter().text(pos2(rect.left() + 10.0, rect.top() + 35.0), egui::Align2::LEFT_CENTER, info, FontId::new(10.0, FontFamily::Proportional), MUTED);
                        if sel {
                            ui.painter().rect_filled(egui::Rect::from_min_size(pos2(rect.left(), rect.top() + 12.0), vec2(3.0, 30.0)), 2, ACCENT);
                        }
                        if resp.clicked() {
                            s.macro_selected = i;
                            s.recording = false;
                        }
                    }
                });
                ui.add_space(8.0);
                if ghost_button(ui, t(s, "+ New macro", "+ Новый макрос")).clicked() {
                    s.macros.push(Macro {
                        name: format!("Macro {}", s.macros.len() + 1),
                        trigger: "Not assigned".into(),
                        mode: 0,
                        cycles: 1,
                        steps: Vec::new(),
                    });
                    s.macro_selected = s.macros.len() - 1;
                    toast(v, t(s, "Macro created", "Макрос создан"), time as f64);
                }
                ui.add_space(10.0);
                let used: usize = s.macros.iter().map(|m| m.bytes()).sum();
                ui.label(RichText::new(format!("{} / {} bytes", used.min(MACRO_BUFFER), MACRO_BUFFER)).size(10.0).color(DIM));
                ui.add_space(4.0);
                let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                ui.painter().rect_filled(bar, 3, PANEL_3);
                let frac = (used.min(MACRO_BUFFER) as f32 / MACRO_BUFFER as f32).max(0.02);
                ui.painter().rect_filled(egui::Rect::from_min_size(bar.min, vec2(bar.width() * frac, 6.0)), 3, if frac > 0.9 { AMBER } else { ACCENT });
                ui.add_space(6.0);
                ui.label(RichText::new(t(s, "Macro buffer from keyboard profile: 4096 bytes", "Буфер макросов из профиля клавиатуры: 4096 байт")).size(9.5).color(DIM));
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                if s.macros.is_empty() {
                    ui.label(RichText::new(t(s, "No macros yet", "Макросов пока нет")).size(15.0).color(MUTED));
                    return;
                }
                let idx = s.macro_selected.min(s.macros.len() - 1);
                ui.horizontal(|ui| {
                    let mut name = s.macros[idx].name.clone();
                    if ui.add(TextEdit::singleline(&mut name).desired_width(270.0).font(FontId::new(16.0, FontFamily::Proportional))).changed() {
                        s.macros[idx].name = name;
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(t(s, "Trigger", "Триггер")).size(11.0).color(MUTED));
                    egui::ComboBox::from_id_salt("macro_trigger").selected_text(s.macros[idx].trigger.clone()).width(120.0).show_ui(ui, |ui| {
                        for trig in ["Not assigned", "F9", "F10", "F11", "F12", "Mouse 4", "Mouse 5"] {
                            if ui.selectable_label(s.macros[idx].trigger == trig, trig).clicked() {
                                s.macros[idx].trigger = trig.into();
                            }
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, if s.recording { t(s, "Stop", "Стоп") } else { t(s, "Record", "Запись") }).clicked() {
                            if s.recording {
                                s.recording = false;
                                toast(v, t(s, "Recording stopped", "Запись остановлена"), time as f64);
                            } else {
                                s.recording = true;
                                v.record_started = time as f64;
                                toast(v, t(s, "Recording — press keys now", "Запись — нажимайте клавиши"), time as f64);
                            }
                        }
                        if s.recording {
                            ui.label(RichText::new("● REC").size(11.0).color(RED));
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Playback", "Воспроизведение")).size(11.0).color(MUTED));
                    for (i, label) in [(0usize, "Cycle times"), (1, "Until key released"), (2, "Until any key pressed")] {
                        if chip(ui, s.macros[idx].mode as usize == i, label).clicked() {
                            s.macros[idx].mode = i as u8;
                        }
                    }
                    if s.macros[idx].mode == 0 {
                        ui.add_space(4.0);
                        ui.add(egui::DragValue::new(&mut s.macros[idx].cycles).range(1..=999).speed(1.0));
                        ui.label(RichText::new("×").size(11.0).color(MUTED));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if accent_button(ui, t(s, "Play", "Пуск")).clicked() {
                            v.macro_play = Some(time as f64);
                            toast(v, t(s, "Playing macro (preview)", "Воспроизведение макроса (просмотр)"), time as f64);
                        }
                    });
                });
                ui.add_space(10.0);
                section(ui, t(s, "Steps", "Шаги"));
                ui.add_space(4.0);
                Frame::new().fill(Color32::from_rgb(28, 28, 32)).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ScrollArea::vertical().max_height(300.0).auto_shrink([false, false]).show(ui, |ui| {
                        let mut delete: Option<usize> = None;
                        let playhead = v.macro_play.map(|start| ((time as f64 - start) * 2.0) as usize % (s.macros[idx].steps.len().max(1)));
                        for (i, step) in s.macros[idx].steps.iter_mut().enumerate() {
                            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
                            if Some(i) == playhead {
                                ui.painter().rect_filled(rect, 6, Color32::from_rgba_unmultiplied(10, 147, 255, 28));
                            }
                            ui.painter().text(pos2(rect.left() + 12.0, rect.center().y), egui::Align2::CENTER_CENTER, format!("{:02}", i + 1), FontId::new(10.0, FontFamily::Monospace), DIM);
                            let tag = match step.kind { StepKind::Key => "KEY", StepKind::Mouse => "MOUSE", StepKind::Delay => "WAIT", StepKind::Text => "TEXT" };
                            let tag_color = match step.kind { StepKind::Key => ACCENT, StepKind::Mouse => GREEN, StepKind::Delay => AMBER, StepKind::Text => Color32::from_rgb(191, 90, 242) };
                            let hold_label = tr(s.prefs.lang, "Hold", "Пауза");
                            let tag_rect = egui::Rect::from_min_size(pos2(rect.left() + 30.0, rect.center().y - 9.0), vec2(40.0, 18.0));
                            ui.painter().rect_filled(tag_rect, 4, Color32::from_rgba_unmultiplied(tag_color.r(), tag_color.g(), tag_color.b(), 30));
                            ui.painter().text(tag_rect.center(), egui::Align2::CENTER_CENTER, tag, FontId::new(8.0, FontFamily::Proportional), tag_color);
                            let mut x = rect.left() + 80.0;
                            if step.kind == StepKind::Delay {
                                ui.painter().text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, hold_label, FontId::new(11.0, FontFamily::Proportional), MUTED);
                                x += 42.0;
                            } else {
                                ui.painter().text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, &step.text, FontId::new(11.5, FontFamily::Proportional), TEXT);
                                x += 160.0;
                                ui.painter().text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, format!("hold {} ms", step.holding_ms), FontId::new(9.5, FontFamily::Proportional), DIM);
                                x += 105.0;
                            }
                            ui.painter().text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, format!("+{} ms", step.delay_ms), FontId::new(9.5, FontFamily::Proportional), DIM);
                            let del = egui::Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(20.0, 20.0));
                            let dr = ui.interact(del, ui.scope_id().with(("del_step", idx, i)), Sense::click());
                            ui.painter().text(del.center(), egui::Align2::CENTER_CENTER, "×", FontId::new(14.0, FontFamily::Proportional), if dr.hovered() { RED } else { DIM });
                            if dr.clicked() {
                                delete = Some(i);
                            }
                        }
                        if let Some(i) = delete {
                            s.macros[idx].steps.remove(i);
                        }
                        if s.macros[idx].steps.is_empty() {
                            ui.add_space(14.0);
                            ui.label(RichText::new(t(s, "Press Record, then use the keyboard. Steps appear here.", "Нажмите «Запись» и используйте клавиатуру. Шаги появятся здесь.")).size(12.0).color(DIM));
                            ui.add_space(14.0);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ghost_button(ui, t(s, "+ Key", "+ Клавиша")).clicked() {
                        s.macros[idx].steps.push(MacroStep { kind: StepKind::Key, text: "A".into(), delay_ms: 50, holding_ms: 30 });
                    }
                    if ghost_button(ui, t(s, "+ Mouse", "+ Мышь")).clicked() {
                        s.macros[idx].steps.push(MacroStep { kind: StepKind::Mouse, text: "Left click".into(), delay_ms: 50, holding_ms: 20 });
                    }
                    if ghost_button(ui, t(s, "+ Delay", "+ Пауза")).clicked() {
                        s.macros[idx].steps.push(MacroStep { kind: StepKind::Delay, text: String::new(), delay_ms: 100, holding_ms: 0 });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if accent_button(ui, t(s, "Save to profile", "Сохранить в профиль")).clicked() {
                            save_current_profile(s);
                            toast(v, t(s, "Macro saved to the active profile", "Макрос сохранён в активный профиль"), time as f64);
                        }
                    });
                });
            });
        });
    });
}

pub fn music_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Music",
        "Музыка",
        "Rhythm-reactive lighting modes from the keyboard profile",
        "Режимы подсветки под ритм из профиля клавиатуры",
    );
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 320.0, 732.0);
        ui.allocate_ui_with_layout(vec2(w_left, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(14).show(ui, |ui| {
                ui.label(RichText::new(t(s, "Music modes", "Музыкальные режимы")).size(14.0).strong());
                ui.add_space(6.0);
                ScrollArea::vertical().max_height(440.0).auto_shrink([false, false]).show(ui, |ui| {
                    for mode in 1..=11u8 {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
                        let sel = s.music_mode == mode;
                        if sel {
                            ui.painter().rect_filled(rect, 7, ACCENT_DARK);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 7, PANEL_2);
                        }
                        ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, format!("{:02}", mode), FontId::new(10.0, FontFamily::Monospace), if sel { ACCENT } else { DIM });
                        ui.painter().text(pos2(rect.left() + 32.0, rect.center().y), egui::Align2::LEFT_CENTER, format!("Music mode {}", mode), FontId::new(12.0, FontFamily::Proportional), if sel { Color32::WHITE } else { MUTED });
                        if sel {
                            ui.painter().circle_filled(pos2(rect.right() - 12.0, rect.center().y), 3.0, ACCENT);
                        }
                        if resp.clicked() {
                            s.music_mode = mode;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new(t(s, "Modes come from the F75 profile (11 slots). Mode names are not stored in the extracted files.", "Режимы берутся из профиля F75 (11 слотов). Названия режимов отсутствуют в извлечённых файлах.")).size(9.5).color(DIM));
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 560.0), Layout::top_down(Align::Min), |ui| {
            card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Live preview", "Живой предпросмотр")).size(15.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(ui, t(s, "SIMULATED BEAT", "ИМИТАЦИЯ РИТМА"), AMBER);
                    });
                });
                ui.add_space(4.0);
                let mut preview = s.clone();
                preview.lighting.effect = music_effect(s.music_mode);
                preview.lighting.brightness = s.music_sensitivity.min(9);
                preview.lighting.speed = 3;
                keyboard::keyboard(ui, &preview, vec2((w_right - 36.0).max(320.0), 300.0), time * 1.4, false, false);
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(vec2(696.0, 52.0), Sense::hover());
                let bars = 48;
                for i in 0..bars {
                    let x = rect.left() + i as f32 * (rect.width() / bars as f32);
                    let phase = time * 3.0 + i as f32 * 0.37;
                    let h = (0.25 + 0.75 * (0.5 + 0.5 * phase.sin()) * (0.5 + 0.5 * (i as f32 * 0.31).sin())) * 46.0;
                    let color = hsl((i as f32 / bars as f32 * 0.8 + time * 0.05).fract(), 0.8, 0.58);
                    ui.painter().rect_filled(egui::Rect::from_min_size(pos2(x + 1.0, rect.bottom() - h), vec2(rect.width() / bars as f32 - 2.0, h)), 1.5, with_alpha(color, 170));
                }
            });
            ui.add_space(12.0);
            card_pad(16).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t(s, "Sensitivity", "Чувствительность")).size(11.5).color(MUTED));
                        ui.horizontal(|ui| {
                            let mut sens = s.music_sensitivity;
                            if slider(ui, &mut sens, 0..=9, 150.0).changed() {
                                s.music_sensitivity = sens;
                            }
                            ui.label(RichText::new(format!("{}", s.music_sensitivity + 1)).size(11.0).color(TEXT));
                        });
                    });
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t(s, "Colour source", "Источник цвета")).size(11.5).color(MUTED));
                        ui.horizontal(|ui| {
                            for p in PALETTE.iter().take(7) {
                                if swatch(ui, *p, s.lighting.color == *p).clicked() {
                                    s.lighting.color = *p;
                                }
                            }
                        });
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if accent_button(ui, t(s, "Save music mode", "Сохранить режим")).clicked() {
                            save_current_profile(s);
                            toast(v, t(s, "Music mode saved", "Музыкальный режим сохранён"), time as f64);
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(RichText::new(t(s, "The keyboard reacts on its own; the driver only stores the mode and sensitivity. Audio capture is not needed on Linux.", "Клавиатура реагирует сама; драйвер лишь сохраняет режим и чувствительность. Захват звука в Linux не нужен.")).size(10.0).color(DIM));
            });
        });
    });
}

fn music_effect(mode: u8) -> usize {
    match mode {
        1 => 11,
        2 => 13,
        3 => 12,
        4 => 7,
        5 => 6,
        6 => 15,
        7 => 16,
        8 => 17,
        9 => 18,
        10 => 5,
        _ => 8,
    }
}

pub fn profiles_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Profiles",
        "Профили",
        "Five local slots, import and export",
        "Пять локальных слотов, импорт и экспорт",
    );
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        if ghost_button(ui, t(s, "Import profile", "Импорт профиля")).clicked() {
            v.file_dialog = Some(false);
            v.import_name.clear();
        }
        if ghost_button(
            ui,
            t(s, "Export active profile", "Экспорт активного профиля"),
        )
        .clicked()
        {
            v.file_dialog = Some(true);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(t(
                    s,
                    "Stored locally · no account",
                    "Хранится локально · без аккаунта",
                ))
                .size(10.5)
                .color(DIM),
            );
        });
    });
    ui.add_space(10.0);
    ScrollArea::vertical().max_height(510.0).auto_shrink([false, false]).show(ui, |ui| {
        // Three profile cards per row on a wide window, narrower cards on a
        // tiled half-screen so the third column is never cut off.
        let card_w = ((ui.available_width() - 20.0) / 3.0).max(238.0);
        let card_h = if card_w < 320.0 { 226.0 } else { 190.0 };
        let mut activate: Option<usize> = None;
        let mut copy: Option<usize> = None;
        let mut rename: Option<usize> = None;
        let mut delete: Option<usize> = None;
        for row in 0..2 {
            ui.horizontal_top(|ui| {
                for col in 0..3 {
                    let i = row * 3 + col;
                    if i >= s.profiles.len() {
                        if i == 5 {
                            ui.allocate_ui_with_layout(vec2(card_w, card_h), Layout::top_down(Align::Min), |ui| {
                                Frame::new().fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, LINE)).corner_radius(12).inner_margin(Margin::same(18)).show(ui, |ui| {
                                    ui.set_min_size(vec2(card_w - 36.0, card_h - 36.0));
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(46.0);
                                        ui.label(RichText::new("+").size(28.0).color(DIM));
                                        ui.label(RichText::new(t(s, "Slot reserved for a future profile", "Слот для будущего профиля")).size(11.0).color(DIM));
                                    });
                                });
                            });
                        } else if row == 1 {
                            ui.allocate_ui_with_layout(vec2(card_w, card_h), Layout::top_down(Align::Min), |ui| {
                                card().show(ui, |ui| {
                                    ui.set_min_size(vec2(card_w - 36.0, card_h - 36.0));
                                    ui.label(RichText::new(t(s, "Where profiles live", "Где хранятся профили")).size(14.0).strong());
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(t(s, "Profiles are written to your home folder as small text files. Nothing is uploaded.", "Профили сохраняются в домашней папке небольшими текстовыми файлами. Ничего не отправляется в сеть.")).size(11.0).color(MUTED));
                                    ui.add_space(10.0);
                                    ui.label(RichText::new("~/.config/f75-control/profiles").size(10.0).color(DIM));
                                });
                            });
                        }
                        continue;
                    }
                    let active = s.active_profile == i;
                    let p = &s.profiles[i];
                    ui.allocate_ui_with_layout(vec2(card_w, card_h), Layout::top_down(Align::Min), |ui| {
                        let frame = Frame::new()
                            .fill(if active { Color32::from_rgb(30, 43, 55) } else { PANEL })
                            .stroke(Stroke::new(1.0, if active { ACCENT_SOFT } else { LINE_SOFT }))
                            .corner_radius(12)
                            .inner_margin(Margin::same(16));
                        frame.show(ui, |ui| {
                            ui.set_min_size(vec2(card_w - 32.0, card_h - 32.0));
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                                ui.painter().rect_filled(rect, 8, PANEL_3);
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}", i + 1), FontId::new(14.0, FontFamily::Proportional), if active { ACCENT } else { MUTED });
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&p.name).size(14.5).strong());
                                    ui.label(RichText::new(if p.built_in { "Factory profile" } else { "Local profile" }).size(9.5).color(DIM));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if active {
                                        pill(ui, t(s, "ACTIVE", "АКТИВЕН"), ACCENT);
                                    }
                                });
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(9.0, 9.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.5, rgb(p.data.lighting.color));
                                ui.label(RichText::new(format!("{} · {}% · {} macros", effect_by_ui(p.data.lighting.effect).name, (p.data.lighting.brightness as u32 + 1) * 10, p.macros)).size(10.5).color(MUTED));
                            });
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                if !active && accent_button(ui, t(s, "Activate", "Активировать")).clicked() {
                                    activate = Some(i);
                                }
                                if ghost_button(ui, t(s, "Copy", "Копия")).clicked() {
                                    copy = Some(i);
                                }
                                if !p.built_in && ghost_button(ui, t(s, "Rename", "Имя")).clicked() {
                                    rename = Some(i);
                                }
                                if !p.built_in && ghost_button(ui, t(s, "Delete", "Удалить")).clicked() {
                                    delete = Some(i);
                                }
                            });
                        });
                    });
                    if col < 2 {
                        ui.add_space(10.0);
                    }
                }
            });
            ui.add_space(10.0);
        }
        if let Some(i) = activate {
            s.active_profile = i;
            let data = s.profiles[i].data.clone();
            s.lighting = data.lighting;
            s.keys = data.keys;
            toast(v, format!("{} activated", s.profiles[i].name), time as f64);
        }
        if let Some(i) = copy {
            let mut p = s.profiles[i].clone();
            p.name = format!("{} copy", p.name);
            p.built_in = false;
            if s.profiles.len() < 5 {
                s.profiles.push(p);
            } else {
                toast(v, t(s, "All five slots are full", "Все пять слотов заняты"), time as f64);
            }
        }
        if let Some(i) = rename {
            v.rename = Some(i);
            v.rename_buf = s.profiles[i].name.clone();
        }
        if let Some(i) = delete {
            if s.profiles.len() > 1 {
                s.profiles.remove(i);
                if s.active_profile >= s.profiles.len() {
                    s.active_profile = 0;
                }
                toast(v, t(s, "Profile deleted", "Профиль удалён"), time as f64);
            }
        }
    });

    if let Some(i) = v.rename {
        let mut open = true;
        let mut save = false;
        egui::Window::new(t(s, "Rename profile", "Переименовать профиль"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.set_width(280.0);
                ui.add(TextEdit::singleline(&mut v.rename_buf).desired_width(280.0));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if accent_button(ui, t(s, "Save", "Сохранить")).clicked() {
                        save = true;
                    }
                    if ghost_button(ui, t(s, "Cancel", "Отмена")).clicked() {
                        save = false;
                    }
                });
            });
        if save {
            if i < s.profiles.len() && !v.rename_buf.trim().is_empty() {
                s.profiles[i].name = v.rename_buf.trim().to_string();
            }
            v.rename = None;
        } else if !open {
            v.rename = None;
        }
    }
}

pub fn settings_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Settings",
        "Настройки",
        "Language, power, typing and software",
        "Язык, питание, ввод и программа",
    );
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 530.0, 522.0);
        ui.allocate_ui_with_layout(vec2(w_left, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                ui.label(RichText::new(t(s, "Language & appearance", "Язык и оформление")).size(14.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Language", "Язык")).size(12.0).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if chip(ui, s.prefs.lang == Lang::Ru, "Русский").clicked() {
                            s.prefs.lang = Lang::Ru;
                        }
                        if chip(ui, s.prefs.lang == Lang::En, "English").clicked() {
                            s.prefs.lang = Lang::En;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Theme", "Тема")).size(12.0).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(ui, t(s, "DARK", "ТЁМНАЯ"), MUTED);
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Accent", "Акцент")).size(12.0).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 8.0, ACCENT);
                        ui.label(RichText::new("#0A93FF").size(10.5).color(DIM));
                    });
                });
                ui.add_space(10.0);
                divider(ui);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Remember last page", "Запоминать последнюю страницу")).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut x = s.prefs.remember_page;
                        if toggle(ui, &mut x).clicked() { s.prefs.remember_page = x; }
                    });
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Show FN hints on keys", "Показывать подсказки FN")).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut x = s.prefs.show_fn_tip;
                        if toggle(ui, &mut x).clicked() { s.prefs.show_fn_tip = x; }
                    });
                });
            });
            ui.add_space(12.0);
            card_pad(16).show(ui, |ui| {
                ui.label(RichText::new(t(s, "Power & typing", "Питание и ввод")).size(14.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Sleep after", "Сон через")).size(12.0)).on_hover_text(t(s, "SleepTime and SleepUI from the keyboard profile", "SleepTime и SleepUI из профиля клавиатуры"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("sleep").selected_text(format!("{} s", SLEEP_UI[s.prefs.sleep_index.min(8)])).width(92.0).show_ui(ui, |ui| {
                            for (i, secs) in SLEEP_UI.iter().enumerate() {
                                if ui.selectable_label(s.prefs.sleep_index == i, format!("{} s", secs)).clicked() {
                                    s.prefs.sleep_index = i;
                                }
                            }
                        });
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Device value: {} × 30 s", SLEEP_HW[s.prefs.sleep_index.min(8)])).size(10.0).color(DIM));
                });
                ui.add_space(6.0);
                ui.label(RichText::new(t(s, "Debounce", "Антидребезг")).size(12.0).color(MUTED));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (i, ms) in DEBOUNCE_MS.iter().enumerate() {
                        if chip(ui, s.prefs.debounce as usize == i, &format!("{} ms", ms)).clicked() {
                            s.prefs.debounce = i as u8;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new(t(s, "Stage values estimated from the packed ShowDebounce field (0x04000101).", "Значения ступеней оценены по упакованному полю ShowDebounce (0x04000101).")).size(9.5).color(DIM));
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                ui.label(RichText::new(t(s, "Application", "Приложение")).size(14.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Start with the system", "Запускать с системой")).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut x = s.prefs.autostart;
                        if toggle(ui, &mut x).clicked() {
                            s.prefs.autostart = x;
                            let msg = if x { t(s, "Autostart entry will be created on Save", "Автозапуск будет создан при сохранении") } else { t(s, "Autostart disabled", "Автозапуск отключён") };
                            toast(v, msg, time as f64);
                        }
                    });
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Closing the window", "Закрытие окна")).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("close").selected_text(if s.prefs.close_to_tray { t(s, "Keep in tray", "Свернуть в трей") } else { t(s, "Exit", "Выход") }).width(120.0).show_ui(ui, |ui| {
                            if ui.selectable_label(!s.prefs.close_to_tray, t(s, "Exit", "Выход")).clicked() { s.prefs.close_to_tray = false; }
                            if ui.selectable_label(s.prefs.close_to_tray, t(s, "Keep in tray", "Свернуть в трей")).clicked() { s.prefs.close_to_tray = true; }
                        });
                    });
                });
                ui.add_space(10.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(RichText::new(t(s, "Software version", "Версия программы")).size(12.0).color(MUTED));
                ui.label(RichText::new(format!("{}  ·  build 2026.11", APP_VERSION)).size(12.0));
                ui.add_space(6.0);
                ui.label(RichText::new(t(s, "Firmware update", "Обновление прошивки")).size(12.0).color(MUTED));
                ui.add_space(3.0);
                ui.label(RichText::new(t(s, "Offline panel: this open build does not contact an update server.", "Офлайн-панель: эта открытая сборка не обращается к серверу обновлений.")).size(10.5).color(DIM));
                ui.add_space(6.0);
                if ghost_button(ui, t(s, "Check installed firmware", "Проверить прошивку")).clicked() {
                    toast(v, format!("Keyboard profile reports firmware {}", FIRMWARE), time as f64);
                }
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Logging", "Журнал")).size(12.5).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, t(s, "Open log", "Открыть журнал")).clicked() {
                            s.page = 7;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (i, name) in [
                        t(s, "Off", "Выкл"),
                        t(s, "Errors", "Ошибки"),
                        t(s, "Normal", "Обычный"),
                        t(s, "Debug", "Отладка"),
                    ]
                    .iter()
                    .enumerate()
                    {
                        if chip(ui, s.prefs.log_level as usize == i, name).clicked() {
                            s.prefs.log_level = i as u8;
                            log::set_level(i as u8);
                            log::info("log", format!("level changed from Settings to {i}"));
                            toast(v, t(s, "Log level changed", "Уровень журнала изменён"), time as f64);
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(s, "Write Log.txt", "Писать Log.txt")).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut x = s.prefs.log_to_file;
                        if toggle(ui, &mut x).clicked() {
                            s.prefs.log_to_file = x;
                            log::set_file_enabled(x);
                        }
                    });
                });
            });
            ui.add_space(12.0);
            card_pad(16).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t(s, "Reset everything", "Сбросить всё")).size(14.0).strong().color(RED));
                        ui.label(RichText::new(t(s, "Restores lighting, keys, macros and settings to the defaults.", "Возвращает подсветку, клавиши, макросы и настройки к значениям по умолчанию.")).size(10.5).color(MUTED));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if v.confirm_reset {
                            if ghost_button(ui, t(s, "Yes, reset", "Да, сбросить")).clicked() {
                                *s = AppState::default();
                                v.confirm_reset = false;
                                log::set_level(s.prefs.log_level);
                                log::set_file_enabled(s.prefs.log_to_file);
                                log::warn("app", "all settings were reset to defaults");
                                toast(v, t(s, "Settings reset", "Настройки сброшены"), time as f64);
                            }
                        } else if ghost_button(ui, t(s, "Reset", "Сбросить")).clicked() {
                            v.confirm_reset = true;
                        }
                    });
                });
            });
        });
    });
}

pub fn access_ready(v: &UiState) -> bool {
    if v.access.iter().any(|a| a.control) {
        v.access.iter().any(|a| a.control && a.writable)
    } else {
        v.access.iter().any(|a| a.writable)
    }
}

pub fn refresh_access(v: &mut UiState) {
    v.devices = crate::device::detect();
    v.access = crate::device::check_access();
    v.access_checked = true;
    v.probe.clear();
    v.probe_checked = false;
    let writable = v.access.iter().filter(|a| a.writable).count();
    log::debug(
        "access",
        format!(
            "refresh: {} device node(s), {} writable",
            v.devices.len(),
            writable
        ),
    );
}

fn access_card(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    let russian = s.prefs.lang == Lang::Ru;
    let busy = v.busy();
    card_pad(15).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t(s, "Device access", "Доступ к устройству"))
                    .size(13.5)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, color) = if busy {
                    (t(s, "WORKING", "РАБОТАЮ"), ACCENT)
                } else if v.devices.is_empty() {
                    (t(s, "NOT FOUND", "НЕ НАЙДЕНА"), AMBER)
                } else if access_ready(v) {
                    (t(s, "READY", "ГОТОВО"), GREEN)
                } else {
                    (t(s, "NO ACCESS", "НЕТ ПРАВ"), AMBER)
                };
                pill(ui, label, color);
            });
        });
        ui.add_space(6.0);
        if v.devices.is_empty() {
            ui.label(RichText::new(t(s, "The keyboard is not visible to Linux. Check the cable, press a key to wake it and press FN+W for Windows mode.", "Linux не видит клавиатуру. Проверьте кабель, нажмите клавишу, чтобы разбудить её, и включите режим Windows сочетанием FN+W.")).size(10.5).color(MUTED));
            ui.add_space(6.0);
            if ghost_button(ui, t(s, "Rescan USB", "Проверить USB")).clicked() {
                refresh_access(v);
                let msg = if v.devices.is_empty() {
                    t(s, "Still no F75 hidraw device", "hidraw-устройство F75 всё ещё не найдено")
                } else {
                    t(s, "Keyboard found", "Клавиатура найдена")
                };
                toast(v, msg, time as f64);
            }
            return;
        }
        for a in &v.access {
            ui.horizontal(|ui| {
                let role = if a.control {
                    t(s, "control", "управление")
                } else {
                    t(s, "keyboard", "клавиатура")
                };
                ui.label(
                    RichText::new(format!("{}  ·  {role}", a.path))
                        .size(10.0)
                        .monospace()
                        .color(MUTED),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if a.writable {
                        ui.label(
                            RichText::new(t(s, "access allowed", "доступ есть"))
                                .size(9.5)
                                .color(GREEN),
                        );
                    } else {
                        let label = if a.denied {
                            format!("{} {}", t(s, "no permission", "нет прав"), a.mode)
                        } else {
                            format!("{} ({})", t(s, "cannot open", "не открывается"), a.os_error)
                        };
                        ui.label(RichText::new(label).size(9.5).color(AMBER));
                    }
                });
            });
        }
        ui.add_space(4.0);

        if access_ready(v) {
            ui.label(RichText::new(t(s, "Access is allowed: the app can read and write settings. Applying lighting still needs the keyboard to answer (USB is the reliable link).", "Доступ есть: программа может читать и записывать настройки. Для применения подсветки клавиатура ещё должна ответить (надёжнее по USB).")).size(10.5).color(MUTED));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if accent_button(ui, t(s, "Check connection", "Проверить связь")).clicked() {
                        v.probe.clear();
                        v.probe_checked = false;
                        v.job = Some(Job {
                            label: if russian {
                                "Проверяю связь с клавиатурой".into()
                            } else {
                                "Testing the keyboard connection".into()
                            },
                            started: time as f64,
                            rx: crate::device::spawn_probe(),
                        });
                        toast(
                            v,
                            if russian {
                                "Проверяю. Запрос может ждать ответа до 5 секунд; окно не замирает."
                            } else {
                                "Testing. A request may wait up to 5 seconds; the window stays responsive."
                            },
                            time as f64,
                        );
                    }
                });
                ui.add_space(6.0);
                if ghost_button(ui, t(s, "Check access", "Проверить доступ")).clicked() {
                    refresh_access(v);
                    toast(
                        v,
                        if access_ready(v) {
                            t(s, "Access confirmed", "Доступ подтверждён")
                        } else {
                            t(s, "Access disappeared — replug the keyboard", "Доступ пропал — переподключите клавиатуру")
                        },
                        time as f64,
                    );
                }
            });
            if v.probe_checked {
                ui.add_space(8.0);
                let verdict = crate::device::probe_verdict(&v.probe, russian);
                let good = v.probe.iter().any(|p| p.profile_ok);
                Frame::new()
                    .fill(if good {
                        Color32::from_rgb(22, 40, 30)
                    } else {
                        Color32::from_rgb(44, 36, 20)
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if good {
                            Color32::from_rgb(42, 92, 62)
                        } else {
                            Color32::from_rgb(92, 70, 32)
                        },
                    ))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(verdict)
                                .size(10.5)
                                .color(if good { GREEN } else { AMBER }),
                        );
                    });
                ui.add_space(6.0);
                for p in &v.probe {
                    let read = if p.read_ok {
                        t(s, "read ok", "чтение ок")
                    } else {
                        t(s, "read failed", "чтение не прошло")
                    };
                    let request = if !p.control {
                        t(s, "no settings channel", "нет канала настроек").to_string()
                    } else if p.request_ok {
                        format!(
                            "{} {} ms",
                            t(s, "settings request ok in", "запрос настроек прошёл за"),
                            p.request_ms
                        )
                    } else {
                        format!(
                            "{} {} ms · {}",
                            t(s, "settings request failed after", "запрос настроек отклонён через"),
                            p.request_ms,
                            p.request_error
                        )
                    };
                    ui.label(
                        RichText::new(format!("{} · {read} · {request}", p.path))
                            .size(9.5)
                            .monospace()
                            .color(MUTED),
                    );
                }
            } else {
                ui.add_space(4.0);
                ui.label(RichText::new(t(s, "The check only reads the device and sends the harmless profile-read request; it does not change any setting.", "Проверка только читает устройство и отправляет безопасный запрос чтения профиля; настройки она не меняет.")).size(9.0).color(DIM));
            }
        } else {
            let state = crate::device::rule_state();
            let reason = match state {
                crate::device::RuleState::LegacyPath => t(
                    s,
                    "The old rule is in 99-f75.rules. udev reads that file after 73-seat-late.rules, so the uaccess tag arrives too late and no ACL is created — that is exactly why the command you ran did not help. The new rule goes to 70-f75.rules and also names your group.",
                    "Старое правило лежит в 99-f75.rules. udev читает его позже 73-seat-late.rules, поэтому тег uaccess не успевает сработать и ACL не появляется — именно поэтому ваша команда не помогла. Новое правило записывается в 70-f75.rules и вдобавок указывает вашу группу.",
                ),
                crate::device::RuleState::NoGroup => t(
                    s,
                    "The rule file is present but has no GROUP= line. MODE=0660 then keeps the node owned by root, so your account still cannot open it.",
                    "Файл правила есть, но в нём нет строки GROUP=. Режим 0660 оставляет устройство за root, и ваша учётная запись по-прежнему не может его открыть.",
                ),
                crate::device::RuleState::WrongGroup => t(
                    s,
                    "The rule names a different user group, so it does not cover your account.",
                    "В правиле указана другая группа пользователей, поэтому вашей учётной записи оно не помогает.",
                ),
                crate::device::RuleState::Missing => t(
                    s,
                    "There is no access rule for the keyboard yet.",
                    "Правила доступа к клавиатуре пока нет.",
                ),
                crate::device::RuleState::Current => t(
                    s,
                    "The rule with your group is already written, but access has not been applied yet. Run the fix once more, or replug the keyboard.",
                    "Правило с вашей группой уже записано, но доступ ещё не применился. Запустите исправление ещё раз или переподключите клавиатуру.",
                ),
            };
            Frame::new()
                .fill(Color32::from_rgb(44, 36, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(92, 70, 32)))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.label(RichText::new(reason).size(10.5).color(AMBER));
                });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if accent_button(ui, t(s, "Allow access now", "Дать доступ сейчас")).clicked() {
                        if !crate::device::pkexec_available() {
                            v.install_result = Some((
                                t(
                                    s,
                                    "pkexec is not installed. Copy the command below instead.",
                                    "pkexec не установлен. Скопируйте команду ниже.",
                                )
                                .to_string(),
                                false,
                            ));
                        } else {
                            v.install_result = None;
                            v.job = Some(Job {
                                label: if russian {
                                    "Жду пароль администратора".into()
                                } else {
                                    "Waiting for the administrator password".into()
                                },
                                started: time as f64,
                                rx: crate::device::spawn_install(),
                            });
                            toast(
                                v,
                                if russian {
                                    "Открываю системное окно. Введите пароль администратора — программа его не видит и не хранит."
                                } else {
                                    "Opening the system prompt. Enter the administrator password — the app never sees or stores it."
                                },
                                time as f64,
                            );
                        }
                    }
                });
                ui.add_space(6.0);
                if ghost_button(ui, t(s, "Copy command", "Скопировать команду")).clicked() {
                    // eframe's "clipboard" feature is enabled in Cargo.toml;
                    // without it egui keeps the text in a private in-app
                    // buffer and the terminal receives nothing.
                    ui.ctx().copy_text(crate::device::install_command());
                    toast(
                        v,
                        t(
                            s,
                            "Command copied (with GROUP= this time). Paste it into a terminal with Ctrl+Shift+V and press Enter.",
                            "Команда скопирована (теперь с GROUP=). Вставьте её в терминал через Ctrl+Shift+V и нажмите Enter.",
                        ),
                        time as f64,
                    );
                }
                ui.add_space(6.0);
                if ghost_button(ui, t(s, "Save as script", "Сохранить скриптом")).clicked() {
                    let msg = match crate::device::save_install_script() {
                        Ok(path) => format!(
                            "{} {}",
                            t(s, "Script saved:", "Скрипт сохранён:"),
                            path.display()
                        ),
                        Err(error) => format!(
                            "{} {error}",
                            t(s, "Could not save the script:", "Не удалось сохранить скрипт:")
                        ),
                    };
                    toast(v, msg, time as f64);
                }
                ui.add_space(6.0);
                if ghost_button(ui, t(s, "Check access", "Проверить доступ")).clicked() {
                    refresh_access(v);
                    let msg = if access_ready(v) {
                        t(s, "Access confirmed — apply lighting", "Доступ подтверждён — можно применять подсветку")
                    } else {
                        t(s, "No access yet — run the fix or reconnect the keyboard", "Прав пока нет — запустите исправление или переподключите клавиатуру")
                    };
                    toast(v, msg, time as f64);
                }
            });
            ui.add_space(5.0);
            ui.label(RichText::new(t(s, "The system asks for the administrator password in its own window; the app does not run as root and never stores the password. This writes one udev rule and re-applies it immediately.", "Система сама спросит пароль администратора в своём окне; программа не работает от root и не хранит пароль. Она записывает одно правило udev и сразу применяет его.")).size(9.0).color(DIM));
            ui.add_space(4.0);
            egui::CollapsingHeader::new(t(
                s,
                "Show the terminal command",
                "Показать команду для терминала",
            ))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(RichText::new(t(s, "Use this if the system prompt does not appear. The command must be one line.", "Этот способ — если системное окно с паролем не появилось. Команда должна быть одной строкой.")).size(9.0).color(DIM));
                ui.add_space(3.0);
                Frame::new()
                    .fill(Color32::from_rgb(18, 20, 24))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 62)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(crate::device::install_command())
                                    .monospace()
                                    .size(9.0)
                                    .color(Color32::from_rgb(190, 202, 214)),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                ui.add_space(3.0);
                ui.label(RichText::new(t(s, "If the paste is empty, select the line with the mouse and copy it with Ctrl+C.", "Если вставка пустая, выделите строку мышью и скопируйте через Ctrl+C.")).size(9.0).color(DIM));
            });
        }

        if busy {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0).color(ACCENT));
                ui.label(
                    RichText::new(
                        v.job
                            .as_ref()
                            .map(|j| j.label.clone())
                            .unwrap_or_default(),
                    )
                    .size(10.0)
                    .color(ACCENT),
                );
            });
        }
        if let Some((message, ok)) = v.install_result.clone() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(message)
                    .size(9.5)
                    .color(if ok { GREEN } else { AMBER }),
            );
        }
    });
}

pub fn device_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    if !v.access_checked {
        refresh_access(v);
    }
    page_header(
        ui,
        s,
        "Device",
        "Устройство",
        "What the keyboard reports about itself",
        "Что клавиатура сообщает о себе",
    );
    ui.add_space(12.0);
    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 690.0, 362.0);
        ui.allocate_ui_with_layout(vec2(w_left, 560.0), Layout::top_down(Align::Min), |ui| {
            access_card(ui, s, v, time);
            ui.add_space(10.0);
            card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("F75").size(26.0).strong());
                        ui.label(RichText::new(t(s, "75% wireless mechanical keyboard", "75% беспроводная механическая клавиатура")).size(11.0).color(MUTED));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (label, color) = if v.devices.is_empty() { (t(s, "NOT CONNECTED", "НЕ ПОДКЛЮЧЕНА"), AMBER) } else { (t(s, "DETECTED", "НАЙДЕНА"), GREEN) };
                        pill(ui, label, color);
                    });
                });
                ui.add_space(4.0);
                keyboard::keyboard(ui, s, vec2((w_left - 42.0).max(300.0), 200.0), time, false, false);
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 560.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                ui.label(RichText::new(t(s, "Identity", "Идентификация")).size(14.0).strong());
                ui.add_space(8.0);
                info_row(ui, "Model", "F75 / F75 KR");
                info_row(ui, t(s, "Profile name", "Имя в профиле"), "F75 KR");
                info_row(ui, t(s, "Firmware", "Прошивка"), &format!("{} (0x{:02X})", FIRMWARE, FIRMWARE));
                info_row(ui, t(s, "Layout", "Раскладка"), "75% · 80 keys");
                info_row(ui, t(s, "Matrix", "Матрица"), &format!("{} key slots", MATRIX_LEN));
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(RichText::new(t(s, "USB", "USB")).size(10.0).strong().color(DIM));
                info_row(ui, t(s, "Wired", "Провод"), &format!("{:04X}:{:04X}", VID_WIRED, PID_WIRED));
                info_row(ui, t(s, "Wireless", "Радиоканал"), &format!("{:04X}:{:04X}", VID_WIRELESS, PID_WIRELESS));
                if let Some(d) = v.devices.first() {
                    info_row(ui, t(s, "Port", "Порт"), &d.path);
                } else {
                    info_row(ui, t(s, "Port", "Порт"), "—");
                }
                ui.add_space(4.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(RichText::new(t(s, "Applying settings", "Применение настроек")).size(12.0).strong());
                ui.add_space(3.0);
                if let Some((message, ok)) = v.last_apply.clone() {
                    ui.label(RichText::new(message).size(9.5).color(if ok { GREEN } else { AMBER }));
                    ui.add_space(3.0);
                }
                ui.add_enabled_ui(!v.busy(), |ui| {
                    if accent_button(ui, t(s, "Apply lighting now", "Применить подсветку")).clicked() {
                        apply_to_keyboard(s, v, time as f64);
                    }
                });
                if v.busy() {
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(12.0).color(ACCENT));
                        ui.label(
                            RichText::new(
                                v.job
                                    .as_ref()
                                    .map(|j| j.label.clone())
                                    .unwrap_or_default(),
                            )
                            .size(9.5)
                            .color(ACCENT),
                        );
                    });
                }
                ui.add_space(4.0);
                ui.label(RichText::new(t(s, "1. Press FN+W for Windows mode.  2. Wake the keyboard with a key.  3. USB is most reliable.", "1. Нажмите FN+W для режима Windows.  2. Разбудите клавиатуру клавишей.  3. Надёжнее по USB.")).size(9.0).color(DIM));
            });
        });
    });
}

fn log_color(level: log::Level) -> Color32 {
    match level {
        log::Level::Error => RED,
        log::Level::Warn => AMBER,
        log::Level::Info => ACCENT,
        log::Level::Debug => MUTED,
    }
}

fn log_level_title(lang: Lang, level: u8) -> &'static str {
    match (lang, level) {
        (Lang::Ru, 0) => "Выключен",
        (Lang::Ru, 1) => "Только ошибки",
        (Lang::Ru, 2) => "Обычный",
        (Lang::Ru, _) => "Отладка",
        (Lang::En, 0) => "Off",
        (Lang::En, 1) => "Errors only",
        (Lang::En, 2) => "Normal",
        (Lang::En, _) => "Debug",
    }
}

fn log_row(ui: &mut Ui, entry: &log::Entry) {
    let color = log_color(entry.level);
    let tint = match entry.level {
        log::Level::Error => Color32::from_rgb(46, 28, 32),
        log::Level::Warn => Color32::from_rgb(46, 38, 24),
        _ => PANEL_2,
    };
    Frame::new()
        .fill(tint)
        .stroke(Stroke::new(1.0, LINE_SOFT))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&entry.at).monospace().size(10.0).color(DIM));
                ui.label(
                    RichText::new(entry.level.label())
                        .monospace()
                        .size(10.0)
                        .strong()
                        .color(color),
                );
                ui.label(
                    RichText::new(&entry.module)
                        .monospace()
                        .size(10.0)
                        .color(MUTED),
                );
            });
            ui.add(
                egui::Label::new(
                    RichText::new(&entry.message)
                        .monospace()
                        .size(10.5)
                        .color(TEXT),
                )
                .wrap(),
            );
        });
}

pub fn log_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Log",
        "Журнал",
        "What the app did, what failed and why",
        "Что делала программа, что не получилось и почему",
    );
    ui.add_space(10.0);

    let counts = log::counts();
    ui.horizontal(|ui| {
        pill(
            ui,
            &format!("{} {}", counts[0], t(s, "ERRORS", "ОШИБКИ")),
            RED,
        );
        pill(
            ui,
            &format!("{} {}", counts[1], t(s, "WARNINGS", "ПРЕДУПРЕЖДЕНИЯ")),
            AMBER,
        );
        pill(
            ui,
            &format!("{} {}", counts[2], t(s, "INFO", "ИНФО")),
            ACCENT,
        );
        pill(
            ui,
            &format!("{} {}", counts[3], t(s, "DEBUG", "ОТЛАДКА")),
            MUTED,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(t(
                    s,
                    "Newest first · latest 300 shown",
                    "Сначала новые · показаны последние 300",
                ))
                .size(9.5)
                .color(DIM),
            );
        });
    });
    ui.add_space(8.0);

    ui.horizontal_top(|ui| {
        let (w_left, w_right) = columns(ui, 746.0, 320.0);
        ui.allocate_ui_with_layout(vec2(w_left, 548.0), Layout::top_down(Align::Min), |ui| {
            card_pad(12).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, label) in [
                        (0u8, t(s, "All", "Все")),
                        (1u8, t(s, "Errors", "Ошибки")),
                        (2u8, t(s, "Warnings+", "Предупр.+")),
                        (3u8, t(s, "Info+", "Инфо+")),
                    ] {
                        if chip(ui, v.log_filter == i, label).clicked() {
                            v.log_filter = i;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            TextEdit::singleline(&mut v.log_search)
                                .hint_text(t(s, "Search", "Поиск"))
                                .desired_width(180.0),
                        );
                    });
                });
                ui.add_space(7.0);

                let needle = v.log_search.trim().to_lowercase();
                let entries: Vec<log::Entry> = log::snapshot()
                    .into_iter()
                    .filter(|e| match v.log_filter {
                        1 => e.level == log::Level::Error,
                        2 => (e.level as u8) <= 1,
                        3 => (e.level as u8) <= 2,
                        _ => true,
                    })
                    .filter(|e| {
                        needle.is_empty()
                            || e.message.to_lowercase().contains(&needle)
                            || e.module.to_lowercase().contains(&needle)
                    })
                    .collect();
                if entries.is_empty() {
                    ui.add_space(18.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(t(
                                s,
                                "Nothing here yet. Apply a profile or run a device check.",
                                "Пока пусто. Примените профиль или запустите проверку устройства.",
                            ))
                            .size(11.5)
                            .color(MUTED),
                        );
                    });
                    ui.add_space(18.0);
                }
                ScrollArea::vertical()
                    .id_salt("log_scroll")
                    .auto_shrink([false, false])
                    .max_height(432.0)
                    .show(ui, |ui| {
                        for entry in entries.iter().rev().take(300) {
                            log_row(ui, entry);
                            ui.add_space(3.0);
                        }
                    });
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(w_right, 548.0), Layout::top_down(Align::Min), |ui| {
            card_pad(16).show(ui, |ui| {
                ui.label(
                    RichText::new(t(s, "Log file", "Файл журнала"))
                        .size(14.0)
                        .strong(),
                );
                ui.add_space(5.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(log::log_path().display().to_string())
                            .size(9.5)
                            .color(MUTED),
                    )
                    .wrap(),
                );
                let size = log::file_size();
                ui.label(
                    RichText::new(if size >= 1024 {
                        format!("{:.1} KiB", size as f64 / 1024.0)
                    } else {
                        format!("{} B", size)
                    })
                    .size(10.0)
                    .color(DIM),
                );
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(
                    RichText::new(t(s, "Level", "Уровень"))
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(3.0);
                let mut level = s.prefs.log_level;
                egui::ComboBox::from_id_salt("log_level")
                    .selected_text(log_level_title(s.prefs.lang, level))
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for option in 0..LOG_LEVEL_COUNT as u8 {
                            if ui
                                .selectable_label(
                                    level == option,
                                    log_level_title(s.prefs.lang, option),
                                )
                                .clicked()
                            {
                                level = option;
                            }
                        }
                    });
                if level != s.prefs.log_level {
                    s.prefs.log_level = level;
                    log::set_level(level);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(s, "Write to disk", "Писать на диск")).size(12.0),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut x = s.prefs.log_to_file;
                        if toggle(ui, &mut x).clicked() {
                            s.prefs.log_to_file = x;
                            log::set_file_enabled(x);
                        }
                    });
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ghost_button(ui, t(s, "Copy", "Копировать")).clicked() {
                        let filtered: Vec<log::Entry> = log::snapshot()
                            .into_iter()
                            .filter(|e| match v.log_filter {
                                1 => e.level == log::Level::Error,
                                2 => (e.level as u8) <= 1,
                                3 => (e.level as u8) <= 2,
                                _ => true,
                            })
                            .collect();
                        ui.ctx().copy_text(log::text(&filtered));
                        v.log_notice = Some(format!(
                            "{} {}",
                            filtered.len(),
                            t(s, "entries copied", "записей скопировано")
                        ));
                    }
                    if ghost_button(ui, t(s, "Export report", "Отчёт")).clicked() {
                        v.log_notice = Some(match log::export_diagnostics() {
                            Ok(path) => format!(
                                "{} {}",
                                t(s, "Saved", "Сохранено"),
                                path.display()
                            ),
                            Err(err) => format!(
                                "{}: {err}",
                                t(s, "Could not save the report", "Не удалось сохранить отчёт")
                            ),
                        });
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ghost_button(ui, t(s, "Folder", "Папка"))
                        .on_hover_text(t(s, "Open the folder with the log", "Открыть папку с журналом"))
                        .clicked()
                    {
                        let dir = log::dir();
                        let _ = std::fs::create_dir_all(&dir);
                        if let Err(err) = std::process::Command::new("xdg-open")
                            .arg(&dir)
                            .spawn()
                        {
                            v.log_notice = Some(format!("xdg-open: {err}"));
                        }
                    }
                    if v.log_confirm_clear {
                        if ghost_button(ui, t(s, "Yes, clear", "Да, очистить")).clicked() {
                            log::clear();
                            v.log_confirm_clear = false;
                            v.log_notice =
                                Some(t(s, "Log cleared", "Журнал очищен").to_string());
                        }
                    } else if ghost_button(ui, t(s, "Clear", "Очистить")).clicked() {
                        v.log_confirm_clear = true;
                    }
                });
                if let Some(notice) = v.log_notice.clone() {
                    ui.add_space(5.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(notice).size(9.5).color(ACCENT),
                        )
                        .wrap(),
                    );
                }
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(t(
                            s,
                            "The current session is kept in memory (2000 lines) and copied to Log.txt. Above 1 MiB the old file becomes Log.1.txt.",
                            "Текущая сессия хранится в памяти (2000 строк) и пишется в Log.txt. Свыше 1 МиБ старый файл становится Log.1.txt.",
                        ))
                        .size(9.0)
                        .color(DIM),
                    )
                    .wrap(),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(t(
                            s,
                            "Windows starts its Log.txt at desktop\\Log.txt and truncates it once per launch; rotation and the line limit are our additions.",
                            "Windows пишет Log.txt в папку рабочего стола и очищает его при каждом запуске; ротация и лимит строк добавлены нами.",
                        ))
                        .size(9.0)
                        .color(DIM),
                    )
                    .wrap(),
                );
            });
        });
    });
    let _ = time;
}

pub fn docs_page(ui: &mut Ui, s: &mut AppState, v: &mut UiState, time: f32) {
    page_header(
        ui,
        s,
        "Documentation",
        "Документация",
        "Connect, apply and troubleshoot without reading code",
        "Подключение, применение и решение проблем без чтения кода",
    );
    ui.add_space(14.0);
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let (w_left, w_right) = columns(ui, 660.0, 362.0);
            ui.allocate_ui_with_layout(vec2(w_left, 560.0), Layout::top_down(Align::Min), |ui| {
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "Quick start", "Быстрый старт")).size(14.0).strong());
                    ui.add_space(8.0);
                    doc_point(ui, "1", t(s, "Connect the keyboard by USB (or the 2.4 GHz receiver) and press any key to wake it.", "Подключите клавиатуру по USB (или через приёмник 2,4 ГГц) и нажмите любую клавишу, чтобы разбудить её."));
                    doc_point(ui, "2", t(s, "Press FN + W on the keyboard to select Windows mode. The vendor protocol may not answer in other modes.", "Нажмите FN + W на клавиатуре, чтобы включить режим Windows. В других режимах фирменный протокол может не отвечать."));
                    doc_point(ui, "3", t(s, "Open Device and press Check USB. The F75 name and its /dev/hidraw path should appear.", "Откройте «Устройство» и нажмите «Проверить USB». Должны появиться имя F75 и путь /dev/hidraw."));
                    doc_point(ui, "4", t(s, "Change lighting and press Apply to keyboard. Saving locally only writes a file on this computer.", "Измените подсветку и нажмите «Применить к клавиатуре». Сохранение локально пишет только файл на этом компьютере."));
                });
                ui.add_space(12.0);
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "Applying to the keyboard", "Применение к клавиатуре")).size(14.0).strong());
                    ui.add_space(8.0);
                    doc_point(ui, "•", t(s, "The app reads the 254-byte profile, patches the effect and the custom-colour flag, then saves it back.", "Программа читает 254-байтовый профиль, меняет эффект и флаг своей цветовой матрицы, затем сохраняет его обратно."));
                    doc_point(ui, "•", t(s, "For Self-define it also sends the 1890-byte per-key colour matrix in two chunks, as the vendor driver does.", "Для «Self-define» она дополнительно отправляет 1890-байтовую матрицу цветов двумя посылками, как вендорский драйвер."));
                    doc_point(ui, "•", t(s, "Key remaps and macros are written too: the app reads the live key matrix, overlays changed keys, sends the macro buffer, then saves the profile. Lighting covers effect, per-key colours, brightness, speed, direction and random colour.", "Переназначения клавиш и макросы тоже записываются: программа читает текущую матрицу клавиш, накладывает изменённые клавиши, отправляет буфер макросов и сохраняет профиль. Подсветка — эффект, цвета клавиш, яркость, скорость, направление и случайный цвет."));
                    ui.add_space(4.0);
                    if accent_button(ui, t(s, "Apply lighting now", "Применить подсветку сейчас")).clicked() {
                        apply_to_keyboard(s, v, time as f64);
                    }
                });
                ui.add_space(12.0);
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "If something does not work", "Если что-то не работает")).size(14.0).strong());
                    ui.add_space(8.0);
                    doc_point(ui, "•", t(s, "No keyboard: check USB, wake it with a key, press FN + W, then Rescan USB on Device.", "Нет клавиатуры: проверьте USB, разбудите клавишей, нажмите FN + W, затем «Проверить USB» на странице «Устройство»."));
                    doc_point(ui, "•", t(s, "“No permission” is not a driver fault: Linux lets only root open /dev/hidraw* by default. On Device press “Allow access now” — the system asks for the administrator password once, and the app writes a rule that names your user group.", "«Нет прав» — это не ошибка драйвера: Linux по умолчанию открывает /dev/hidraw* только root. На странице «Устройство» нажмите «Дать доступ сейчас» — система один раз спросит пароль администратора, и программа запишет правило с вашей группой пользователей."));
                    doc_point(ui, "•", t(s, "If the password window does not appear, copy the terminal command from Device; it now contains GROUP=, which the earlier command was missing.", "Если окно с паролем не появилось, скопируйте команду для терминала на странице «Устройство»; теперь в ней есть GROUP=, которого не было в прежней команде."));
                    doc_point(ui, "•", t(s, "Access is fine but the write times out: the 2.4 GHz receiver accepts reads only right now. Connect the USB cable and press Apply again.", "Права есть, но запись обрывается по таймауту: приёмник 2,4 ГГц сейчас принимает только чтение. Подключите USB-кабель и нажмите «Применить» ещё раз."));
                    doc_point(ui, "•", t(s, "Nothing changed: close the vendor app, keep the keyboard awake, use USB, then apply once more.", "Ничего не изменилось: закройте программу производителя, не давайте клавиатуре уснуть, используйте USB и примените ещё раз."));
                    doc_point(ui, "•", t(s, "Command and combination actions, FN-layer remaps and the music visualiser stay in the app — those payloads are not decoded yet.", "Команды, комбинации, слой FN и музыкальный визуализатор остаются в приложении — их посылки ещё не расшифрованы."));
                    doc_point(ui, "•", t(s, "If a step fails, open Log: it lists every device attempt, the exact error and a one-click diagnostics report.", "Если шаг не сработал, откройте «Журнал»: там каждая попытка связи, точная ошибка и отчёт одним нажатием."));
                });
            });
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(vec2(w_right, 560.0), Layout::top_down(Align::Min), |ui| {
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "Written to the keyboard", "Записывается в клавиатуру")).size(14.0).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(t(s, "Effect id · brightness · speed · direction · random colour · base colour · key remaps · macro assignments", "Номер эффекта · яркость · скорость · направление · случайный цвет · базовый цвет · переназначения · макросы")).size(11.0).color(MUTED));
                    ui.add_space(10.0);
                    divider(ui);
                    ui.add_space(8.0);
                    ui.label(RichText::new(t(s, "STAYS IN THE APP", "ОСТАЁТСЯ В ПРОГРАММЕ")).size(9.0).strong().color(DIM));
                    ui.add_space(3.0);
                    ui.label(RichText::new(t(s, "Command/combination actions · FN-layer remaps · music visualiser · firmware panel", "Команды и комбинации · слой FN · музыкальный визуализатор · прошивка")).size(10.5).color(MUTED));
                });
                ui.add_space(12.0);
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "Files", "Файлы")).size(14.0).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new("~/.config/f75-control/state.json").size(10.0).color(DIM));
                    ui.label(RichText::new("~/.config/f75-control/profiles/").size(10.0).color(DIM));
                    ui.label(RichText::new("~/.config/f75-control/Log.txt").size(10.0).color(DIM));
                    ui.add_space(6.0);
                    ui.label(RichText::new(t(s, "Profiles are plain JSON files. Nothing is uploaded; the app needs no network.", "Профили — обычные JSON-файлы. Ничего не отправляется в сеть; программа не требует интернета.")).size(10.5).color(MUTED));
                });
                ui.add_space(12.0);
                card_pad(16).show(ui, |ui| {
                    ui.label(RichText::new(t(s, "More detail", "Подробнее")).size(14.0).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(t(s, "README.md — this guide in full.", "README.md — это руководство целиком.")).size(10.5).color(MUTED));
                    ui.label(RichText::new(t(s, "MECHANICS.md — the recovered protocol and timings.", "MECHANICS.md — разобранный протокол и тайминги.")).size(10.5).color(MUTED));
                    ui.label(RichText::new(t(s, "mechanics.json — 157 values with their sources.", "mechanics.json — 157 значений с источниками.")).size(10.5).color(MUTED));
                    ui.add_space(8.0);
                    pill(ui, t(s, "VERSION 1.0.0", "ВЕРСИЯ 1.0.0"), ACCENT);
                });
            });
        });
    });
}

fn doc_point(ui: &mut Ui, marker: &str, text: &str) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 8.0, ACCENT_DARK);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            marker,
            FontId::new(9.5, FontFamily::Proportional),
            ACCENT,
        );
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(text).size(11.0).color(MUTED)).wrap());
    });
    ui.add_space(5.0);
}

fn info_row(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(11.0).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(11.0).color(TEXT));
        });
    });
    ui.add_space(1.0);
}

pub fn file_dialog(ctx: &egui::Context, s: &mut AppState, v: &mut UiState, time: f64) {
    let Some(export) = v.file_dialog else {
        return;
    };
    let mut open = true;
    let mut action = false;
    egui::Window::new(if export {
        t(s, "Export profile", "Экспорт профиля")
    } else {
        t(s, "Import profile", "Импорт профиля")
    })
    .collapsible(false)
    .resizable(false)
    .open(&mut open)
    .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
    .show(ctx, |ui| {
        ui.set_width(360.0);
        ui.label(
            RichText::new(if export {
                t(
                    s,
                    "File name (saved in ~/.config/f75-control/profiles)",
                    "Имя файла (сохраняется в ~/.config/f75-control/profiles)",
                )
            } else {
                t(
                    s,
                    "File name from ~/.config/f75-control/profiles",
                    "Имя файла из ~/.config/f75-control/profiles",
                )
            })
            .size(10.5)
            .color(MUTED),
        );
        ui.add_space(6.0);
        let buf = if export {
            &mut v.export_name
        } else {
            &mut v.import_name
        };
        ui.add(TextEdit::singleline(buf).desired_width(360.0));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if accent_button(
                ui,
                if export {
                    t(s, "Export", "Экспорт")
                } else {
                    t(s, "Import", "Импорт")
                },
            )
            .clicked()
            {
                action = true;
            }
            if ghost_button(ui, t(s, "Cancel", "Отмена")).clicked() {
                v.file_dialog = None;
            }
        });
    });
    if action {
        if export {
            save_current_profile(s);
            let msg = match export_profile(s, &v.export_name) {
                Ok(path) => {
                    log::info("profile", format!("exported profile to {path}"));
                    format!("Exported to {}", path)
                }
                Err(e) => {
                    log::error("profile", format!("export failed: {e}"));
                    format!("Export failed: {e}")
                }
            };
            toast(v, msg, time);
        } else {
            let msg = match import_profile(s, &v.import_name) {
                Ok(name) => {
                    log::info("profile", format!("imported profile \"{name}\""));
                    format!("Imported {}", name)
                }
                Err(e) => {
                    log::error("profile", format!("import failed: {e}"));
                    format!("Import failed: {e}")
                }
            };
            toast(v, msg, time);
        }
        v.file_dialog = None;
    } else if !open {
        v.file_dialog = None;
    }
}

fn profile_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".config/f75-control/profiles")
}

fn export_profile(s: &AppState, name: &str) -> Result<String, String> {
    let dir = profile_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        safe = "profile".into();
    }
    let path = dir.join(format!("{safe}.json"));
    let text = serde_json::to_string_pretty(&s.profiles[s.active_profile].data)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

fn import_profile(s: &mut AppState, name: &str) -> Result<String, String> {
    let dir = profile_dir();
    let mut safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    if safe.is_empty() {
        return Err("empty file name".into());
    }
    if !safe.ends_with(".json") {
        safe.push_str(".json");
    }
    let path = dir.join(&safe);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: ProfileData = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let name = safe.trim_end_matches(".json").to_string();
    if s.profiles.len() >= 5 {
        return Err("all slots are full".into());
    }
    s.profiles.push(Profile {
        name: name.clone(),
        built_in: false,
        data,
        macros: 0,
    });
    Ok(name)
}
