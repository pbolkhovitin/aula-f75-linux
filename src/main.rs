mod device;
mod headless;
mod keyboard;
mod log;
mod model;
mod pages;
mod theme;

use eframe::egui::{
    self, pos2, vec2, Align, Align2, CentralPanel, Color32, Frame, Id, Layout, Margin, Panel,
    RichText, Sense, Stroke, ViewportBuilder,
};
use model::*;
use pages::UiState;
use std::path::PathBuf;
use theme::*;

struct F75App {
    state: AppState,
    view: UiState,
    frame: u64,
    shot_mode: bool,
    last_record: f64,
    dirty: bool,
    saved_at: f64,
}

fn main() {
    let mut screenshot: Option<PathBuf> = None;
    let mut initial_page: Option<usize> = None;
    let mut initial_lang: Option<Lang> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--screenshot needs a file path");
                    std::process::exit(2);
                }
                screenshot = Some(PathBuf::from(&args[i]));
            }
            "--page" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--page needs a name");
                    std::process::exit(2);
                }
                initial_page = match args[i].as_str() {
                    "lighting" => Some(0),
                    "keys" => Some(1),
                    "macros" => Some(2),
                    "music" => Some(3),
                    "profiles" => Some(4),
                    "settings" => Some(5),
                    "device" => Some(6),
                    "log" | "logs" => Some(7),
                    "docs" => Some(8),
                    other => {
                        eprintln!("unknown page {other}");
                        std::process::exit(2);
                    }
                };
            }
            "--lang" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--lang needs en or ru");
                    std::process::exit(2);
                }
                initial_lang = match args[i].as_str() {
                    "en" => Some(Lang::En),
                    "ru" => Some(Lang::Ru),
                    other => {
                        eprintln!("unknown language {other}");
                        std::process::exit(2);
                    }
                };
            }
            "--help" | "-h" => {
                println!("F75 Control Center\n  --screenshot <file.png>   draw 120 frames, save and exit\n  --page <name>            lighting|keys|macros|music|profiles|settings|device|log|docs\n  --lang <en|ru>           interface language");
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let shot_mode = screenshot.is_some();
    let (mut state, load_problem) = if shot_mode {
        (AppState::default(), None)
    } else {
        load_state()
    };
    if let Some(p) = initial_page {
        state.page = p;
    }
    if let Some(lang) = initial_lang {
        state.prefs.lang = lang;
    }
    if state.page >= 9 {
        state.page = 0;
    }

    // Start the log before building the UI: UiState::default() probes the
    // keyboard and its findings belong in the session file. Screenshot runs
    // keep the ring and stderr but do not truncate a real report.
    log::init(
        log::default_path(),
        state.prefs.log_level,
        state.prefs.log_to_file && !shot_mode,
    );
    log::install_panic_hook();
    if let Some(problem) = load_problem {
        log::warn("app", problem);
    }
    log::info(
        "app",
        if shot_mode {
            "started in screenshot mode"
        } else {
            "started with window"
        },
    );

    let mut app = F75App {
        state,
        view: UiState::default(),
        frame: 0,
        shot_mode,
        last_record: 0.0,
        dirty: false,
        saved_at: 0.0,
    };
    // Real permission state at startup: the top bar can then say NO ACCESS
    // instead of pretending the keyboard is ready just because a node exists.
    pages::refresh_access(&mut app.view);

    if let Some(path) = screenshot {
        log::info("app", format!("rendering screenshot {}", path.display()));
        headless::render(&mut app, &path);
        log::info("app", "screenshot finished");
        return;
    }

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([WINDOW_W, WINDOW_H])
            .with_min_inner_size([1040.0, 680.0])
            .with_title(APP_NAME),
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            theme::setup(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    );
    if let Err(err) = result {
        log::error("app", format!("failed to start the window: {err}"));
        eprintln!("failed to start the window: {err}");
        std::process::exit(1);
    }
    log::info("app", "window closed");
}

impl eframe::App for F75App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.frame += 1;
        self.draw_ui(root, now);

        // Auto-save config a few seconds after the last edit in normal mode.
        if !self.shot_mode && self.dirty && now - self.saved_at > 3.0 {
            self.save(now);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.shot_mode {
            self.persist();
            log::info("app", "settings persisted on exit");
        }
    }
}

impl F75App {
    pub(crate) fn draw_ui(&mut self, root: &mut egui::Ui, now: f64) {
        let ctx = root.ctx().clone();
        pages::poll_jobs(&mut self.state, &mut self.view, now);
        self.top_bar(root, now);
        self.status_bar(root, now);
        self.left_rail(root, now);

        CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin {
                left: 24,
                right: 24,
                top: 16,
                bottom: 12,
            }))
            .show(root, |ui| match self.state.page {
                0 => pages::lighting(ui, &mut self.state, &mut self.view, now as f32),
                1 => pages::keys_page(ui, &mut self.state, &mut self.view, now as f32),
                2 => pages::macros_page(ui, &mut self.state, &mut self.view, now as f32),
                3 => pages::music_page(ui, &mut self.state, &mut self.view, now as f32),
                4 => pages::profiles_page(ui, &mut self.state, &mut self.view, now as f32),
                5 => pages::settings_page(ui, &mut self.state, &mut self.view, now as f32),
                6 => pages::device_page(ui, &mut self.state, &mut self.view, now as f32),
                7 => pages::log_page(ui, &mut self.state, &mut self.view, now as f32),
                _ => pages::docs_page(ui, &mut self.state, &mut self.view, now as f32),
            });

        pages::file_dialog(&ctx, &mut self.state, &mut self.view, now);
        self.toast(&ctx, now);
        self.record(&ctx, now);
        // A worker may finish at any moment; keep repainting while it runs so
        // the spinner moves and the result appears without extra mouse input.
        if self.view.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn top_bar(&mut self, root: &mut egui::Ui, now: f64) {
        Panel::top("topbar")
            .exact_size(64.0)
            .frame(
                Frame::new()
                    .fill(TOP)
                    .stroke(Stroke::new(1.0, LINE_SOFT))
                    .inner_margin(Margin {
                        left: 20,
                        right: 18,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    logo(ui, 34.0);
                    ui.add_space(9.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("F75 CONTROL CENTER")
                                .size(14.5)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new("75% keyboard  ·  USB / 2.4 GHz  ·  Linux")
                                .size(10.0)
                                .color(DIM),
                        );
                    });
                    ui.add_space(24.0);
                    let line =
                        egui::Rect::from_min_size(pos2(ui.cursor().min.x, 18.0), vec2(1.0, 28.0));
                    ui.painter().rect_filled(line, 0u8, LINE_SOFT);
                    ui.add_space(20.0);
                    ui.label(RichText::new("PROFILE").size(9.0).strong().color(DIM));
                    ui.add_space(2.0);
                    let active_name = self
                        .state
                        .profiles
                        .get(self.state.active_profile)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Default".into());
                    let mut chosen = self.state.active_profile;
                    egui::ComboBox::from_id_salt("profile_top")
                        .selected_text(RichText::new(active_name).size(12.5).color(TEXT))
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for (i, p) in self.state.profiles.iter().enumerate() {
                                if ui
                                    .selectable_label(self.state.active_profile == i, &p.name)
                                    .clicked()
                                {
                                    chosen = i;
                                }
                            }
                        });
                    if chosen != self.state.active_profile {
                        self.activate_profile(chosen);
                        pages::toast(&mut self.view, "Profile switched", now);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let lang = self.state.prefs.lang;
                        let busy = self.view.busy();
                        ui.add_enabled_ui(!busy, |ui| {
                            let label = if busy {
                                tr(lang, "Applying…", "Применяю…")
                            } else {
                                tr(lang, "Apply to keyboard", "Применить к клавиатуре")
                            };
                            if accent_button(ui, label).clicked() {
                                pages::apply_to_keyboard(&mut self.state, &mut self.view, now);
                            }
                        });
                        ui.add_space(8.0);
                        if ghost_button(ui, tr(lang, "Save locally", "Сохранить локально")).clicked() {
                            self.save(now);
                            pages::toast(
                                &mut self.view,
                                tr(
                                    lang,
                                    "Profile and settings saved locally",
                                    "Профиль и настройки сохранены на компьютере",
                                ),
                                now,
                            );
                        }
                        ui.add_space(8.0);
                        if ghost_button(ui, tr(lang, "Docs", "Справка")).clicked() {
                            self.state.page = 8;
                        }
                        ui.add_space(8.0);
                        if !self.view.devices.is_empty() && !pages::access_ready(&self.view) {
                            if ghost_button(ui, tr(lang, "Fix access", "Дать доступ")).clicked() {
                                self.state.page = 6;
                                pages::toast(
                                    &mut self.view,
                                    tr(
                                        lang,
                                        "Press “Allow access now” on this page",
                                        "Нажмите «Дать доступ сейчас» на этой странице",
                                    ),
                                    now,
                                );
                            }
                            ui.add_space(8.0);
                        }
                        let (text, color) = if self.view.devices.is_empty() {
                            (tr(lang, "NO KEYBOARD", "НЕТ КЛАВИАТУРЫ"), AMBER)
                        } else if !pages::access_ready(&self.view) {
                            (tr(lang, "NO ACCESS", "НЕТ ПРАВ"), AMBER)
                        } else {
                            (tr(lang, "READY", "ГОТОВО"), GREEN)
                        };
                        pill(ui, text, color);
                        ui.add_space(8.0);
                        let (r, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, color);
                    });
                });
            });
    }

    fn left_rail(&mut self, root: &mut egui::Ui, now: f64) {
        Panel::left("rail")
            .exact_size(74.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(RAIL)
                    .stroke(Stroke::new(1.0, LINE_SOFT))
                    .inner_margin(Margin {
                        left: 14,
                        right: 14,
                        top: 16,
                        bottom: 12,
                    }),
            )
            .show(root, |ui| {
                let items = [
                    (NavIcon::Bulb, "Lighting", "Lighting"),
                    (NavIcon::Keyboard, "Key assignment", "Keys"),
                    (NavIcon::Macro, "Macros", "Macros"),
                    (NavIcon::Music, "Music", "Music"),
                    (NavIcon::People, "Profiles", "Profiles"),
                    (NavIcon::Sliders, "Settings", "Settings"),
                    (NavIcon::Device, "Device", "Device"),
                    (NavIcon::Log, "Log", "Log"),
                    (NavIcon::Book, "Documentation", "Docs"),
                ];
                for (i, (icon, label, _short)) in items.iter().enumerate() {
                    if nav_button(ui, *icon, self.state.page == i, label).clicked() {
                        self.state.page = i;
                        self.dirty = true;
                    }
                    ui.add_space(6.0);
                    if i == 3 || i == 5 {
                        ui.add_space(2.0);
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(2.0);
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(
                        r.center(),
                        4.0,
                        if self.view.devices.is_empty() {
                            DIM
                        } else {
                            GREEN
                        },
                    );
                    ui.label(
                        RichText::new(if self.view.devices.is_empty() {
                            "no device"
                        } else {
                            "device"
                        })
                        .size(8.5)
                        .color(DIM),
                    );
                    ui.add_space(10.0);
                    let _ = now;
                });
            });
    }

    fn status_bar(&mut self, root: &mut egui::Ui, now: f64) {
        Panel::bottom("status")
            .exact_size(28.0)
            .frame(
                Frame::new()
                    .fill(TOP)
                    .stroke(Stroke::new(1.0, LINE_SOFT))
                    .inner_margin(Margin {
                        left: 20,
                        right: 18,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    let lang = self.state.prefs.lang;
                    let ready = !self.view.devices.is_empty() && pages::access_ready(&self.view);
                    let color = if ready { GREEN } else { AMBER };
                    let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.0, color);
                    let text = if self.view.devices.is_empty() {
                        tr(
                            lang,
                            "No keyboard · connect USB and press a key",
                            "Клавиатура не найдена · подключите USB и нажмите клавишу",
                        )
                    } else if !pages::access_ready(&self.view) {
                        tr(
                            lang,
                            "Keyboard found · no permission — open Device",
                            "Клавиатура найдена · нет прав — откройте «Устройство»",
                        )
                    } else {
                        tr(
                            lang,
                            "Keyboard ready · use Apply to keyboard",
                            "Клавиатура готова · нажмите «Применить к клавиатуре»",
                        )
                    };
                    ui.label(RichText::new(text).size(9.5).color(DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("v{}", APP_VERSION))
                                .size(9.5)
                                .color(DIM),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(if self.dirty {
                                "Unsaved changes"
                            } else {
                                "Saved locally"
                            })
                            .size(9.5)
                            .color(if self.dirty {
                                AMBER
                            } else {
                                DIM
                            }),
                        );
                        ui.add_space(10.0);
                        let _ = now;
                    });
                });
            });
    }

    fn toast(&mut self, ctx: &egui::Context, now: f64) {
        let Some((text, until)) = self.view.toast.clone() else {
            return;
        };
        if now > until {
            self.view.toast = None;
            return;
        }
        let fade = ((until - now) / 0.35).min(1.0) as f32;
        egui::Area::new(Id::unique("toast"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-22.0, -42.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_max_width(720.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                Frame::new()
                    .fill(Color32::from_rgb(42, 44, 50))
                    .stroke(Stroke::new(1.0, ACCENT_SOFT))
                    .corner_radius(9)
                    .inner_margin(Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                            ui.painter().circle_filled(
                                r.center(),
                                3.0,
                                with_alpha(ACCENT, (255.0 * fade) as u8),
                            );
                            ui.label(
                                RichText::new(&text)
                                    .size(11.5)
                                    .color(with_alpha(TEXT, (255.0 * fade) as u8)),
                            );
                        });
                    });
            });
    }

    fn record(&mut self, ctx: &egui::Context, now: f64) {
        if !self.state.recording {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let idx = self
            .state
            .macro_selected
            .min(self.state.macros.len().saturating_sub(1));
        if self.state.macros.is_empty() {
            return;
        }
        if self.last_record == 0.0 {
            self.last_record = now;
        }
        for event in events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                let delay = (((now - self.last_record) * 1000.0) as u32).clamp(10, 5000);
                self.last_record = now;
                let mut text = format!("{:?}", key);
                if modifiers.ctrl {
                    text = format!("Ctrl + {}", text);
                }
                if modifiers.shift {
                    text = format!("Shift + {}", text);
                }
                if modifiers.alt {
                    text = format!("Alt + {}", text);
                }
                self.state.macros[idx].steps.push(model::MacroStep {
                    kind: model::StepKind::Key,
                    text,
                    delay_ms: delay,
                    holding_ms: 30,
                });
                self.dirty = true;
            }
        }
        self.state.macros[idx].name = self.state.macros[idx].name.clone();
    }

    fn activate_profile(&mut self, i: usize) {
        if i < self.state.profiles.len() {
            self.state.active_profile = i;
            let data = self.state.profiles[i].data.clone();
            self.state.lighting = data.lighting;
            self.state.keys = data.keys;
            self.dirty = true;
            log::info(
                "profile",
                format!("switched to \"{}\"", self.state.profiles[i].name),
            );
        }
    }

    fn save(&mut self, now: f64) {
        if self.state.active_profile < self.state.profiles.len() {
            self.state.profiles[self.state.active_profile].data = model::ProfileData {
                lighting: self.state.lighting.clone(),
                keys: self.state.keys.clone(),
            };
            self.state.profiles[self.state.active_profile].macros = self.state.macros.len();
        }
        self.persist();
        self.dirty = false;
        self.saved_at = now;
        log::info(
            "profile",
            format!(
                "saved \"{}\" and settings",
                self.state
                    .profiles
                    .get(self.state.active_profile)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Default")
            ),
        );
    }

    fn persist(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                log::error("storage", format!("cannot create {}: {err}", parent.display()));
            }
        }
        match serde_json::to_string_pretty(&self.state) {
            Ok(text) => {
                if let Err(err) = std::fs::write(&path, text) {
                    log::error("storage", format!("cannot write {}: {err}", path.display()));
                } else {
                    log::debug("storage", format!("state written to {}", path.display()));
                }
            }
            Err(err) => log::error("storage", format!("cannot encode state: {err}")),
        }
        update_autostart(self.state.prefs.autostart);
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/f75-control/state.json")
}

fn load_state() -> (AppState, Option<String>) {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<AppState>(&text) {
            Ok(state) => (state, None),
            Err(err) => (
                AppState::default(),
                Some(format!(
                    "settings at {} could not be read ({err}); starting with defaults",
                    path.display()
                )),
            ),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (AppState::default(), None),
        Err(err) => (
            AppState::default(),
            Some(format!(
                "settings at {} could not be opened ({err}); starting with defaults",
                path.display()
            )),
        ),
    }
}

fn update_autostart(enabled: bool) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let path = PathBuf::from(&home).join(".config/autostart/f75-control.desktop");
    if enabled {
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "f75-control".into());
        // Desktop-entry Exec is a single field: a path with spaces must be
        // quoted, and `%` is a field-code prefix, so it is doubled.
        let exec = format!(
            "\"{}\"",
            exe.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('%', "%%")
        );
        let entry = format!("[Desktop Entry]\nType=Application\nName=F75 Control Center\nExec={exec}\nX-GNOME-Autostart-enabled=true\n");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = std::fs::write(&path, entry) {
            log::warn("autostart", format!("cannot enable autostart: {err}"));
        }
    } else if path.exists() {
        if let Err(err) = std::fs::remove_file(&path) {
            log::warn("autostart", format!("cannot disable autostart: {err}"));
        }
    }
}
