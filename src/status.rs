//! Machine-readable status for external consumers (the Noctalia widget).
//!
//! `f75-control --status` prints one JSON object and exits. It never opens the
//! device for writing and never touches the settings file, so it is cheap
//! enough to run from a bar widget every few seconds. `--watch` keeps the
//! process alive and prints a new line whenever the snapshot changes.
//!
//! The schema is versioned with `schema: 1`. Everything here is derived from
//! the same code the GUI uses (`device::detect`, `device::check_access`, the
//! saved `AppState`), so the app stays the single source of truth.

use crate::model::{
    self, AppState, Lang, BRIGHTNESS_LEVELS, FIRMWARE, KEY_COUNT, LED_COUNT, SLEEP_UI, SPEED_LEVELS,
};
use serde_json::{json, Value};
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA: u32 = 1;

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn direction_name(direction: u8) -> &'static str {
    match direction {
        0 => "left",
        1 => "right",
        2 => "up",
        3 => "down",
        _ => "unknown",
    }
}

fn hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

fn lang_name(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "en",
        Lang::Ru => "ru",
    }
}

fn state_snapshot(state: &AppState) -> Value {
    let effect = model::effect_by_ui(state.lighting.effect);
    let profile = state
        .profiles
        .get(state.active_profile)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Default".into());
    let remapped_base = state.keys.base.iter().filter(|a| a.is_some()).count();
    let remapped_func = state.keys.func.iter().filter(|a| a.is_some()).count();
    let macro_steps: usize = state.macros.iter().map(|m| m.steps.len()).sum();
    let sleep_seconds = SLEEP_UI
        .get(state.prefs.sleep_index)
        .copied()
        .unwrap_or(*SLEEP_UI.last().unwrap_or(&300));

    json!({
        "profile": {
            "index": state.active_profile,
            "name": profile,
            "available": state.profiles.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        },
        "lighting": {
            "effect_index": state.lighting.effect,
            "effect_name": effect.name,
            "effect_hardware_id": effect.hw,
            "brightness": state.lighting.brightness,
            "brightness_percent": (u16::from(state.lighting.brightness) + 1) * 10,
            "brightness_steps": BRIGHTNESS_LEVELS,
            "speed": state.lighting.speed,
            "speed_steps": SPEED_LEVELS,
            "direction": state.lighting.direction,
            "direction_name": direction_name(state.lighting.direction),
            "random_color": state.lighting.random_color,
            "color": hex(state.lighting.color),
            "per_key_slots": state.lighting.per_key.len(),
            "per_key_custom": state
                .lighting
                .per_key
                .iter()
                .filter(|c| **c != model::CLR_KEY_HAS_FUNC)
                .count(),
            "side_led": state.lighting.side_led,
            "paint_mode": state.lighting.paint,
        },
        "keys": {
            "count": KEY_COUNT,
            "remapped": remapped_base,
            "remapped_fn": remapped_func,
            "fn_layer": state.fn_layer,
        },
        "macros": {
            "count": state.macros.len(),
            "steps": macro_steps,
            "triggers": state.macros.iter().map(|m| m.trigger.clone()).collect::<Vec<_>>(),
        },
        "prefs": {
            "debounce_ms": model::DEBOUNCE_MS
                .get(state.prefs.debounce as usize)
                .copied()
                .unwrap_or(2),
            "sleep_seconds": sleep_seconds,
            "lang": lang_name(state.prefs.lang),
            "autostart": state.prefs.autostart,
            "log_level": state.prefs.log_level,
            "log_to_file": state.prefs.log_to_file,
        },
        "music": {
            "mode": state.music_mode,
            "sensitivity": state.music_sensitivity,
            "local_only": true,
        },
    })
}

/// Build the current snapshot. This performs device *detection* and a
/// read/write open test, but no HID traffic.
pub fn snapshot() -> Value {
    let devices = crate::device::detect();
    let access = crate::device::check_access();
    let (state, problem) = crate::load_state();
    let state_path = crate::config_path();
    let state_exists = state_path.exists();

    let mut warnings: Vec<String> = Vec::new();
    if let Some(problem) = &problem {
        warnings.push(problem.clone());
    }

    let wired = devices.iter().any(|d| d.wired);
    let wireless = devices.iter().any(|d| !d.wired);
    let transport = if wired {
        "usb"
    } else if wireless {
        "wireless"
    } else {
        "none"
    };

    let control = access.iter().find(|a| a.control);
    let keyboard = access.iter().find(|a| !a.control);

    let access_state = match control {
        None if access.is_empty() => "missing",
        None => "no-control",
        Some(node) if node.writable => "ok",
        Some(node) if node.denied => "denied",
        Some(_) => "error",
    };

    let present = !devices.is_empty();
    let writable = control.map(|n| n.writable).unwrap_or(false);
    let write_supported = present && writable && transport == "usb";

    if !present {
        warnings.push("No AULA F75 found on USB or the 2.4 GHz receiver.".into());
    } else if access_state == "denied" {
        warnings.push(
            "hidraw node is not writable; run the app's \"Allow access now\" action or install the udev rule."
                .into(),
        );
    } else if access_state == "no-control" {
        warnings.push(
            "Only the plain keyboard node is present; no settings channel (replug the receiver or use USB)."
                .into(),
        );
    }
    if transport == "wireless" {
        warnings.push(
            "2.4 GHz receiver: settings writes time out in practice, so lighting/key changes are read-only until a USB cable is used."
                .into(),
        );
    }
    if !state_exists && problem.is_none() {
        warnings.push(format!(
            "No saved settings at {}; defaults are reported.",
            state_path.display()
        ));
    }

    let nodes: Vec<Value> = access
        .iter()
        .map(|node| {
            json!({
                "path": node.path,
                "label": node.label,
                "control": node.control,
                "writable": node.writable,
                "denied": node.denied,
                "mode": node.mode,
                "error": node.os_error,
            })
        })
        .collect();

    json!({
        "schema": SCHEMA,
        "generated_ms": now_ms(),
        "app": {
            "name": model::APP_NAME,
            "version": model::APP_VERSION,
            "settings_file": state_path.display().to_string(),
            "settings_exists": state_exists,
        },
        "device": {
            "present": present,
            "transport": transport,
            "transport_label": if wired {
                "USB cable"
            } else if wireless {
                "2.4 GHz receiver"
            } else {
                "not connected"
            },
            "model": "F75 / F75 KR",
            "layout": format!("75% · {KEY_COUNT} keys · {LED_COUNT} LEDs"),
            "firmware": FIRMWARE,
            "firmware_hex": format!("0x{:02X}", FIRMWARE),
            "control_path": control.map(|n| n.path.clone()),
            "keyboard_path": keyboard.map(|n| n.path.clone()),
            "access": access_state,
            "writable": writable,
            "write_supported": write_supported,
            "nodes": nodes,
        },
        "state": state_snapshot(&state),
        "capabilities": {
            "lighting_write": write_supported,
            "key_remap_write": write_supported,
            "macro_write": write_supported,
            "profile_write": write_supported,
            "fn_layer": false,
            "side_led": true,
            "music": "local-only",
            "hardware_probe": false,
        },
        "ok": present && access_state == "ok",
        "warnings": warnings,
    })
}

fn emit(value: &Value, pretty: bool) {
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    match text {
        Ok(text) => {
            println!("{text}");
            let _ = std::io::stdout().flush();
        }
        Err(err) => eprintln!("could not encode status: {err}"),
    }
}

/// Comparison key for `--watch`: the timestamp changes on every poll, so it
/// must not count as a state change.
fn change_key(value: &Value) -> String {
    let mut copy = value.clone();
    if let Some(object) = copy.as_object_mut() {
        object.remove("generated_ms");
    }
    serde_json::to_string(&copy).unwrap_or_default()
}

pub fn run(pretty: bool) {
    emit(&snapshot(), pretty);
}

/// Print a snapshot now, then another line whenever it changes. Runs until
/// the process is killed (the widget/service owns the lifetime).
pub fn watch(interval_ms: u64, pretty: bool) {
    let interval = Duration::from_millis(interval_ms.max(250));
    let mut last: Option<String> = None;
    loop {
        let value = snapshot();
        let key = change_key(&value);
        if last.as_deref() != Some(key.as_str()) {
            emit(&value, pretty);
            last = Some(key);
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_reported_with_expected_units() {
        let value = state_snapshot(&AppState::default());
        assert_eq!(value["lighting"]["effect_name"], "Neon stream");
        assert_eq!(value["lighting"]["brightness_percent"], 80);
        assert_eq!(value["lighting"]["brightness_steps"], 10);
        assert_eq!(value["lighting"]["direction_name"], "right");
        assert_eq!(value["keys"]["count"], 80);
        assert_eq!(value["keys"]["remapped"], 0);
        assert_eq!(value["keys"]["remapped_fn"], 0);
        assert_eq!(value["prefs"]["sleep_seconds"], 300);
    }

    #[test]
    fn direction_names_cover_the_protocol_values() {
        assert_eq!(direction_name(0), "left");
        assert_eq!(direction_name(1), "right");
        assert_eq!(direction_name(2), "up");
        assert_eq!(direction_name(3), "down");
        assert_eq!(direction_name(9), "unknown");
    }

    #[test]
    fn color_is_upper_case_hex() {
        assert_eq!(hex([10, 147, 255]), "#0A93FF");
    }
}
