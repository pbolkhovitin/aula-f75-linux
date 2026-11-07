//! Linux hidraw discovery and the vendor feature-report write path.
//!
//! Frames and their offsets are documented in MECHANICS.md. The keyboard exposes
//! a 1032-byte feature report (report id 6); settings are applied with an
//! explicit profile read + patch + save, followed by the per-key LED matrix for
//! the self-defined effect.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::log;
use crate::model::{self, Effect, KeyMap, Lighting, Macro, MacroStep, StepKind};

const FEATURE_REPORT_ID: u8 = 6;
const FEATURE_REPORT_LEN: usize = 1032;
const PROFILE_LEN: usize = 254;
const READ_DELAY: Duration = Duration::from_millis(700);
/// FUN_00489080 sleeps 0x1e (30 ms) before FUN_00487c10 writes the profile.
/// The 100 ms sleep in the same function belongs to the RGB-table path only.
const SAVE_DELAY: Duration = Duration::from_millis(30);
const MATRIX_DELAY: Duration = Duration::from_millis(50);
/// The end of FUN_00489080 waits 500 ms before returning to the caller.
const APPLY_SETTLE_DELAY: Duration = Duration::from_millis(500);
/// FUN_00489080 does Sleep(0x1E) before each LED group, then 50 ms between the
/// 0xBC and 0xC0 frames and 50 ms after the pair.
const LED_GROUP_DELAY: Duration = Duration::from_millis(30);
const PROFILE_SAVE_CMD: u8 = 0x03;
const PROFILE_READ_CMD: u8 = 0x83;
const PROFILE_TAG: u8 = 0xB6;
const MATRIX_HEADER: [u8; 7] = [9, 0xBC, 0, 0x40, 0, 0, 0];
const MATRIX_TAIL_HEADER: [u8; 7] = [9, 0xC0, 0, 0x40, 0, 0, 0];
/// FUN_00489080 sleeps 0x32 (50 ms) after each macro chunk and after the key
/// matrix frame.
const KEY_MATRIX_DELAY: Duration = Duration::from_millis(50);
const MACRO_CHUNK_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub wired: bool,
    /// True for the HID interface that exposes report id 6 as a feature
    /// report. The 2.4 GHz receiver presents two hidraw nodes: one is the
    /// plain keyboard and has no feature report, the other is the vendor
    /// control interface the settings frames must go to.
    pub control: bool,
    pub name: String,
}

/// udev reads rule files in lexical order and `73-seat-late.rules` is what
/// actually schedules the `uaccess` builtin. A file named `99-…` is processed
/// *after* that rule, so its `TAG+="uaccess"` is set too late: the tag exists
/// in the database, but no ACL is ever created. This is why the old command
/// left the node at `root:root` even though `udevadm info` showed the tag.
const RULE_PATH: &str = "/etc/udev/rules.d/70-f75.rules";
const OLD_RULE_PATH: &str = "/etc/udev/rules.d/99-f75.rules";

/// The primary group of the person running the app. udev's `uaccess` tag is
/// supposed to hand out an ACL, but on the machine this app was built for the
/// tag was present while no ACL was ever created, so `MODE=0660` still left
/// the node `root:root`. Naming the group makes the same mode work without
/// depending on the seat/ACL path at all.
pub fn user_group() -> String {
    fn safe(name: &str) -> bool {
        !name.is_empty()
            && name.len() < 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    }

    unsafe {
        let gid = libc::getgid();
        let mut buf = vec![0u8; 4096];
        let mut group: libc::group = std::mem::zeroed();
        let mut result: *mut libc::group = std::ptr::null_mut();
        let rc = libc::getgrgid_r(
            gid,
            &mut group,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            &mut result,
        );
        if rc == 0 && !result.is_null() {
            if let Ok(name) = std::ffi::CStr::from_ptr((*result).gr_name).to_str() {
                if safe(name) {
                    return name.to_string();
                }
            }
        }
    }
    // Fallback for exotic libc setups: ask the login tools.
    std::process::Command::new("id")
        .arg("-gn")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|name| safe(name))
        .unwrap_or_else(|| "users".to_string())
}

fn rule_line(vid: &str, pid: &str, group: &str) -> String {
    format!(
        "SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vid}\", ATTRS{{idProduct}}==\"{pid}\", MODE=\"0660\", GROUP=\"{group}\", TAG+=\"uaccess\""
    )
}

/// The two rule lines the app recommends. Kept in one place so the Device
/// page, the installer command and the saved script cannot drift apart.
pub fn udev_rule() -> String {
    let group = user_group();
    format!(
        "{}\n{}",
        rule_line("258a", "010c", &group),
        rule_line("3554", "fa09", &group)
    )
}

/// One copy-paste command that writes the rule, reloads udev and re-applies it.
///
/// It is deliberately a **single line**: some terminals treat a pasted
/// multi-line command as several commands, and a half-pasted `sudo sh -c '…`
/// only leaves a `>` continuation prompt. `printf` writes the two rule lines
/// without an embedded newline in the command itself.
pub fn install_command() -> String {
    let group = user_group();
    let escape = |line: String| line.replace('\\', "\\\\").replace('"', "\\\"");
    let first = escape(rule_line("258a", "010c", &group));
    let second = escape(rule_line("3554", "fa09", &group));
    format!(
        "sudo sh -c 'rm -f {OLD_RULE_PATH} && printf \"%s\\n\" \"{first}\" \"{second}\" > {RULE_PATH} && chmod 644 {RULE_PATH} && udevadm control --reload-rules && udevadm trigger --subsystem-match=hidraw'"
    )
}

/// The same fix as a small shell script. The clipboard is not reliable on
/// every Wayland compositor, so the Device page also can run this script via
/// pkexec or save it for later.
pub fn install_script() -> String {
    let group = user_group();
    let first = rule_line("258a", "010c", &group);
    let second = rule_line("3554", "fa09", &group);
    format!(
        "#!/bin/sh\n\
         # Give the logged-in user access to the AULA F75 hidraw nodes.\n\
         # MODE=0660 alone is not enough: the group must be the user's group,\n\
         # because the uaccess ACL is not applied on every system.\n\
         set -eu\n\
         if [ \"$(id -u)\" -ne 0 ]; then\n\
         \x20 echo \"This script needs administrator rights. Run: sudo sh $0\" >&2\n\
         \x20 exit 1\n\
         fi\n\
         rm -f {OLD_RULE_PATH}\n\
         printf '%s\\n' \\\n\
         \x20 '{first}' \\\n\
         \x20 '{second}' \\\n\
         \x20 > {RULE_PATH}\n\
         chmod 644 {RULE_PATH}\n\
         udevadm control --reload-rules\n\
         udevadm trigger --subsystem-match=hidraw\n\
         echo \"Done. Access is applied now; reconnect the keyboard if it still says no access.\"\n"
    )
}

pub fn script_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/f75-control/fix-access.sh")
}

pub fn save_install_script() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path = script_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, install_script())?;
    let mut perms = std::fs::metadata(&path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms)?;
    log::debug("access", format!("fix script written to {}", path.display()));
    Ok(path)
}

/// What the installed rule file looks like. The app was originally built for a
/// machine where the old rule existed but granted nothing, so the Device page
/// says which of these cases it found instead of a generic "no permission".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleState {
    Missing,
    /// A rule still lives in `99-f75.rules`, which udev reads after
    /// `73-seat-late.rules`, so its `uaccess` tag never schedules the ACL.
    LegacyPath,
    /// The file exists but has no `GROUP=`; `MODE=0660` keeps it root-only.
    NoGroup,
    /// `GROUP=` is present but names a different group.
    WrongGroup,
    Current,
}

pub fn classify_rule(text: &str, group: &str) -> RuleState {
    if text.trim().is_empty() || (!text.contains("258a") && !text.contains("3554")) {
        return RuleState::Missing;
    }
    if text.contains(&format!("GROUP=\"{group}\"")) {
        RuleState::Current
    } else if text.contains("GROUP=") {
        RuleState::WrongGroup
    } else {
        RuleState::NoGroup
    }
}

pub fn rule_state() -> RuleState {
    if let Ok(text) = std::fs::read_to_string(RULE_PATH) {
        if text.contains("258a") || text.contains("3554") {
            return classify_rule(&text, &user_group());
        }
    }
    if std::path::Path::new(OLD_RULE_PATH).exists() {
        return RuleState::LegacyPath;
    }
    RuleState::Missing
}

/// True when a graphical polkit agent can ask for the administrator password.
pub fn pkexec_available() -> bool {
    std::process::Command::new("pkexec")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Run the saved fix script through pkexec. This blocks until the person has
/// answered the system password dialog, so callers run it on a worker thread.
pub fn install_with_pkexec() -> Result<String, String> {
    log::info("access", "asking pkexec to install the udev rule");
    let path = save_install_script().map_err(|e| {
        let message = format!("could not save the fix script: {e}");
        log::error("access", &message);
        message
    })?;
    let output = std::process::Command::new("pkexec")
        .arg("sh")
        .arg(&path)
        .output()
        .map_err(|e| {
            let message = format!("pkexec is not available: {e}");
            log::error("access", &message);
            message
        })?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let message = text.lines().last().unwrap_or("Access rule installed").to_string();
        log::info("access", format!("pkexec finished: {message}"));
        Ok(message)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        let message = if detail.is_empty() {
            format!(
                "the system refused the password prompt (exit {})",
                output.status.code().unwrap_or(-1)
            )
        } else {
            detail
        };
        log::error("access", format!("pkexec failed: {message}"));
        Err(message)
    }
}

/// Scan a HID report descriptor for a feature main item that belongs to the
/// given report id. Used to tell the vendor control interface apart from the
/// ordinary keyboard node before opening anything.
fn has_feature_report(node: &str, report_id: u8) -> bool {
    let path = format!("/sys/class/hidraw/{node}/device/report_descriptor");
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == 0x85 && bytes[i + 1] == report_id {
            let end = (i + 32).min(bytes.len());
            let mut j = i + 2;
            while j < end {
                match bytes[j] {
                    0xB1 | 0xB2 => return true, // feature main item
                    0x85 => break,              // a different report id starts
                    _ => j += 1,
                }
            }
        }
        i += 1;
    }
    false
}

pub fn detect() -> Vec<DeviceInfo> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let uevent = entry.path().join("device/uevent");
        let Ok(text) = std::fs::read_to_string(uevent) else {
            continue;
        };
        let mut vid = 0u16;
        let mut pid = 0u16;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("HID_ID=") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() == 3 {
                    vid = u16::from_str_radix(parts[1], 16).unwrap_or(0);
                    pid = u16::from_str_radix(parts[2], 16).unwrap_or(0);
                }
            }
        }
        let wired = vid == model::VID_WIRED as u16 && pid == model::PID_WIRED as u16;
        let wireless = vid == model::VID_WIRELESS as u16 && pid == model::PID_WIRELESS as u16;
        if wired || wireless {
            found.push(DeviceInfo {
                path: format!("/dev/{name}"),
                vid,
                pid,
                wired,
                control: has_feature_report(&name, FEATURE_REPORT_ID),
                name: if wired {
                    "F75 (USB)".into()
                } else {
                    "F75 (2.4 GHz)".into()
                },
            });
        }
    }
    // Prefer the cable, then the interface that can actually take settings.
    found.sort_by_key(|d| (!d.wired, !d.control));
    log::debug(
        "device",
        format!(
            "detect: {} F75 node(s) [{}]",
            found.len(),
            found
                .iter()
                .map(|d| format!(
                    "{} {:04X}:{:04X} control={}",
                    d.path, d.vid, d.pid, d.control
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    found
}

#[derive(Clone, Debug)]
pub struct AccessCheck {
    pub path: String,
    pub label: String,
    pub control: bool,
    pub writable: bool,
    pub denied: bool,
    pub mode: String,
    pub os_error: String,
}

/// Try to open every F75 hidraw node read+write. This is the same call the
/// apply path makes, so the answer is the real permission state, not a guess.
pub fn check_access() -> Vec<AccessCheck> {
    detect()
        .into_iter()
        .map(|d| {
            let (writable, denied, os_error) =
                match OpenOptions::new().read(true).write(true).open(&d.path) {
                    Ok(_) => (true, false, String::new()),
                    Err(e) => (
                        false,
                        e.kind() == io::ErrorKind::PermissionDenied,
                        e.to_string(),
                    ),
                };
            let mode = std::fs::metadata(&d.path)
                .ok()
                .map(|m| {
                    let owner = if m.uid() == 0 {
                        "root".to_string()
                    } else {
                        format!("uid {}", m.uid())
                    };
                    format!("{:03o} · {owner}", m.mode() & 0o777)
                })
                .unwrap_or_else(|| "?".to_string());
            if writable {
                log::debug(
                    "access",
                    format!("{} is writable (mode {mode})", d.path),
                );
            } else {
                log::warn(
                    "access",
                    format!(
                        "{} is not writable (mode {mode}): {}",
                        d.path,
                        if os_error.is_empty() {
                            "unknown error"
                        } else {
                            os_error.as_str()
                        }
                    ),
                );
            }
            AccessCheck {
                path: d.path,
                label: d.name,
                control: d.control,
                writable,
                denied,
                mode,
                os_error,
            }
        })
        .collect()
}

fn hid_ioc(dir: u32, nr: u8, len: usize) -> libc::c_ulong {
    ((dir << 30) | ((len as u32) << 16) | (0x48 << 8) | nr as u32) as libc::c_ulong
}

fn set_feature(file: &File, buf: &[u8]) -> io::Result<()> {
    let request = hid_ioc(3, 0x06, buf.len());
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), request, buf.as_ptr()) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn get_feature(file: &File, buf: &mut [u8]) -> io::Result<()> {
    let request = hid_ioc(3, 0x07, buf.len());
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), request, buf.as_mut_ptr()) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn save_frame(profile: &[u8]) -> [u8; FEATURE_REPORT_LEN] {
    let mut frame = [0u8; FEATURE_REPORT_LEN];
    frame[0] = FEATURE_REPORT_ID;
    frame[1] = PROFILE_SAVE_CMD;
    frame[2] = PROFILE_TAG;
    frame[3..3 + PROFILE_LEN].copy_from_slice(profile);
    frame
}

fn read_frame() -> [u8; FEATURE_REPORT_LEN] {
    let mut frame = [0u8; FEATURE_REPORT_LEN];
    frame[0] = FEATURE_REPORT_ID;
    frame[1] = PROFILE_READ_CMD;
    frame[2] = PROFILE_TAG;
    frame
}

/// FUN_00490680: 520-byte configuration frame. Header is
/// `[9][checksum][5][1][1][chunk][len_lo][len_hi]`; the checksum is the low
/// byte of the sum of bytes 2..0x207. A payload is split into chunks of at
/// most `CONFIG_CHUNK_SIZE`; the F75 path has six bytes, so chunk 0 carries
/// all of them. Kept separate from the LED/profile frames because the app does
/// not need to send configuration yet; the layout is part of the record.
#[allow(dead_code)]
fn config_frame(payload: &[u8], chunk_index: u8) -> [u8; model::HID_REPORT_SIZE] {
    let mut frame = [0u8; model::HID_REPORT_SIZE];
    let len = payload.len().min(model::CONFIG_CHUNK_SIZE);
    frame[0] = model::HID_REPORT_ID;
    frame[2] = model::CONFIG_COMMAND;
    frame[3] = 1;
    frame[4] = 1;
    frame[5] = chunk_index;
    frame[6..8].copy_from_slice(&(len as u16).to_le_bytes());
    frame[8..8 + len].copy_from_slice(&payload[..len]);
    let sum = frame[model::CONFIG_CHECKSUM_START..=model::CONFIG_CHECKSUM_END]
        .iter()
        .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
    frame[1] = sum;
    frame
}

/// The vendor answers `[report id, 0x83, 0xB6, profile...]`. Accept a response
/// without the report id as well, so kernel/driver differences do not break us.
fn extract_profile(response: &[u8]) -> Option<Vec<u8>> {
    for start in [3usize, 2, 1, 0] {
        if start + PROFILE_LEN <= response.len()
            && response.get(start).copied() == Some(PROFILE_READ_CMD)
            && response.get(start + 1).copied() == Some(PROFILE_TAG)
        {
            return Some(response[start + 2..start + 2 + PROFILE_LEN].to_vec());
        }
    }
    None
}

fn read_profile(file: &File) -> Result<Vec<u8>, String> {
    set_feature(file, &read_frame()).map_err(|e| format!("read request failed: {e}"))?;
    thread::sleep(READ_DELAY);
    let mut response = [0u8; FEATURE_REPORT_LEN];
    get_feature(file, &mut response).map_err(|e| format!("read response failed: {e}"))?;
    extract_profile(&response).ok_or_else(|| "unexpected profile answer".to_string())
}

/// Build the 1890-byte LED table: 5 groups of 3 x 126 planes. The F75 only uses
/// group 0. FUN_00489080 places the colour dword's low byte in plane
/// GameRGBIndex[0], the next byte in GameRGBIndex[1] and the high byte in
/// GameRGBIndex[2]; KB.ini supplies 0,1,2.
fn led_matrix(lighting: &Lighting) -> Vec<u8> {
    let mut matrix = vec![0u8; model::LED_MATRIX_BYTES];
    let [r_plane, g_plane, b_plane] = model::GAME_RGB_INDEX;
    for (key, color) in model::KEYS.iter().zip(lighting.per_key.iter()) {
        let led = key.led as usize;
        if led >= model::LED_COLOR_PLANE_BYTES {
            continue;
        }
        matrix[r_plane as usize * model::LED_COLOR_PLANE_BYTES + led] = color[0];
        matrix[g_plane as usize * model::LED_COLOR_PLANE_BYTES + led] = color[1];
        matrix[b_plane as usize * model::LED_COLOR_PLANE_BYTES + led] = color[2];
    }
    matrix
}

fn matrix_frame(header: [u8; 7], data: &[u8]) -> [u8; FEATURE_REPORT_LEN] {
    let mut frame = [0u8; FEATURE_REPORT_LEN];
    frame[0] = FEATURE_REPORT_ID;
    frame[1..8].copy_from_slice(&header);
    let copy = data.len().min(model::LED_PAYLOAD_BUFFER);
    frame[8..8 + copy].copy_from_slice(&data[..copy]);
    frame
}

fn send_matrix(file: &File, lighting: &Lighting) -> Result<(), String> {
    let matrix = led_matrix(lighting);
    // FUN_00489080 copies matrix[0..1890] into its 1024-byte staging buffer,
    // sends 0xBC from matrix[0], then 0xC0 from matrix[1024]. The second frame
    // carries the remaining 866 bytes and is zero-padded.
    thread::sleep(LED_GROUP_DELAY);
    let first = matrix_frame(MATRIX_HEADER, &matrix[..model::LED_MATRIX_FIRST_CHUNK]);
    set_feature(file, &first).map_err(|e| format!("LED matrix chunk 1 failed: {e}"))?;
    thread::sleep(MATRIX_DELAY);
    let second = matrix_frame(MATRIX_TAIL_HEADER, &matrix[model::LED_MATRIX_FIRST_CHUNK..]);
    set_feature(file, &second).map_err(|e| format!("LED matrix chunk 2 failed: {e}"))?;
    thread::sleep(MATRIX_DELAY);
    Ok(())
}

/// FUN_00488FA0 patches the profile read from the keyboard:
/// byte 4 = (GaoshouIndex == selected LedOpt index), byte 5 = the selected
/// record's hardware effect id, byte 6 = StartGp + the group offset, and
/// bytes 0x14/0x15 = 0xFF.
///
/// It also writes every key's 2-byte entry at profile+0x16: byte 0 is the
/// direction (RandomVal=7 when random colour is on), byte 1 is
/// `(brightness << 4) | speed`. When the selected LedOpt record supports
/// direction, the vendor masks the low nibble and sets 0x80 for the "up"
/// direction (2).
fn patch_profile(
    profile: &mut [u8],
    effect: &Effect,
    lighting: &Lighting,
    selected_index: u8,
    group_offset: u8,
) {
    profile[4] = u8::from(selected_index == model::GAOSHOU_INDEX);
    profile[5] = effect.hw;
    profile[6] = model::START_GP.wrapping_add(group_offset);
    profile[0x14] = 0xFF;
    profile[0x15] = 0xFF;

    let brightness = lighting.brightness & 0x0F;
    let speed = lighting.speed & 0x0F;
    let mut direction = if lighting.random_color {
        model::RANDOM_VAL
    } else {
        lighting.direction
    };
    if effect.direction {
        direction &= 0x0F;
        if lighting.direction == 2 {
            direction |= 0x80;
        }
    }
    let packed = (brightness << 4) | speed;
    for key in 0..model::PROFILE_KEY_COUNT {
        let offset = model::PROFILE_KEY_MAP_OFFSET + key * model::PROFILE_KEY_STRIDE;
        if offset + 1 < profile.len() {
            profile[offset] = direction;
            profile[offset + 1] = packed;
        }
    }
}

/// One 4-byte action record from FUN_0048CF00:
/// `byte0 = (value_hi & 0x0F) | (type << 4)`, `byte1/2 = value`, `byte3 = HID`.
/// The vendor sets bit 7 for a key-down action (its `flags == 0`); key-up
/// actions carry the same HID code without bit 7. Types 3, 4 and 5 do not use
/// the down/up bit.
fn push_hid_action(out: &mut Vec<u8>, hid: u8, kind: u8, value: u32, down: bool) {
    let value = value.min(0x000F_FFFF);
    let mut byte0 = (((value >> 16) as u8) & 0x0F) | (kind << 4);
    if kind != 3 && kind != 4 && kind != 5 && down {
        byte0 |= 0x80;
    }
    out.push(byte0);
    out.push((value >> 8) as u8);
    out.push(value as u8);
    out.push(hid);
}

/// Encode a macro the way FUN_0048CF00 does: a one-byte UTF-16 name length,
/// the UTF-16LE name (max 30 units), then 4-byte action records. A delay is
/// not a record; it is added to the next action's 20-bit time value.
/// Returns the buffer and the number of steps that could not be mapped.
fn encode_macro(script: &Macro) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let units: Vec<u16> = script
        .name
        .encode_utf16()
        .take(model::MACRO_NAME_MAX_CHARS)
        .collect();
    out.push((units.len() * 2) as u8);
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }

    let mut pending = 0u32;
    let mut skipped = 0usize;
    for step in &script.steps {
        let delay = pending.saturating_add(step.delay_ms).min(0x000F_FFFF);
        pending = 0;
        match step.kind {
            StepKind::Delay => {
                pending = delay;
            }
            StepKind::Key => {
                let hid = model::vk_for_action(&step.text).and_then(model::hid_for_vk);
                let Some(hid) = hid else {
                    skipped += 1;
                    continue;
                };
                push_hid_action(&mut out, hid, 1, delay, true);
                push_hid_action(&mut out, hid, 1, step.holding_ms, false);
            }
            StepKind::Mouse => {
                if step.text == "Scroll up" || step.text == "Scroll down" {
                    let wheel = if step.text == "Scroll up" { 1u8 } else { 0xFF };
                    push_hid_action(&mut out, wheel, 5, delay, true);
                    continue;
                }
                let Some(code) = model::key_code_for_action(1, &step.text) else {
                    skipped += 1;
                    continue;
                };
                let hid = match code {
                    0xF0 => 1,
                    0xF1 => 2,
                    0xF2 => 4,
                    0xF3 => 8,
                    0xF4 => 0x10,
                    _ => {
                        skipped += 1;
                        continue;
                    }
                };
                push_hid_action(&mut out, hid, 2, delay, true);
                push_hid_action(&mut out, hid, 2, step.holding_ms, false);
            }
            StepKind::Text => {
                let mut first = true;
                for ch in step.text.chars() {
                    let name = ch.to_string();
                    let hid = model::vk_for_action(&name).and_then(model::hid_for_vk);
                    let Some(hid) = hid else {
                        skipped += 1;
                        continue;
                    };
                    push_hid_action(&mut out, hid, 1, if first { delay } else { 0 }, true);
                    push_hid_action(&mut out, hid, 1, step.holding_ms.max(10), false);
                    first = false;
                }
            }
        }
    }
    (out, skipped)
}

/// FUN_0048D2D0: one `(offset:u16, size:u16)` entry per bound macro at the
/// front of the buffer, followed by the encoded macros. Every offset is
/// shifted by `4 * macro_count`. Returns the buffer, the `(macro id, key
/// index)` bindings and the skipped-step count.
fn build_macro_buffer(scripts: &[Macro]) -> Result<(Vec<u8>, Vec<(u8, usize)>, usize), String> {
    let mut records = Vec::new();
    let mut skipped = 0usize;
    for script in scripts {
        let Some(key_index) = model::macro_trigger_key(&script.trigger) else {
            continue;
        };
        let id = (records.len() + 1) as u8;
        if id > model::MACRO_MAX_ID {
            return Err(format!(
                "more than {} macros are assigned to keys; the vendor limit is {}",
                model::MACRO_MAX_ID,
                model::MACRO_MAX_ID
            ));
        }
        let (data, unsupported) = encode_macro(script);
        skipped += unsupported;
        records.push((id, key_index, data));
    }
    if records.is_empty() {
        return Ok((Vec::new(), Vec::new(), skipped));
    }

    let count = records.len();
    let header = count * model::MACRO_INDEX_ENTRY_BYTES;
    let total: usize = header + records.iter().map(|(_, _, data)| data.len()).sum::<usize>();
    if total > model::MACRO_BUFFER {
        return Err(format!(
            "macro buffer needs {total} bytes but the F75 profile allows {}",
            model::MACRO_BUFFER
        ));
    }

    let mut buffer = vec![0u8; total];
    let mut offset = header;
    for (entry, (_, _, data)) in records.iter().enumerate() {
        let at = entry * model::MACRO_INDEX_ENTRY_BYTES;
        buffer[at..at + 2].copy_from_slice(&(offset as u16).to_le_bytes());
        buffer[at + 2..at + 4].copy_from_slice(&(data.len() as u16).to_le_bytes());
        buffer[offset..offset + data.len()].copy_from_slice(data);
        offset += data.len();
    }
    let bindings = records
        .iter()
        .map(|(id, key, _)| (*id, *key))
        .collect::<Vec<_>>();
    Ok((buffer, bindings, skipped))
}

/// FUN_00487F20: read the current 1024-byte key matrix so an unassigned key
/// keeps its factory action. The request is `84 D4` in a report-id-6 frame;
/// the reply's report id is stripped before the 256 dwords are copied.
fn read_key_matrix(file: &File) -> Result<Vec<u8>, String> {
    let mut request = [0u8; FEATURE_REPORT_LEN];
    request[0] = FEATURE_REPORT_ID;
    request[1..8].copy_from_slice(&model::KEY_MATRIX_READ_HEADER);
    set_feature(file, &request).map_err(|e| format!("key-matrix read request failed: {e}"))?;
    thread::sleep(READ_DELAY);
    let mut response = [0u8; FEATURE_REPORT_LEN];
    get_feature(file, &mut response).map_err(|e| format!("key-matrix read failed: {e}"))?;
    let start = usize::from(response[0] == FEATURE_REPORT_ID);
    if start + model::KEY_MATRIX_BYTES > response.len() {
        return Err("key-matrix answer is too short".to_string());
    }
    Ok(response[start..start + model::KEY_MATRIX_BYTES].to_vec())
}

/// Overlay assignments on the matrix read from the device. FUN_00488C50
/// stores one dword per matrix slot; a macro key keeps its low 24 bits and
/// gets `macro_id - 1` in the high byte.
fn overlay_key_map(matrix: &mut [u8], keys: &KeyMap, bindings: &[(u8, usize)]) {
    for (index, assignment) in keys.base.iter().enumerate() {
        let Some(assignment) = assignment else {
            continue;
        };
        let Some(code) = model::key_code_for_action(assignment.category, &assignment.action) else {
            continue;
        };
        let at = index * 4;
        if at + 4 <= matrix.len() {
            matrix[at..at + 4].copy_from_slice(&code.to_le_bytes());
        }
    }
    for (id, index) in bindings {
        let at = index * 4;
        if at + 4 <= matrix.len() {
            matrix[at + 3] = id.saturating_sub(1);
        }
    }
}

fn send_key_matrix(file: &File, matrix: &[u8]) -> Result<(), String> {
    let frame = matrix_frame(model::KEY_MATRIX_WRITE_HEADER, matrix);
    set_feature(file, &frame).map_err(|e| format!("key-matrix write failed: {e}"))
}

/// FUN_004881F0: macro chunks use `05 <id> 00 40 00 00 00` + 1024 bytes.
/// FUN_00489080 walks the four ids 0xDC, 0xE0, 0xE4, 0xE8 and sleeps 50 ms.
fn send_macro_buffer(file: &File, buffer: &[u8]) -> Result<(), String> {
    for (index, chunk) in buffer.chunks(model::MACRO_CHUNK_SIZE).enumerate() {
        let Some(id) = model::MACRO_COMMAND_IDS.get(index) else {
            return Err("macro buffer needs more than four 1024-byte chunks".to_string());
        };
        let header = [5, *id, 0, 0x40, 0, 0, 0];
        let frame = matrix_frame(header, chunk);
        set_feature(file, &frame).map_err(|e| format!("macro chunk {} failed: {e}", index + 1))?;
        thread::sleep(MACRO_CHUNK_DELAY);
    }
    Ok(())
}

fn apply_to(
    device: &DeviceInfo,
    lighting: &Lighting,
    keys: &KeyMap,
    macros: &[Macro],
) -> Result<String, String> {
    log::info(
        "apply",
        format!(
            "opening {} ({:04X}:{:04X}, control={})",
            device.path, device.vid, device.pid, device.control
        ),
    );
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&device.path)
        .map_err(|e| format!("open failed: {e}"))?;

    let mut steps = Vec::new();
    let mut warnings = Vec::new();

    // FUN_00489080 sends the key work (bit 0) before the profile/lighting
    // branch (bit 1): macros first, then the key matrix, then the profile.
    let (macro_buffer, bindings, skipped) = build_macro_buffer(macros)?;
    if skipped > 0 {
        warnings.push(format!(
            "{skipped} macro step(s) use an action the F75 encoder cannot map"
        ));
        log::warn(
            "apply",
            format!("{skipped} macro step(s) cannot be encoded"),
        );
    }
    let has_key_work = keys.base.iter().any(|assignment| assignment.is_some())
        || !bindings.is_empty();
    log::debug(
        "apply",
        format!(
            "plan: key work={has_key_work}, macro bytes={}, bindings={}, effect {}",
            macro_buffer.len(),
            bindings.len(),
            lighting.effect
        ),
    );
    if has_key_work {
        // Read first: keys the person did not touch must keep their factory
        // action. FUN_00487F20 does the same before FUN_00488C50 overlays.
        let mut matrix = read_key_matrix(&file)?;
        overlay_key_map(&mut matrix, keys, &bindings);
        if !macro_buffer.is_empty() {
            send_macro_buffer(&file, &macro_buffer)?;
            steps.push("macros");
            thread::sleep(KEY_MATRIX_DELAY);
        }
        send_key_matrix(&file, &matrix)?;
        steps.push("key map");
        thread::sleep(KEY_MATRIX_DELAY);
    }

    let mut profile = read_profile(&file)?;
    let effect = model::effect_by_ui(lighting.effect);
    let custom = lighting.effect == 19;
    // The vendor stores the selected 0-based LedOpt record index; GaoshouIndex
    // is 18 in KB.ini, so the flag is true for UI effect 19 (Self-define).
    let selected_index = (lighting.effect.saturating_sub(1)) as u8;
    patch_profile(
        &mut profile,
        effect,
        lighting,
        selected_index,
        model::PROFILE_GROUP_OFFSET,
    );

    if custom {
        send_matrix(&file, lighting)?;
        steps.push("per-key LED table");
    }
    // FUN_00489080: "Cfg(Save...)", Sleep(0x1e), FUN_00487c10.
    thread::sleep(SAVE_DELAY);
    set_feature(&file, &save_frame(&profile)).map_err(|e| format!("save request failed: {e}"))?;
    thread::sleep(APPLY_SETTLE_DELAY);
    steps.insert(0, "effect profile");

    let mut message = format!("Applied {} to {}", steps.join(" + "), device.name);
    if !warnings.is_empty() {
        message.push_str(" (");
        message.push_str(&warnings.join("; "));
        message.push(')');
    }
    log::info("apply", format!("{} finished: {message}", device.path));
    Ok(message)
}

fn failure_text(errors: &[String], russian: bool) -> String {
    let denied = errors
        .iter()
        .any(|e| e.contains("Permission denied") || e.contains("os error 13"));
    let timed_out = errors.iter().any(|e| {
        e.contains("os error 110")
            || e.to_ascii_lowercase().contains("timed out")
            || e.contains("ETIMEDOUT")
    });
    let paths: Vec<&str> = errors
        .iter()
        .filter_map(|e| e.split_once(": ").map(|(path, _)| path))
        .collect();
    let paths = if paths.is_empty() {
        "hidraw".to_string()
    } else {
        paths.join(", ")
    };
    if denied {
        let rule_note = match rule_state() {
            RuleState::LegacyPath => {
                if russian {
                    " Прежнее правило лежит в 99-f75.rules, а udev читает его позже 73-seat-late.rules — тег uaccess просто не успевает сработать, и ACL не появляется."
                } else {
                    " The old rule sits in 99-f75.rules, which udev reads after 73-seat-late.rules; the uaccess tag is set too late and no ACL is created."
                }
            }
            RuleState::NoGroup => {
                if russian {
                    " Файл правила уже есть, но в нём нет GROUP=, поэтому доступ так и не появился."
                } else {
                    " The rule file exists but has no GROUP=, so access never appeared."
                }
            }
            RuleState::WrongGroup => {
                if russian {
                    " В правиле указана другая группа пользователей."
                } else {
                    " The rule names a different user group."
                }
            }
            _ => "",
        };
        if russian {
            format!(
                "Клавиатура найдена, но Linux не дал программе открыть {paths}: файлы /dev/hidraw* по умолчанию принадлежат root.{rule_note} Откройте «Устройство» и нажмите «Дать доступ сейчас» — система один раз спросит пароль администратора. Подробности: {}",
                errors.join("; ")
            )
        } else {
            format!(
                "The keyboard was found, but Linux did not let the app open {paths}: /dev/hidraw* belongs to root by default.{rule_note} Open Device and press “Allow access now”; the system asks for the administrator password once. Details: {}",
                errors.join("; ")
            )
        }
    } else if timed_out {
        if russian {
            format!(
                "Права есть, запрос ушёл, но клавиатура не ответила за 5 секунд (таймаут приёмника). Это не ошибка настроек: по радиоканалу приёмник сейчас принимает только чтение. Подключите клавиатуру USB-кабелем, нажмите на ней любую клавишу и повторите. Подробности: {}",
                errors.join("; ")
            )
        } else {
            format!(
                "Access is fine and the request was sent, but the keyboard did not answer within 5 seconds (receiver timeout). The 2.4 GHz receiver currently accepts reads only. Connect the keyboard with the USB cable, press a key and try again. Details: {}",
                errors.join("; ")
            )
        }
    } else if russian {
        format!(
            "Не удалось записать в клавиатуру. Разбудите её клавишей, подключите по USB и закройте другую программу, которая может её занимать. Подробности: {}",
            errors.join("; ")
        )
    } else {
        format!(
            "Could not write to the keyboard. Wake it with a key, connect it by USB and close any other program that may hold it. Details: {}",
            errors.join("; ")
        )
    }
}

/// Apply the current lighting to the first reachable keyboard. A failed device
/// does not stop the next one: wired and wireless nodes can both be present.
/// When the control interface is known, the plain keyboard node is skipped so
/// the report is not sent to an interface that cannot accept it.
pub fn apply_lighting(
    lighting: &Lighting,
    keys: &KeyMap,
    macros: &[Macro],
    russian: bool,
) -> Result<String, String> {
    let devices = detect();
    if devices.is_empty() {
        log::error("apply", "no F75 hidraw node is present");
        return Err(if russian {
            "Клавиатура F75 не найдена. Подключите её по USB, нажмите любую клавишу, включите режим Windows (FN+W) и повторите."
                .into()
        } else {
            "No F75 found. Connect it by USB, press a key to wake it, and use FN+W for Windows mode.".into()
        });
    }

    let has_control = devices.iter().any(|d| d.control);
    log::info(
        "apply",
        format!(
            "found {} node(s); {} will receive the frames",
            devices.len(),
            if has_control {
                "only the control interface"
            } else {
                "all nodes"
            }
        ),
    );
    let mut errors = Vec::new();
    for device in devices.iter().filter(|d| !has_control || d.control) {
        match apply_to(device, lighting, keys, macros) {
            Ok(message) => return Ok(message),
            Err(error) => {
                log::warn("apply", format!("{}: {error}", device.path));
                errors.push(format!("{}: {error}", device.path));
            }
        }
    }
    let message = failure_text(&errors, russian);
    log::error("apply", format!("all device attempts failed: {errors:?}"));
    Err(message)
}

fn looks_timed_out(error: &str) -> bool {
    error.contains("os error 110")
        || error.to_ascii_lowercase().contains("timed out")
        || error.contains("ETIMEDOUT")
}

/// One device as seen by a read-only connection test. `request_ok` is the
/// vendor profile-read request (a feature-report write); it is the same call
/// the apply path makes, so a timeout here explains a failed apply before the
/// person changes any setting.
#[derive(Clone, Debug)]
pub struct ProbeReport {
    pub path: String,
    pub label: String,
    pub control: bool,
    pub access_ok: bool,
    pub access_error: String,
    pub read_ok: bool,
    pub read_error: String,
    pub request_ok: bool,
    pub request_error: String,
    pub request_ms: u128,
    pub profile_ok: bool,
    pub profile_error: String,
}

/// Open every F75 node, read the vendor feature report, and on the control
/// interface send the harmless 254-byte profile *read* request. Nothing is
/// written to the keyboard's memory.
pub fn probe_devices() -> Vec<ProbeReport> {
    let mut out = Vec::new();
    for device in detect() {
        let mut report = ProbeReport {
            path: device.path.clone(),
            label: device.name.clone(),
            control: device.control,
            access_ok: false,
            access_error: String::new(),
            read_ok: false,
            read_error: String::new(),
            request_ok: false,
            request_error: String::new(),
            request_ms: 0,
            profile_ok: false,
            profile_error: String::new(),
        };
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device.path)
        {
            Ok(file) => {
                report.access_ok = true;
                file
            }
            Err(error) => {
                report.access_error = error.to_string();
                log::warn(
                    "probe",
                    format!("{} cannot be opened: {error}", device.path),
                );
                out.push(report);
                continue;
            }
        };
        let mut buffer = [0u8; FEATURE_REPORT_LEN];
        match get_feature(&file, &mut buffer) {
            Ok(()) => report.read_ok = true,
            Err(error) => {
                report.read_error = error.to_string();
                log::warn(
                    "probe",
                    format!("{} feature read failed: {error}", device.path),
                );
            }
        }
        if device.control {
            let started = Instant::now();
            match set_feature(&file, &read_frame()) {
                Ok(()) => {
                    report.request_ok = true;
                    report.request_ms = started.elapsed().as_millis();
                    thread::sleep(READ_DELAY);
                    let mut answer = [0u8; FEATURE_REPORT_LEN];
                    match get_feature(&file, &mut answer) {
                        Ok(()) => match extract_profile(&answer) {
                            Some(_) => report.profile_ok = true,
                            None => {
                                report.profile_error =
                                    "the answer is not a profile (wrong header)".to_string()
                            }
                        },
                        Err(error) => report.profile_error = error.to_string(),
                    }
                }
                Err(error) => {
                    report.request_ms = started.elapsed().as_millis();
                    log::warn(
                        "probe",
                        format!(
                            "{} settings request failed after {} ms: {error}",
                            device.path, report.request_ms
                        ),
                    );
                    report.request_error = error.to_string();
                }
            }
        }
        out.push(report);
    }
    out
}

/// A plain-language conclusion from the probe, for the Device page.
pub fn probe_verdict(reports: &[ProbeReport], russian: bool) -> String {
    if reports.is_empty() {
        return if russian {
            "Клавиатура не найдена. Подключите её и нажмите любую клавишу.".into()
        } else {
            "No keyboard found. Connect it and press any key.".into()
        };
    }
    if reports.iter().any(|r| !r.access_ok) {
        return if russian {
            "Сначала нужен доступ к устройству: нажмите «Дать доступ сейчас».".into()
        } else {
            "Device access is needed first: press “Allow access now”.".into()
        };
    }
    let controls: Vec<&ProbeReport> = reports.iter().filter(|r| r.control).collect();
    if controls.is_empty() {
        return if russian {
            "Найден только узел клавиатуры без канала настроек. Подключите USB-кабель или переподключите приёмник.".into()
        } else {
            "Only the plain keyboard node was found, without the settings channel. Connect the USB cable or replug the receiver.".into()
        };
    }
    if let Some(ok) = controls.iter().find(|r| r.profile_ok) {
        return if russian {
            format!(
                "Клавиатура ответила по {}: чтение и запрос настроек работают. Можно применять подсветку.",
                ok.path
            )
        } else {
            format!(
                "The keyboard answered on {}: reads and the settings request both work. Lighting can be applied.",
                ok.path
            )
        };
    }
    if let Some(timeout) = controls.iter().find(|r| looks_timed_out(&r.request_error)) {
        let elapsed = if timeout.request_ms == 0 {
            RECEIVER_TIMEOUT_MS
        } else {
            timeout.request_ms
        };
        return if russian {
            format!(
                "Приёмник на {} отвечает на чтение, но запрос настроек отклонён по таймауту ({} мс). По радиоканалу запись сейчас не проходит — подключите USB-кабель. Это не проблема прав.",
                timeout.path, elapsed
            )
        } else {
            format!(
                "The receiver on {} answers reads, but the settings request timed out after {} ms. Writing over 2.4 GHz does not work right now — connect the USB cable. This is not a permission problem.",
                timeout.path, elapsed
            )
        };
    }
    if let Some(reply) = controls.iter().find(|r| !r.profile_ok) {
        return if russian {
            format!(
                "Запрос ушёл, но ответ не похож на профиль ({}). Попробуйте USB-кабель. {}",
                reply.profile_error, reply.request_error
            )
        } else {
            format!(
                "The request was sent, but the answer is not a profile ({}). Try the USB cable. {}",
                reply.profile_error, reply.request_error
            )
        };
    }
    if russian {
        "Проверка не дала ответа. Попробуйте USB-кабель.".into()
    } else {
        "The check gave no answer. Try the USB cable.".into()
    }
}

/// Work that must not block the window: a write can wait up to five seconds
/// per attempt inside the kernel, and pkexec waits for a password dialog.
pub enum JobOutcome {
    Applied(Result<String, String>),
    Installed(Result<String, String>),
    Probed(Vec<ProbeReport>),
}

/// Measured on the reported machine: a profile-read feature write on the
/// 2.4 GHz receiver returns `ETIMEDOUT` after roughly this long. Used as the
/// displayed fallback when the kernel reports no elapsed time.
pub const RECEIVER_TIMEOUT_MS: u128 = 5100;

pub fn spawn_apply(
    lighting: Lighting,
    keys: KeyMap,
    macros: Vec<Macro>,
    russian: bool,
) -> Receiver<JobOutcome> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(JobOutcome::Applied(apply_lighting(
            &lighting, &keys, &macros, russian,
        )));
    });
    rx
}

pub fn spawn_install() -> Receiver<JobOutcome> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(JobOutcome::Installed(install_with_pkexec()));
    });
    rx
}

pub fn spawn_probe() -> Receiver<JobOutcome> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(JobOutcome::Probed(probe_devices()));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_frame_matches_vendor_layout() {
        let frame = config_frame(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF], 0);
        assert_eq!(frame[0], 9);
        assert_eq!(frame[2], 5);
        assert_eq!(frame[3], 1);
        assert_eq!(frame[4], 1);
        assert_eq!(frame[5], 0);
        assert_eq!(&frame[6..8], &[6, 0]);
        assert_eq!(&frame[8..14], &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let expected = frame[2..=0x207]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        assert_eq!(frame[1], expected);
    }

    #[test]
    fn permission_failure_names_the_node_and_the_fix() {
        let errors = vec!["/dev/hidraw6: open failed: Permission denied (os error 13)".to_string()];
        let ru = failure_text(&errors, true);
        assert!(ru.contains("/dev/hidraw6"));
        assert!(ru.contains("root"));
        assert!(ru.contains("Устройство"));
        let en = failure_text(&errors, false);
        assert!(en.contains("/dev/hidraw6"));
        assert!(en.contains("root"));
        assert!(en.contains("Device"));
    }

    #[test]
    fn other_failure_does_not_blame_permissions() {
        let errors = vec!["/dev/hidraw6: save request failed: Invalid argument".to_string()];
        let text = failure_text(&errors, true);
        assert!(!text.contains("root"));
        assert!(text.contains("USB"));
    }

    #[test]
    fn receiver_timeout_is_reported_as_wireless_not_permission() {
        let errors = vec![
            "/dev/hidraw6: read request failed: Connection timed out (os error 110)".to_string(),
        ];
        let text = failure_text(&errors, true);
        assert!(!text.contains("Права нет"));
        assert!(!text.contains("root"));
        assert!(text.contains("5 секунд"));
        assert!(text.contains("USB"));
    }

    #[test]
    fn probe_verdict_explains_a_receiver_timeout() {
        let report = ProbeReport {
            path: "/dev/hidraw6".into(),
            label: "F75 (2.4 GHz)".into(),
            control: true,
            access_ok: true,
            access_error: String::new(),
            read_ok: true,
            read_error: String::new(),
            request_ok: false,
            request_error: "Connection timed out (os error 110)".into(),
            request_ms: 5130,
            profile_ok: false,
            profile_error: String::new(),
        };
        let text = probe_verdict(&[report], true);
        assert!(text.contains("5130"));
        assert!(text.contains("USB"));
        assert!(text.contains("не проблема прав"));
    }

    #[test]
    fn profile_key_map_uses_vendor_packing() {
        let mut profile = vec![0u8; PROFILE_LEN];
        let effect = model::effect_by_ui(11);
        let lighting = Lighting {
            effect: 11,
            brightness: 7,
            speed: 2,
            direction: 1,
            random_color: false,
            ..Lighting::default()
        };
        patch_profile(&mut profile, effect, &lighting, 10, 0);
        assert_eq!(profile[4], 0);
        assert_eq!(profile[5], effect.hw);
        assert_eq!(profile[6], model::START_GP);
        assert_eq!(&profile[0x14..=0x15], &[0xFF, 0xFF]);
        for key in 0..model::PROFILE_KEY_COUNT {
            let offset = model::PROFILE_KEY_MAP_OFFSET + key * model::PROFILE_KEY_STRIDE;
            assert_eq!(profile[offset], 1);
            assert_eq!(profile[offset + 1], (7 << 4) | 2);
        }
    }

    #[test]
    fn profile_key_map_random_and_direction_up() {
        let mut profile = vec![0u8; PROFILE_LEN];
        let effect = model::effect_by_ui(11);
        let lighting = Lighting {
            effect: 11,
            brightness: 3,
            speed: 4,
            direction: 2,
            random_color: true,
            ..Lighting::default()
        };
        patch_profile(&mut profile, effect, &lighting, 10, 0);
        // F75 effects have direction support 0, so byte 0 is the raw RandomVal.
        assert_eq!(profile[model::PROFILE_KEY_MAP_OFFSET], model::RANDOM_VAL);
        assert_eq!(profile[model::PROFILE_KEY_MAP_OFFSET + 1], (3 << 4) | 4);
    }

    #[test]
    fn config_frame_caps_payload_at_512_bytes() {
        let payload = vec![0x5Au8; 600];
        let frame = config_frame(&payload, 3);
        assert_eq!(frame[5], 3);
        assert_eq!(&frame[6..8], &[0, 2]); // 512 little-endian
        assert_eq!(frame[8 + 511], 0x5A);
        assert_eq!(frame[519], 0x5A); // payload fills 8..520, capped at 512
    }

    #[test]
    fn install_command_is_one_paste_safe_line() {
        // A line break in the copied text turns a paste into several shell
        // commands (and a bare `>` continuation prompt) in some terminals.
        let command = install_command();
        assert!(!command.contains('\n'));
        assert!(command.starts_with("sudo sh -c '"));
        assert!(command.ends_with("udevadm trigger --subsystem-match=hidraw'"));
        assert!(command.contains("258a"));
        assert!(command.contains("fa09"));
        assert!(command.contains("/etc/udev/rules.d/70-f75.rules"));
        // A 99- rule runs after 73-seat-late.rules, so its uaccess tag never
        // schedules the ACL; the fix must remove it rather than coexist.
        assert!(command.contains("rm -f /etc/udev/rules.d/99-f75.rules"));
        // The old rule had no GROUP=, which is why MODE=0660 still left the
        // node root-only on the machine this was reported on.
        assert!(command.contains(&format!("GROUP=\\\"{}\\\"", user_group())));

        // `sh -n` parses without running: catches a broken quote or escape
        // before the person pastes it into a root shell.
        let status = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .status()
            .expect("sh must be available on Linux");
        assert!(status.success(), "copied command must be valid shell");
    }

    #[test]
    fn installed_rule_grants_the_users_group() {
        let rule = udev_rule();
        assert!(rule.contains(&format!("GROUP=\"{}\"", user_group())));
        assert!(!rule.contains("GROUP=\"root\""));
        assert_eq!(rule.lines().count(), 2);
    }

    #[test]
    fn rule_state_separates_the_old_rule_from_the_fixed_one() {
        let old = "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"3554\", ATTRS{idProduct}==\"fa09\", MODE=\"0660\", TAG+=\"uaccess\"";
        assert_eq!(classify_rule(old, "pbolk"), RuleState::NoGroup);
        let current = rule_line("3554", "fa09", "pbolk");
        assert_eq!(classify_rule(&current, "pbolk"), RuleState::Current);
        assert_eq!(classify_rule(&current, "anna"), RuleState::WrongGroup);
        assert_eq!(classify_rule("", "pbolk"), RuleState::Missing);
    }

    #[test]
    fn vendor_vk_table_maps_known_keys() {
        assert_eq!(model::hid_for_vk(0x41), Some(0x04)); // A
        assert_eq!(model::hid_for_vk(0x1B), Some(0x29)); // Esc
        assert_eq!(model::hid_for_vk(0xA2), Some(0xE0)); // left Ctrl
        assert_eq!(model::hid_for_vk(0xA0), Some(0xE1)); // left Shift
        assert_eq!(model::hid_for_vk(0x5B), Some(0xE3)); // left Win
        assert_eq!(model::hid_for_vk(0xC4), Some(0x90)); // KB.ini right Ctrl
        assert_eq!(model::hid_for_vk(0xFE), None);
    }

    #[test]
    fn macro_encoder_writes_name_and_down_up_actions() {
        let script = Macro {
            name: "Test".into(),
            trigger: "F9".into(),
            mode: 0,
            cycles: 1,
            steps: vec![MacroStep {
                kind: StepKind::Key,
                text: "A".into(),
                delay_ms: 50,
                holding_ms: 30,
            }],
        };
        let (encoded, skipped) = encode_macro(&script);
        assert_eq!(skipped, 0);
        assert_eq!(&encoded[..9], &[8, b'T', 0, b'e', 0, b's', 0, b't', 0]);
        // A: down at +50 ms, up at +30 ms. Type 1, HID 0x04.
        assert_eq!(&encoded[9..13], &[0x90, 0x00, 50, 0x04]);
        assert_eq!(&encoded[13..17], &[0x10, 0x00, 30, 0x04]);
        assert_eq!(encoded.len(), 17);
    }

    #[test]
    fn macro_buffer_puts_shifted_index_table_first() {
        let key = |name: &str, trigger: &str| Macro {
            name: name.into(),
            trigger: trigger.into(),
            mode: 0,
            cycles: 1,
            steps: vec![MacroStep {
                kind: StepKind::Key,
                text: "A".into(),
                delay_ms: 0,
                holding_ms: 10,
            }],
        };
        let scripts = vec![key("", "F9"), key("", "F10")];
        let (buffer, bindings, skipped) = build_macro_buffer(&scripts).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(bindings, vec![(1, 9), (2, 10)]);
        // Two macros -> 8-byte header; both macros are name byte + 8 bytes.
        assert_eq!(buffer.len(), 8 + 9 + 9);
        let first_offset = u16::from_le_bytes([buffer[0], buffer[1]]) as usize;
        let first_size = u16::from_le_bytes([buffer[2], buffer[3]]) as usize;
        let second_offset = u16::from_le_bytes([buffer[4], buffer[5]]) as usize;
        let second_size = u16::from_le_bytes([buffer[6], buffer[7]]) as usize;
        assert_eq!(first_offset, 8);
        assert_eq!(first_size, 9);
        assert_eq!(second_offset, 17);
        assert_eq!(second_size, 9);
    }

    #[test]
    fn key_matrix_keeps_untouched_keys_and_sets_macro_high_byte() {
        let mut matrix = vec![0u8; model::KEY_MATRIX_BYTES];
        matrix[..4].copy_from_slice(&0x1Bu32.to_le_bytes()); // factory Esc
        matrix[4..8].copy_from_slice(&0x71u32.to_le_bytes()); // factory F2
        let mut keys = KeyMap::default();
        keys.base[1] = Some(model::Assignment::new(0, "A"));
        overlay_key_map(&mut matrix, &keys, &[(1, 9)]);
        assert_eq!(&matrix[..4], &0x1Bu32.to_le_bytes()); // untouched
        assert_eq!(&matrix[4..8], &0x41u32.to_le_bytes()); // F2 -> A
        assert_eq!(matrix[36 + 3], 0); // macro id 1 -> high byte 0
        assert_eq!(matrix[40..44], [0, 0, 0, 0]); // F10 untouched
    }

    #[test]
    fn unsupported_command_assignment_is_not_sent() {
        assert_eq!(model::key_code_for_action(3, "Lock computer"), None);
        assert_eq!(model::key_code_for_action(4, "Ctrl + C"), None);
        assert_eq!(model::key_code_for_action(2, "Mute"), Some(0x0200_00E2));
    }

    #[test]
    fn install_script_writes_the_same_rule() {
        let script = install_script();
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("ATTRS{idVendor}==\"258a\""));
        assert!(script.contains("ATTRS{idVendor}==\"3554\""));
        assert!(script.contains("70-f75.rules"));
        assert!(script.contains("rm -f /etc/udev/rules.d/99-f75.rules"));
        assert!(script.contains(&format!("GROUP=\"{}\"", user_group())));
        assert!(script.contains("udevadm control --reload-rules"));
        let status = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .status()
            .expect("sh must be available on Linux");
        assert!(status.success(), "saved script must be valid shell");
    }
}
