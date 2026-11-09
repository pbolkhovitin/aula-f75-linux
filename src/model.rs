//! Values and layout recovered from the F75 keyboard profile and the driver.
//! See MECHANICS.md for where each number comes from.

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "F75 Control Center";
pub const APP_VERSION: &str = "1.0.0";
pub const WINDOW_W: f32 = 1200.0;
pub const WINDOW_H: f32 = 760.0;

/// Logging limits. The vendor opens `%s\Log.txt` next to its config and
/// truncates it once at startup (FUN_00407770); keeping the same file name
/// makes a report easy to recognise. Rotation and the in-memory ring are our
/// additions, so they are marked estimated in mechanics.json.
pub const LOG_FILE_NAME: &str = "Log.txt";
pub const LOG_MAX_BYTES: u64 = 1_048_576; // 1 MiB, then Log.1.txt
pub const LOG_RING_CAP: usize = 2_000;
pub const LOG_LEVEL_COUNT: usize = 4;

pub const KEY_COUNT: usize = 80;
pub const LED_COUNT: usize = 90;
/// `MatrixLen` is the key-slot / loop bound used when walking the colour
/// matrix (FUN_00410a70 stores it at devState+0x2ff8); it is not a byte count.
pub const MATRIX_LEN: usize = 128;
pub const MACRO_BUFFER: usize = 4096;
pub const FIRMWARE: u32 = 24;
/// 520-byte configuration frame: FUN_00490680 uses a 0x208-byte buffer and
/// HidD_SetFeature; the same report id is used for the 1032-byte LED frame.
pub const HID_REPORT_ID: u8 = 9;
pub const HID_REPORT_SIZE: usize = 520;
pub const CONFIG_COMMAND: u8 = 5;
pub const CONFIG_CHECKSUM_START: usize = 2;
pub const CONFIG_CHECKSUM_END: usize = 0x207;
/// FUN_00490680 caps one frame's payload at 0x200 (512) bytes and advances by
/// that chunk; the F75's six-byte payload is sent in a single frame.
pub const CONFIG_CHUNK_SIZE: usize = 512;
/// FUN_00490680 calls FUN_00405100(0x14) before each configuration frame.
/// FUN_00405100 is a QueryPerformanceCounter busy-wait in milliseconds, so the
/// real pre-send delay is 20 ms (not the 30 ms sleep used elsewhere).
pub const CONFIG_FRAME_DELAY_MS: u64 = 20;
pub const CONFIG_RETRY_DELAY_MS: u64 = 60; // Sleep(0x3c) between retries
/// FUN_00489080 sleeps 0x64 (100 ms) only after SetLedRgbTab, the compressed
/// RGB table sent when the selected effect is not the self-defined one. It is
/// not the profile save delay; saving sleeps 0x1e (30 ms) instead.
pub const RGB_TABLE_DELAY_MS: u64 = 100;
/// Macro upload uses 0x400-byte buffers, one per macro id. FUN_00489080 calls
/// FUN_004881f0 with ids 0xDC, 0xE0, 0xE4 and 0xE8.
pub const MACRO_CHUNK_SIZE: usize = 1024;
pub const MACRO_COMMAND_IDS: [u8; 4] = [0xDC, 0xE0, 0xE4, 0xE8];
pub const HID_CMD_LED_MATRIX: u8 = 0xBC;
pub const HID_CMD_LED_COLOR: u8 = 0xC0;
pub const LED_SETTLE_MS: u64 = 50; // Sleep(0x32) after each LED command
pub const LED_RETRY_DELAY_MS: u64 = 200; // Sleep(200) in FUN_004879f0 retries
pub const LED_MATRIX_BYTES: usize = 1890;
/// FUN_00489080 sends the 1890-byte table as two 1024-byte payloads: the first
/// frame starts at matrix[0], the second at matrix[1024] (866 bytes + padding).
/// There is no leading gap; the earlier 8-byte lead-in was a misreading of the
/// stack pointer used by FUN_00487DA0.
pub const LED_MATRIX_FIRST_CHUNK: usize = 1024;
/// GameRGBIndex=0,1,2 from KB.ini; plane order for R, G and B in the 1890-byte
/// table. FUN_00489080 reads these from devState+0x2FFF..0x3001.
pub const GAME_RGB_INDEX: [u8; 3] = [0, 1, 2];
pub const LED_COLOR_GROUP_BYTES: usize = 378;
pub const LED_COLOR_GROUPS_USED: usize = 1;
pub const LED_PAYLOAD_BUFFER: usize = 1024;
pub const LED_FEATURE_REPORT_ID: usize = 6;
pub const LED_FEATURE_REPORT_SIZE: usize = 1032;
pub const LED_COLOR_PLANE_BYTES: usize = 126;
/// FUN_00410a70 parses up to 24 `LedOpt%d` records of 0x28 bytes each; the F75
/// KB.ini ships 20 of them.
pub const LED_EFFECT_SLOTS: usize = 24;
pub const VID_WIRED: u32 = 0x258A;
pub const PID_WIRED: u32 = 0x010C;
pub const VID_WIRELESS: u32 = 0x3554;
pub const PID_WIRELESS: u32 = 0xFA09;
/// KB.ini `SleepUI`/`SleepHW` (the F75 profile overrides the parser fallback).
/// FUN_00410a70 derives the hardware value as UI seconds / 0x1E.
pub const SLEEP_UI: [u16; 9] = [60, 90, 120, 180, 240, 300, 600, 900, 1200];
pub const SLEEP_HW: [u8; 9] = [2, 3, 4, 6, 8, 10, 20, 30, 40];
pub const SLEEP_DIVISOR: u16 = 30;
/// Built-in fallback in FUN_00410a70 when `SleepUI` is missing: 30..1200 s,
/// halved to the hardware units 1..40.
pub const BUILTIN_SLEEP_UI: [u16; 10] = [30, 60, 90, 120, 180, 240, 300, 600, 900, 1200];
pub const SPEED_LEVELS: u8 = 5;
pub const BRIGHTNESS_LEVELS: u8 = 10;
/// The parser stores only the packed `ShowDebounce=0x04000101`; it reads
/// optional DebounceUI/DebounceHW lists but discards them, so these four
/// millisecond values are still an estimate (see MECHANICS.md).
pub const DEBOUNCE_MS: [u8; 4] = [1, 2, 4, 8];
pub const DEFAULT_EFFECT: usize = 11; // Neon stream; KB.ini DefLedIndex=10, 0-based
/// FUN_00410a70 colour defaults when the keys are absent: clrKeyOv and
/// clrKeyDn are 0xFFFFFF; the F75 profile overrides clrKeyDn to 255,0,0.
/// clrKeyHasFunc is 0x1D45E8 (10,147,255), also the app's default colour.
pub const CLR_KEY_OV: [u8; 3] = [255, 255, 255];
pub const CLR_KEY_DN: [u8; 3] = [255, 0, 0];
pub const CLR_KEY_HAS_FUNC: [u8; 3] = [10, 147, 255];
/// Profile bytes patched by FUN_00488FA0 before the save frame.
/// `GaoshouIndex=18` in KB.ini; `StartGp=0x20` plus the selected group offset.
pub const GAOSHOU_INDEX: u8 = 18;
pub const START_GP: u8 = 0x20;
pub const PROFILE_GROUP_OFFSET: u8 = 0;
/// FUN_00488FA0 writes the per-key colour table at profile+0x16, two bytes per
/// key. Byte 0 is direction (or RandomVal=7 when random colour is on); byte 1 is
/// `(brightness << 4) | speed`. The F75 profile carries 80 key records.
pub const PROFILE_KEY_MAP_OFFSET: usize = 0x16;
pub const PROFILE_KEY_STRIDE: usize = 2;
pub const PROFILE_KEY_COUNT: usize = KEY_COUNT;
/// FUN_00410a70's RandomVal default, used when a key's random-colour flag is on.
pub const RANDOM_VAL: u8 = 7;

/// Key matrix sent by FUN_00487E60: `04 D4 00 40 00 00 00` + 1024 bytes.
/// FUN_00487F20 reads the same table back with `84 D4 00 00 00 00`; the
/// low-level sender prepends feature-report id 6 to both.
pub const KEY_MATRIX_BYTES: usize = 1024;
pub const KEY_MATRIX_READ_HEADER: [u8; 7] = [0x84, 0xD4, 0, 0, 0, 0, 0];
pub const KEY_MATRIX_WRITE_HEADER: [u8; 7] = [0x04, 0xD4, 0, 0x40, 0, 0, 0];
/// FUN_00488C50 iterates 132 key records, so matrix indices 0..=0x83 are
/// valid. The F75 profile has 80 keys; K1..K80 are stored in that order.
pub const KEY_MATRIX_SLOTS: usize = 132;
/// FUN_0048D2D0 prefixes the macro data with one 4-byte (offset, size) pair
/// per macro; FUN_0048CF00 stops at 30 UTF-16 characters.
pub const MACRO_INDEX_ENTRY_BYTES: usize = 4;
pub const MACRO_NAME_MAX_CHARS: usize = 30;
/// FUN_00488C50 accepts macro buffer ids 1..=0x20 and stores id-1 in the
/// high byte of the matrix dword.
pub const MACRO_MAX_ID: u8 = 0x20;

/// `(VK, HID usage)` table at VA 0x607290, 115 entries, read directly from
/// OemDrv.exe. FUN_0048CF00 searches it linearly for a macro key's VK.
pub static VK_TO_HID: [(u8, u8); 115] = [
    (0xE2, 0x64),
    (0x1B, 0x29),
    (0x70, 0x3A),
    (0x71, 0x3B),
    (0x72, 0x3C),
    (0x73, 0x3D),
    (0x74, 0x3E),
    (0x75, 0x3F),
    (0x76, 0x40),
    (0x77, 0x41),
    (0x78, 0x42),
    (0x79, 0x43),
    (0x7A, 0x44),
    (0x7B, 0x45),
    (0x7C, 0x68),
    (0x41, 0x04),
    (0x42, 0x05),
    (0x43, 0x06),
    (0x44, 0x07),
    (0x45, 0x08),
    (0x46, 0x09),
    (0x47, 0x0A),
    (0x48, 0x0B),
    (0x49, 0x0C),
    (0x4A, 0x0D),
    (0x4B, 0x0E),
    (0x4C, 0x0F),
    (0x4D, 0x10),
    (0x4E, 0x11),
    (0x4F, 0x12),
    (0x50, 0x13),
    (0x51, 0x14),
    (0x52, 0x15),
    (0x53, 0x16),
    (0x54, 0x17),
    (0x55, 0x18),
    (0x56, 0x19),
    (0x57, 0x1A),
    (0x58, 0x1B),
    (0x59, 0x1C),
    (0x5A, 0x1D),
    (0x31, 0x1E),
    (0x32, 0x1F),
    (0x33, 0x20),
    (0x34, 0x21),
    (0x35, 0x22),
    (0x36, 0x23),
    (0x37, 0x24),
    (0x38, 0x25),
    (0x39, 0x26),
    (0x30, 0x27),
    (0x09, 0x2B),
    (0x14, 0x39),
    (0x0D, 0x28),
    (0x08, 0x2A),
    (0x20, 0x2C),
    (0xDB, 0x2F),
    (0xDD, 0x30),
    (0xDC, 0x31),
    (0xBA, 0x33),
    (0xDE, 0x34),
    (0xBC, 0x36),
    (0xBE, 0x37),
    (0xBF, 0x38),
    (0xC0, 0x35),
    (0xBB, 0x2E),
    (0xBD, 0x2D),
    (0x2C, 0x46),
    (0x91, 0x47),
    (0x13, 0x48),
    (0x2D, 0x49),
    (0x24, 0x4A),
    (0x21, 0x4B),
    (0x2E, 0x4C),
    (0x23, 0x4D),
    (0x22, 0x4E),
    (0x26, 0x52),
    (0x28, 0x51),
    (0x25, 0x50),
    (0x27, 0x4F),
    (0x61, 0x59),
    (0x62, 0x5A),
    (0x63, 0x5B),
    (0x64, 0x5C),
    (0x65, 0x5D),
    (0x66, 0x5E),
    (0x67, 0x5F),
    (0x68, 0x60),
    (0x69, 0x61),
    (0x60, 0x62),
    (0x6B, 0x57),
    (0x6D, 0x56),
    (0x6A, 0x55),
    (0x6F, 0x54),
    (0x90, 0x53),
    (0xFD, 0x58),
    (0x6E, 0x63),
    (0xA2, 0xE0),
    (0xA0, 0xE1),
    (0xA4, 0xE2),
    (0x5B, 0xE3),
    (0xA3, 0xE4),
    (0xA1, 0xE5),
    (0xA5, 0xE6),
    (0x5C, 0xE7),
    (0x5D, 0x65),
    (0xC2, 0x85),
    (0xC1, 0x87),
    (0xC3, 0x89),
    (0xD8, 0x32),
    (0xDA, 0x8A),
    (0xD7, 0x88),
    (0xEB, 0x8B),
    (0xC4, 0x90),
    (0xC5, 0x91),
];

/// FUN_0048CF00 maps the eight Windows modifier VKs to the HID modifier
/// usages before it consults the table.
pub fn hid_for_vk(vk: u8) -> Option<u8> {
    match vk {
        0x10 | 0xA0 => Some(0xE1),
        0x11 | 0xA2 => Some(0xE0),
        0x12 | 0xA4 => Some(0xE2),
        0x5B => Some(0xE3),
        0x5C => Some(0xE7),
        0xA1 => Some(0xE5),
        0xA3 => Some(0xE4),
        0xA5 => Some(0xE6),
        _ => VK_TO_HID
            .iter()
            .find(|(candidate, _)| *candidate == vk)
            .map(|(_, hid)| *hid),
    }
}

/// The vendor's media/consumer codes are 32-bit usage values of the form
/// `0x02000000 | usage`, exactly as written in KB.ini `[FN1]`.
pub fn media_code_for_action(action: &str) -> Option<u32> {
    let usage: u32 = match action {
        "Play / Pause" => 0xCD,
        "Next track" => 0xB5,
        "Previous track" => 0xB6,
        "Stop" => 0xB7,
        "Mute" => 0xE2,
        "Volume up" => 0xE9,
        "Volume down" => 0xEA,
        "Media select" => 0x183,
        "Calculator" => 0x192,
        "Mail" => 0x18A,
        "Home page" => 0x223,
        "Browser back" => 0x224,
        "Browser forward" => 0x225,
        "Browser refresh" => 0x227,
        "Search" => 0x221,
        _ => return None,
    };
    Some(0x0200_0000 | usage)
}

/// VK for a key-assignment or macro label. The special list mirrors
/// `actions_for(0)` and KB.ini; one-character names cover letters, digits and
/// punctuation.
pub fn vk_for_action(action: &str) -> Option<u8> {
    let vk = match action {
        "Esc" => 0x1B,
        "Tab" => 0x09,
        "Enter" => 0x0D,
        "Space" => 0x20,
        "Backspace" => 0x08,
        "Delete" => 0x2E,
        "Insert" => 0x2D,
        "Home" => 0x24,
        "End" => 0x23,
        "Page Up" => 0x21,
        "Page Down" => 0x22,
        "Print Screen" => 0x2C,
        "Pause" => 0x13,
        "Scroll Lock" => 0x91,
        "Caps Lock" => 0x14,
        "Arrow Up" => 0x26,
        "Arrow Down" => 0x28,
        "Arrow Left" => 0x25,
        "Arrow Right" => 0x27,
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        "F11" => 0x7A,
        "F12" => 0x7B,
        "Left Ctrl" => 0xA2,
        "Left Shift" => 0xA0,
        "Left Alt" => 0xA4,
        "Left Win" => 0x5B,
        "Right Ctrl" => 0xC4,
        "Right Shift" => 0xA1,
        "Right Alt" => 0xA5,
        "-" => 0xBD,
        "=" => 0xBB,
        "[" => 0xDB,
        "]" => 0xDD,
        "\\" => 0xDC,
        ";" => 0xBA,
        "'" => 0xDE,
        "," => 0xBC,
        "." => 0xBE,
        "/" => 0xBF,
        "`" => 0xC0,
        _ => {
            let bytes = action.as_bytes();
            if bytes.len() == 1 {
                let ch = bytes[0];
                if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
                    return Some(ch);
                }
                if ch.is_ascii_lowercase() {
                    return Some(ch.to_ascii_uppercase());
                }
            }
            return KEYS
                .iter()
                .find(|key| key.label.eq_ignore_ascii_case(action))
                .map(|key| key.vk as u8);
        }
    };
    Some(vk)
}

/// The 32-bit value stored in the key matrix for an assignment.
/// Keyboard actions use their VK, mouse buttons use the vendor's 0xF0..0xF4
/// codes and media actions use the KB.ini `0x02000000 | usage` form.
/// Commands, combinations and "disable" are not decoded yet and return None.
pub fn key_code_for_action(category: u8, action: &str) -> Option<u32> {
    match category {
        0 => vk_for_action(action).map(u32::from),
        1 => match action {
            "Left click" => Some(0xF0),
            "Right click" => Some(0xF1),
            "Middle click" => Some(0xF2),
            "Double click" => Some(0xF3),
            "Back" => Some(0xF4),
            "Forward" => Some(0xF4),
            "Scroll up" => Some(0xF5),
            "Scroll down" => Some(0xF6),
            _ => None,
        },
        2 => media_code_for_action(action),
        _ => None,
    }
}

/// Macros are bound to a physical key by the vendor UI. The decoded trigger
/// list is F9..F12; those are KEYS indices 9..12.
pub fn macro_trigger_key(trigger: &str) -> Option<usize> {
    match trigger {
        "F9" => Some(9),
        "F10" => Some(10),
        "F11" => Some(11),
        "F12" => Some(12),
        _ => None,
    }
}

/// One physical key from KB.ini `[KEY]`. Coordinates are the original 600x233
/// keyboard diagram units, kept so the preview matches the vendor layout.
pub struct KeyDef {
    pub index: usize,
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub vk: u32,
    pub led: u8,
}

macro_rules! K {
    ($i:expr, $label:expr, $x:expr, $y:expr, $w:expr, $h:expr, $vk:expr, $led:expr) => {
        KeyDef {
            index: $i,
            label: $label,
            x: $x,
            y: $y,
            w: $w,
            h: $h,
            vk: $vk,
            led: $led,
        }
    };
}

pub static KEYS: [KeyDef; KEY_COUNT] = [
    K!(0, "Esc", 30.0, 30.0, 25.0, 27.0, 0x1B, 0),
    K!(1, "F1", 107.0, 30.0, 25.0, 27.0, 0x70, 12),
    K!(2, "F2", 144.0, 30.0, 25.0, 27.0, 0x71, 18),
    K!(3, "F3", 183.0, 30.0, 25.0, 27.0, 0x72, 24),
    K!(4, "F4", 222.0, 30.0, 25.0, 27.0, 0x73, 30),
    K!(5, "F5", 269.0, 30.0, 25.0, 27.0, 0x74, 36),
    K!(6, "F6", 305.0, 30.0, 25.0, 27.0, 0x75, 42),
    K!(7, "F7", 343.0, 30.0, 25.0, 27.0, 0x76, 48),
    K!(8, "F8", 388.0, 30.0, 25.0, 27.0, 0x77, 54),
    K!(9, "F9", 439.0, 30.0, 25.0, 27.0, 0x78, 60),
    K!(10, "F10", 477.0, 30.0, 25.0, 27.0, 0x79, 66),
    K!(11, "F11", 516.0, 30.0, 25.0, 27.0, 0x7A, 72),
    K!(12, "F12", 555.0, 30.0, 25.0, 27.0, 0x7B, 78),
    K!(13, "End", 605.0, 195.0, 25.0, 27.0, 0x23, 88),
    K!(14, "`", 31.0, 81.0, 25.0, 27.0, 0xC0, 1),
    K!(15, "1", 70.0, 81.0, 25.0, 27.0, 0x31, 7),
    K!(16, "2", 108.0, 81.0, 25.0, 27.0, 0x32, 13),
    K!(17, "3", 147.0, 81.0, 25.0, 27.0, 0x33, 19),
    K!(18, "4", 184.0, 81.0, 25.0, 27.0, 0x34, 25),
    K!(19, "5", 223.0, 81.0, 25.0, 27.0, 0x35, 31),
    K!(20, "6", 262.0, 81.0, 25.0, 27.0, 0x36, 37),
    K!(21, "7", 300.0, 80.0, 25.0, 27.0, 0x37, 43),
    K!(22, "8", 338.0, 81.0, 25.0, 27.0, 0x38, 49),
    K!(23, "9", 377.0, 81.0, 25.0, 27.0, 0x39, 55),
    K!(24, "0", 416.0, 81.0, 25.0, 27.0, 0x30, 61),
    K!(25, "-", 453.0, 81.0, 25.0, 27.0, 0xBD, 67),
    K!(26, "=", 491.0, 81.0, 25.0, 27.0, 0xBB, 73),
    K!(27, "Bksp", 531.0, 81.0, 60.0, 27.0, 0x08, 79),
    K!(28, "Del", 604.0, 81.0, 25.0, 27.0, 0x2E, 85),
    K!(29, "Tab", 34.0, 120.0, 44.0, 27.0, 0x09, 2),
    K!(30, "Q", 89.0, 119.0, 25.0, 27.0, 0x51, 8),
    K!(31, "W", 127.0, 119.0, 25.0, 27.0, 0x57, 14),
    K!(32, "E", 165.0, 119.0, 25.0, 27.0, 0x45, 20),
    K!(33, "R", 204.0, 119.0, 25.0, 27.0, 0x52, 26),
    K!(34, "T", 242.0, 119.0, 25.0, 27.0, 0x54, 32),
    K!(35, "Y", 280.0, 119.0, 25.0, 27.0, 0x59, 38),
    K!(36, "U", 318.0, 119.0, 25.0, 27.0, 0x55, 44),
    K!(37, "I", 356.0, 119.0, 25.0, 27.0, 0x49, 50),
    K!(38, "O", 395.0, 119.0, 25.0, 27.0, 0x4F, 56),
    K!(39, "P", 433.0, 119.0, 25.0, 27.0, 0x50, 62),
    K!(40, "[", 471.0, 120.0, 25.0, 27.0, 0xDB, 68),
    K!(41, "]", 509.0, 119.0, 25.0, 27.0, 0xDD, 74),
    K!(42, "\\", 549.0, 119.0, 44.0, 27.0, 0xDC, 80),
    K!(43, "PgUp", 605.0, 120.0, 25.0, 27.0, 0x21, 86),
    K!(44, "Caps", 33.0, 158.0, 52.0, 27.0, 0x14, 3),
    K!(45, "A", 98.0, 158.0, 25.0, 27.0, 0x41, 9),
    K!(46, "S", 136.0, 158.0, 25.0, 27.0, 0x53, 15),
    K!(47, "D", 174.0, 158.0, 25.0, 27.0, 0x44, 21),
    K!(48, "F", 213.0, 158.0, 25.0, 27.0, 0x46, 27),
    K!(49, "G", 252.0, 158.0, 25.0, 27.0, 0x47, 33),
    K!(50, "H", 290.0, 158.0, 25.0, 27.0, 0x48, 39),
    K!(51, "J", 327.0, 158.0, 25.0, 27.0, 0x4A, 45),
    K!(52, "K", 366.0, 158.0, 25.0, 27.0, 0x4B, 51),
    K!(53, "L", 405.0, 158.0, 25.0, 27.0, 0x4C, 57),
    K!(54, ";", 443.0, 158.0, 25.0, 27.0, 0xBA, 63),
    K!(55, "'", 481.0, 158.0, 25.0, 27.0, 0xDE, 69),
    K!(56, "Enter", 521.0, 158.0, 70.0, 27.0, 0x0D, 81),
    K!(57, "Shift", 33.0, 195.0, 72.0, 27.0, 0xA0, 4),
    K!(58, "Z", 116.0, 195.0, 25.0, 27.0, 0x5A, 10),
    K!(59, "X", 155.0, 195.0, 25.0, 27.0, 0x58, 16),
    K!(60, "C", 193.0, 195.0, 25.0, 27.0, 0x43, 22),
    K!(61, "V", 232.0, 195.0, 25.0, 27.0, 0x56, 28),
    K!(62, "B", 270.0, 196.0, 25.0, 27.0, 0x42, 34),
    K!(63, "N", 309.0, 195.0, 25.0, 27.0, 0x4E, 40),
    K!(64, "M", 347.0, 195.0, 25.0, 27.0, 0x4D, 46),
    K!(65, ",", 384.0, 195.0, 25.0, 27.0, 0xBC, 52),
    K!(66, ".", 424.0, 195.0, 25.0, 27.0, 0xBE, 58),
    K!(67, "/", 463.0, 195.0, 25.0, 27.0, 0xBF, 64),
    K!(68, "Shift", 502.0, 195.0, 56.0, 27.0, 0xA1, 70),
    K!(69, "Up", 567.0, 195.0, 25.0, 27.0, 0x26, 82),
    K!(70, "Ctrl", 34.0, 234.0, 35.0, 27.0, 0xA2, 5),
    K!(71, "Win", 79.0, 234.0, 35.0, 27.0, 0x5B, 11),
    K!(72, "Alt", 122.0, 235.0, 35.0, 27.0, 0xA4, 17),
    K!(73, "Space", 174.0, 235.0, 226.0, 27.0, 0x20, 35),
    K!(74, "Ctrl", 463.0, 236.0, 34.0, 27.0, 0xC4, 59),
    K!(75, "Fn", 414.0, 235.0, 35.0, 27.0, 0xFA, 53),
    K!(76, "Left", 529.0, 234.0, 25.0, 27.0, 0x25, 77),
    K!(77, "Down", 568.0, 234.0, 25.0, 27.0, 0x28, 83),
    K!(78, "Right", 605.0, 234.0, 25.0, 27.0, 0x27, 89),
    K!(79, "PgDn", 604.0, 159.0, 25.0, 27.0, 0x22, 87),
];

/// Lighting effects and their capability flags, from `LedOpt1..LedOpt20` in
/// KB.ini: (UI index, hardware id, speed, brightness, direction, random,
/// colour).
pub struct Effect {
    pub ui: usize,
    pub name: &'static str,
    pub hw: u8,
    pub speed: bool,
    pub brightness: bool,
    pub direction: bool,
    pub random: bool,
    pub color: bool,
}

macro_rules! E {
    ($ui:expr, $name:expr, $hw:expr, $sp:expr, $br:expr, $dir:expr, $rnd:expr, $col:expr) => {
        Effect {
            ui: $ui,
            name: $name,
            hw: $hw,
            speed: $sp,
            brightness: $br,
            direction: $dir,
            random: $rnd,
            color: $col,
        }
    };
}

pub static EFFECTS: [Effect; 20] = [
    E!(1, "Fixed on", 1, false, true, false, true, true),
    E!(2, "Respire", 3, true, true, false, true, true),
    E!(3, "Rainbow", 2, true, true, false, false, false),
    E!(4, "Flash away", 19, true, true, false, true, true),
    E!(5, "Raindrops", 15, true, true, false, true, true),
    E!(6, "Rainbow wheel", 13, true, true, false, true, true),
    E!(7, "Ripples shining", 20, true, true, false, true, true),
    E!(8, "Stars twinkle", 16, true, true, false, true, true),
    E!(9, "Shadow disappear", 18, true, true, false, true, true),
    E!(10, "Retro snake", 5, true, true, false, true, true),
    E!(11, "Neon stream", 7, true, true, false, true, true),
    E!(12, "Reaction", 17, true, true, false, true, true),
    E!(13, "Sine wave", 12, true, true, false, true, true),
    E!(14, "Retinue scanning", 8, true, true, false, true, true),
    E!(15, "Rotating windmill", 28, true, true, false, false, false),
    E!(
        16,
        "Colorful waterfall",
        30,
        true,
        true,
        false,
        false,
        false
    ),
    E!(17, "Blossoming", 14, true, true, false, false, false),
    E!(18, "Rotating storm", 29, true, true, false, true, true),
    E!(19, "Self-define", 0, false, false, false, false, false),
    E!(20, "Off", 0, false, false, false, false, false),
];

pub fn effect_by_ui(ui: usize) -> &'static Effect {
    EFFECTS
        .iter()
        .find(|e| e.ui == ui)
        .unwrap_or(&EFFECTS[DEFAULT_EFFECT - 1])
}

pub static PALETTE: [[u8; 3]; 10] = [
    [255, 255, 255],
    [255, 59, 48],
    [255, 159, 10],
    [255, 214, 10],
    [48, 209, 88],
    [50, 215, 180],
    [100, 210, 255],
    [10, 132, 255],
    [191, 90, 242],
    [255, 55, 95],
];

pub static KEY_CATEGORIES: [&str; 6] = [
    "Keyboard",
    "Mouse",
    "Multimedia",
    "Commands",
    "Combination",
    "Disable",
];

pub fn actions_for(category: usize) -> &'static [&'static str] {
    match category {
        0 => &[
            "Esc",
            "Tab",
            "Enter",
            "Space",
            "Backspace",
            "Delete",
            "Insert",
            "Home",
            "End",
            "Page Up",
            "Page Down",
            "Print Screen",
            "Pause",
            "Scroll Lock",
            "Caps Lock",
            "Arrow Up",
            "Arrow Down",
            "Arrow Left",
            "Arrow Right",
            "F1",
            "F2",
            "F3",
            "F4",
            "F5",
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "F11",
            "F12",
            "Left Ctrl",
            "Left Shift",
            "Left Alt",
            "Left Win",
            "Right Ctrl",
            "Right Shift",
            "Right Alt",
        ],
        1 => &[
            "Left click",
            "Right click",
            "Middle click",
            "Double click",
            "Scroll up",
            "Scroll down",
            "Scroll left",
            "Scroll right",
            "Mouse move up",
            "Mouse move down",
            "Mouse move left",
            "Mouse move right",
            "Back",
            "Forward",
            "DPI cycle",
        ],
        2 => &[
            "Play / Pause",
            "Next track",
            "Previous track",
            "Stop",
            "Mute",
            "Volume up",
            "Volume down",
            "Media select",
            "Calculator",
            "Mail",
            "Home page",
            "Browser back",
            "Browser forward",
            "Browser refresh",
            "Search",
        ],
        3 => &[
            "Close window",
            "Lock computer",
            "Run program",
            "Show desktop",
            "Zoom in",
            "Zoom out",
            "Task manager",
            "Undo",
            "Save",
            "Lock Win key",
            "Lock keyboard",
            "Sleep",
            "Reset",
        ],
        4 => &[
            "Ctrl + C",
            "Ctrl + V",
            "Ctrl + X",
            "Ctrl + Z",
            "Ctrl + A",
            "Ctrl + S",
            "Ctrl + F",
            "Ctrl + Shift + Esc",
            "Alt + Tab",
            "Alt + F4",
            "Win + L",
            "Win + E",
            "Win + D",
            "Shift + F10",
        ],
        _ => &["Disable this key"],
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    En,
    Ru,
}

pub fn tr(lang: Lang, en: &'static str, ru: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Ru => ru,
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub category: u8,
    pub action: String,
}

impl Assignment {
    pub fn new(category: u8, action: &str) -> Self {
        Self {
            category,
            action: action.to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Lighting {
    pub effect: usize,
    pub brightness: u8, // 0..9 UI index, device gets index + 1
    pub speed: u8,      // 0..4
    pub direction: u8,  // 0 left, 1 right, 2 up, 3 down
    pub random_color: bool,
    pub color: [u8; 3],
    pub per_key: Vec<[u8; 3]>,
    pub side_led: bool,
    pub paint: bool,
}

impl Default for Lighting {
    fn default() -> Self {
        let color = CLR_KEY_HAS_FUNC;
        Self {
            effect: DEFAULT_EFFECT,
            brightness: 7,
            speed: 2,
            direction: 1,
            random_color: false,
            color,
            per_key: vec![color; KEY_COUNT],
            side_led: true,
            paint: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct KeyMap {
    pub base: Vec<Option<Assignment>>,
    pub func: Vec<Option<Assignment>>,
}

impl Default for KeyMap {
    fn default() -> Self {
        // Empty means "factory keyboard". Only keys the person remaps are
        // overlaid on the matrix read back from the device, so an empty map
        // never disables a working key.
        Self {
            base: vec![None; KEY_COUNT],
            func: vec![None; KEY_COUNT],
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProfileData {
    pub lighting: Lighting,
    pub keys: KeyMap,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub built_in: bool,
    pub data: ProfileData,
    pub macros: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Key,
    Mouse,
    Delay,
    Text,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MacroStep {
    pub kind: StepKind,
    pub text: String,
    pub delay_ms: u32,
    pub holding_ms: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub trigger: String,
    pub mode: u8, // 0 cycles, 1 until released, 2 until any key
    pub cycles: u32,
    pub steps: Vec<MacroStep>,
}

impl Macro {
    /// Encoded size for the vendor macro buffer (FUN_0048CF00/0x48D2D0):
    /// one length byte plus UTF-16LE name, then 4 bytes for every key-down and
    /// key-up action. A delay has no record of its own; it is folded into the
    /// next action's 20-bit time value.
    pub fn bytes(&self) -> usize {
        let name = 1 + self.name.chars().count().min(MACRO_NAME_MAX_CHARS) * 2;
        name + self
            .steps
            .iter()
            .map(|s| match s.kind {
                StepKind::Delay => 0,
                StepKind::Text => s.text.chars().count() * 8,
                StepKind::Key | StepKind::Mouse => 8,
            })
            .sum::<usize>()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub lang: Lang,
    pub autostart: bool,
    pub remember_page: bool,
    pub close_to_tray: bool,
    pub sleep_index: usize,
    pub debounce: u8,
    pub show_fn_tip: bool,
    /// 0 = off, 1 = errors only, 2 = normal, 3 = verbose. The vendor's
    /// Cfg.ini [OPT] Debug key is 0 by default; Linux defaults to 2 so a
    /// report exists without the person having to switch anything on.
    #[serde(default = "default_log_level")]
    pub log_level: u8,
    #[serde(default = "default_log_to_file")]
    pub log_to_file: bool,
}

fn default_log_level() -> u8 {
    2
}

fn default_log_to_file() -> bool {
    true
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            lang: Lang::En,
            autostart: false,
            remember_page: true,
            close_to_tray: false,
            sleep_index: 5, // 300 s, the KB.ini default
            debounce: 1,
            show_fn_tip: true,
            log_level: default_log_level(),
            log_to_file: default_log_to_file(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppState {
    pub page: usize,
    pub lighting: Lighting,
    pub keys: KeyMap,
    pub profiles: Vec<Profile>,
    pub active_profile: usize,
    pub macros: Vec<Macro>,
    pub prefs: Preferences,
    pub active_key: usize,
    pub action_category: usize,
    pub action_choice: usize,
    pub fn_layer: bool,
    pub macro_selected: usize,
    pub recording: bool,
    pub music_mode: u8,
    pub music_sensitivity: u8,
}

impl Default for AppState {
    fn default() -> Self {
        let lighting = Lighting::default();
        let keys = KeyMap::default();
        let mut profiles = Vec::new();
        let names = ["Default", "Night typing", "Gaming", "Work", "Streaming"];
        for (i, name) in names.iter().enumerate() {
            let mut data = ProfileData {
                lighting: lighting.clone(),
                keys: keys.clone(),
            };
            if i == 1 {
                data.lighting.effect = 1;
                data.lighting.color = [255, 159, 10];
                data.lighting.brightness = 3;
            } else if i == 2 {
                data.lighting.effect = 12;
                data.lighting.color = [255, 55, 95];
                data.lighting.speed = 4;
            } else if i == 3 {
                data.lighting.effect = 11;
                data.lighting.color = [48, 209, 88];
                data.lighting.brightness = 5;
            } else if i == 4 {
                data.lighting.effect = 6;
                data.lighting.color = [191, 90, 242];
            }
            profiles.push(Profile {
                name: name.to_string(),
                built_in: i == 0,
                data,
                macros: [0, 0, 2, 1, 3][i],
            });
        }
        let macros = vec![
            Macro {
                name: "Rapid fire".into(),
                trigger: "Mouse 4".into(),
                mode: 1,
                cycles: 1,
                steps: vec![
                    MacroStep {
                        kind: StepKind::Key,
                        text: "E".into(),
                        delay_ms: 40,
                        holding_ms: 20,
                    },
                    MacroStep {
                        kind: StepKind::Delay,
                        text: String::new(),
                        delay_ms: 40,
                        holding_ms: 0,
                    },
                    MacroStep {
                        kind: StepKind::Key,
                        text: "E".into(),
                        delay_ms: 40,
                        holding_ms: 20,
                    },
                    MacroStep {
                        kind: StepKind::Delay,
                        text: String::new(),
                        delay_ms: 40,
                        holding_ms: 0,
                    },
                    MacroStep {
                        kind: StepKind::Key,
                        text: "E".into(),
                        delay_ms: 40,
                        holding_ms: 20,
                    },
                ],
            },
            Macro {
                name: "Copy / paste".into(),
                trigger: "F9".into(),
                mode: 0,
                cycles: 1,
                steps: vec![
                    MacroStep {
                        kind: StepKind::Key,
                        text: "Ctrl + C".into(),
                        delay_ms: 120,
                        holding_ms: 30,
                    },
                    MacroStep {
                        kind: StepKind::Delay,
                        text: String::new(),
                        delay_ms: 120,
                        holding_ms: 0,
                    },
                    MacroStep {
                        kind: StepKind::Key,
                        text: "Ctrl + V".into(),
                        delay_ms: 120,
                        holding_ms: 30,
                    },
                ],
            },
            Macro {
                name: "Open workspace".into(),
                trigger: "F10".into(),
                mode: 0,
                cycles: 1,
                steps: vec![
                    MacroStep {
                        kind: StepKind::Key,
                        text: "Win + E".into(),
                        delay_ms: 200,
                        holding_ms: 40,
                    },
                    MacroStep {
                        kind: StepKind::Delay,
                        text: String::new(),
                        delay_ms: 600,
                        holding_ms: 0,
                    },
                    MacroStep {
                        kind: StepKind::Mouse,
                        text: "Double click".into(),
                        delay_ms: 150,
                        holding_ms: 20,
                    },
                ],
            },
        ];
        Self {
            page: 0,
            lighting,
            keys,
            profiles,
            active_profile: 0,
            macros,
            prefs: Preferences::default(),
            active_key: 0,
            action_category: 2,
            action_choice: 0,
            fn_layer: false,
            macro_selected: 0,
            recording: false,
            music_mode: 1,
            music_sensitivity: 6,
        }
    }
}

impl AppState {
    pub fn is_remapped(&self, key: usize) -> bool {
        let map = if self.fn_layer {
            &self.keys.func
        } else {
            &self.keys.base
        };
        map.get(key).map(|a| a.is_some()).unwrap_or(false)
    }

    pub fn assignment(&self, key: usize) -> Option<&Assignment> {
        let map = if self.fn_layer {
            &self.keys.func
        } else {
            &self.keys.base
        };
        map.get(key).and_then(|a| a.as_ref())
    }

    pub fn set_assignment(&mut self, key: usize, value: Option<Assignment>) {
        let map = if self.fn_layer {
            &mut self.keys.func
        } else {
            &mut self.keys.base
        };
        if key < map.len() {
            map[key] = value;
        }
    }
}
