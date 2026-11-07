//! Small, dependency-free logging core.
//!
//! The Windows driver logs through function pointers selected at startup
//! (FUN_00407770): with `[OPT] Debug=0` logging is off, `Debug=1` sends lines
//! to OutputDebugString and `Debug>1` appends them to `Log.txt`. On Linux the
//! same three levels map to a file, stderr and the in-app log page.
//!
//! Every public entry point is safe to call from worker threads. The ring
//! buffer is capped, the file is appended line by line and a panic hook keeps
//! the last crash in the same file.

use crate::model::{LOG_FILE_NAME, LOG_LEVEL_COUNT, LOG_MAX_BYTES, LOG_RING_CAP};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
        }
    }

}

#[derive(Clone)]
pub struct Entry {
    pub at: String,
    pub level: Level,
    pub module: String,
    pub message: String,
}

impl Entry {
    pub fn line(&self) -> String {
        format_line(
            &self.at,
            self.level,
            &self.module,
            self.message.as_str(),
        )
    }
}

fn format_line(at: &str, level: Level, module: &str, message: &str) -> String {
    // The vendor writes "\nMM:SS.mmm   " before each message (strings at
    // 0x5FAAC0 and 0x5FAB08). We keep the three spaces and add the hour so a
    // file that spans a longer session stays readable.
    format!("{at}   {:<5} {module}: {message}", level.label())
}

struct Store {
    entries: VecDeque<Entry>,
    counters: [u64; LOG_LEVEL_COUNT],
    level: u8,
    file_enabled: bool,
    file: Option<File>,
    path: PathBuf,
    bytes: u64,
}

impl Store {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            counters: [0; LOG_LEVEL_COUNT],
            level: 2,
            file_enabled: true,
            file: None,
            path: default_path(),
            bytes: 0,
        }
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

pub fn dir() -> PathBuf {
    home().join(".config/f75-control")
}

pub fn default_path() -> PathBuf {
    dir().join(LOG_FILE_NAME)
}

fn lock() -> MutexGuard<'static, Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE
        .get_or_init(|| Mutex::new(Store::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// True when a message at `level` should be kept for a given 0..3 setting.
fn records(config: u8, level: Level) -> bool {
    match config {
        0 => false,
        // Debug=1 in the vendor file is the OutputDebugString-only mode; it
        // carries errors, not the whole conversation.
        1 => level == Level::Error,
        _ => (level as u8) <= config,
    }
}

fn push_bounded(ring: &mut VecDeque<Entry>, entry: Entry, cap: usize) {
    while ring.len() >= cap {
        ring.pop_front();
    }
    ring.push_back(entry);
}

/// Start a session: truncate the log like the vendor does, but first move a
/// file that already grew past the cap to `Log.1.txt` so a crash report is not
/// lost. Call once, before the window opens.
pub fn init(path: PathBuf, level: u8, file_enabled: bool) {
    let mut store = lock();
    store.path = path;
    store.level = level.min((LOG_LEVEL_COUNT - 1) as u8);
    store.file_enabled = file_enabled && store.level > 0;
    store.entries.clear();
    store.counters = [0; LOG_LEVEL_COUNT];
    store.file = None;
    store.bytes = 0;

    if let Some(parent) = store.path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if store.file_enabled {
        if let Ok(meta) = fs::metadata(&store.path) {
            if meta.len() > LOG_MAX_BYTES {
                let rotated = store.path.with_file_name("Log.1.txt");
                let _ = fs::remove_file(&rotated);
                let _ = fs::rename(&store.path, &rotated);
            }
        }
        match File::create(&store.path) {
            Ok(_) => {
                store.file = open_append(&store.path);
            }
            Err(err) => {
                eprintln!("cannot create log file {}: {err}", store.path.display());
                store.file_enabled = false;
            }
        }
    }
    let path = store.path.clone();
    let level = store.level;
    let to_file = store.file_enabled;
    drop(store);

    info(
        "app",
        format!(
            "--- session start · F75 Control Center v{} · level {level} · file {to_file} · {}",
            crate::model::APP_VERSION,
            path.display()
        ),
    );
}

fn open_append(path: &PathBuf) -> Option<File> {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => Some(file),
        Err(err) => {
            // Cannot log this through log(): that would recurse.
            eprintln!("cannot open log file {}: {err}", path.display());
            None
        }
    }
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    if ok {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec,
            now.subsec_millis()
        )
    } else {
        format!("00:00:{:02}.{:03}", secs % 60, now.subsec_millis())
    }
}

/// Record one line. Messages above the configured level are dropped; errors
/// are also echoed to stderr so a terminal launch shows them immediately.
pub fn log(level: Level, module: &str, message: impl AsRef<str>) {
    let mut store = lock();
    if !records(store.level, level) {
        return;
    }
    store.counters[level as usize] += 1;
    let entry = Entry {
        at: timestamp(),
        level,
        module: module.to_string(),
        message: message.as_ref().to_string(),
    };
    let line = entry.line();
    push_bounded(&mut store.entries, entry, LOG_RING_CAP);

    if level == Level::Error || store.level == 1 {
        eprintln!("{line}");
        // Level 1 is the "debugger only" mode; nothing is written to disk.
        if !store.file_enabled || store.level < 2 {
            return;
        }
    }

    if store.level >= 2 && store.file_enabled {
        if store.file.is_none() {
            store.file = open_append(&store.path);
        }
        if let Some(file) = store.file.as_mut() {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
            store.bytes += line.len() as u64 + 1;
            if store.bytes > LOG_MAX_BYTES {
                rotate(&mut store);
            }
        }
    }
}

fn rotate(store: &mut Store) {
    store.file = None;
    let rotated = store.path.with_file_name("Log.1.txt");
    let _ = fs::remove_file(&rotated);
    let _ = fs::rename(&store.path, &rotated);
    match File::create(&store.path) {
        Ok(_) => {
            store.bytes = 0;
            store.file = open_append(&store.path);
        }
        Err(err) => {
            eprintln!("cannot rotate log file: {err}");
            store.file_enabled = false;
        }
    }
}

pub fn error(module: &str, message: impl AsRef<str>) {
    log(Level::Error, module, message);
}

pub fn warn(module: &str, message: impl AsRef<str>) {
    log(Level::Warn, module, message);
}

pub fn info(module: &str, message: impl AsRef<str>) {
    log(Level::Info, module, message);
}

pub fn debug(module: &str, message: impl AsRef<str>) {
    log(Level::Debug, module, message);
}

pub fn snapshot() -> Vec<Entry> {
    lock().entries.iter().cloned().collect()
}

pub fn counts() -> [u64; LOG_LEVEL_COUNT] {
    lock().counters
}

pub fn level() -> u8 {
    lock().level
}

pub fn set_level(value: u8) {
    let value = value.min((LOG_LEVEL_COUNT - 1) as u8);
    lock().level = value;
    info("log", format!("log level set to {value}"));
}

pub fn file_enabled() -> bool {
    lock().file_enabled
}

pub fn set_file_enabled(enabled: bool) {
    let mut store = lock();
    store.file_enabled = enabled && store.level > 0;
    if store.file_enabled {
        if let Some(parent) = store.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        store.file = open_append(&store.path);
    } else {
        store.file = None;
    }
    let enabled = store.file_enabled;
    drop(store);
    info("log", format!("writing to file: {enabled}"));
}

pub fn log_path() -> PathBuf {
    lock().path.clone()
}

pub fn file_size() -> u64 {
    fs::metadata(log_path()).map(|m| m.len()).unwrap_or(0)
}

/// Human-readable lines, newest last. Used by the page and by export.
pub fn text(entries: &[Entry]) -> String {
    entries.iter().map(Entry::line).collect::<Vec<_>>().join("\n")
}

pub fn clear() {
    let mut store = lock();
    store.entries.clear();
    store.counters = [0; LOG_LEVEL_COUNT];
    store.file = None;
    store.bytes = 0;
    if store.file_enabled {
        let _ = File::create(&store.path);
        store.file = open_append(&store.path);
    }
    drop(store);
    info("log", "log cleared");
}

/// A plain-text report next to the log: version, system, settings, counters
/// and every line still in the ring. Returns the file that was written.
pub fn export_diagnostics() -> Result<PathBuf, String> {
    let path = dir().join("diagnostics.txt");
    let entries = snapshot();
    let counts = counts();
    let mut report = String::new();
    report.push_str("F75 Control Center diagnostics\n");
    report.push_str(&format!("version: {}\n", crate::model::APP_VERSION));
    report.push_str(&format!(
        "system: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    report.push_str(&format!("log file: {}\n", log_path().display()));
    report.push_str(&format!(
        "level: {} · file: {}\n",
        level(),
        file_enabled()
    ));
    report.push_str(&format!(
        "counters: errors {} · warnings {} · info {} · debug {}\n",
        counts[0], counts[1], counts[2], counts[3]
    ));
    report.push_str(&format!("entries kept: {}\n\n", entries.len()));
    report.push_str(&text(&entries));
    report.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, report).map_err(|e| e.to_string())?;
    info("log", format!("diagnostics written to {}", path.display()));
    Ok(path)
}

/// Keep a panic in the log before the default hook prints it. The previous
/// hook is chained so a terminal launch still gets the usual message.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".into());
        let message = if let Some(text) = info.payload().downcast_ref::<&str>() {
            (*text).to_string()
        } else if let Some(text) = info.payload().downcast_ref::<String>() {
            text.clone()
        } else {
            "non-string panic payload".into()
        };
        error("panic", format!("{message} at {location}"));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seq: u64) -> Entry {
        Entry {
            at: "12:34:56.789".into(),
            level: Level::Info,
            module: "test".into(),
            message: format!("line {seq}"),
        }
    }

    #[test]
    fn level_zero_records_nothing() {
        assert!(!records(0, Level::Error));
        assert!(!records(0, Level::Debug));
    }

    #[test]
    fn level_one_keeps_errors_only() {
        assert!(records(1, Level::Error));
        assert!(!records(1, Level::Warn));
    }

    #[test]
    fn level_two_keeps_info_and_above() {
        assert!(records(2, Level::Info));
        assert!(!records(2, Level::Debug));
    }

    #[test]
    fn level_three_keeps_everything() {
        assert!(records(3, Level::Debug));
    }

    #[test]
    fn line_has_time_level_and_module() {
        let line = format_line("09:05:01.042", Level::Warn, "device", "no access");
        assert_eq!(line, "09:05:01.042   WARN  device: no access");
    }

    #[test]
    fn file_is_truncated_at_start_and_receives_lines() {
        let path = std::env::temp_dir().join(format!("f75-log-test-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        init(path.clone(), 3, true);
        info("test", "first session marker");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("first session marker"));
        assert!(text.contains("session start"));

        init(path.clone(), 3, true);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("first session marker"));
        assert!(text.contains("session start"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ring_drops_the_oldest_entry() {
        let mut ring = VecDeque::new();
        for seq in 0..(LOG_RING_CAP + 5) {
            push_bounded(&mut ring, entry(seq as u64), LOG_RING_CAP);
        }
        assert_eq!(ring.len(), LOG_RING_CAP);
        assert_eq!(ring.front().unwrap().message, "line 5");
        assert_eq!(
            ring.back().unwrap().message,
            format!("line {}", LOG_RING_CAP + 4)
        );
    }
}
