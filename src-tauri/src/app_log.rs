use serde::Serialize;
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LINES: usize = 2_000;
static LOGS: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();

fn logs() -> &'static Mutex<VecDeque<LogEntry>> {
    LOGS.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub origin: String,
    pub level: String,
    pub message: String,
}

pub struct TeeWriter;
impl Write for TeeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        io::stderr().write_all(buffer)?;
        let text = String::from_utf8_lossy(buffer);
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            push("rust", detect_level(line), line);
        }
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}
pub fn make_writer() -> TeeWriter {
    TeeWriter
}
pub fn push_frontend(level: &str, message: &str) {
    push("webview", level, message);
}
pub fn snapshot() -> Vec<LogEntry> {
    logs()
        .lock()
        .map(|logs| logs.iter().cloned().collect())
        .unwrap_or_default()
}
fn push(origin: &str, level: &str, message: &str) {
    if let Ok(mut logs) = logs().lock() {
        logs.push_back(LogEntry {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_millis() as u64)
                .unwrap_or(0),
            origin: origin.into(),
            level: level.into(),
            message: message.trim().into(),
        });
        while logs.len() > MAX_LINES {
            logs.pop_front();
        }
    }
}
fn detect_level(line: &str) -> &'static str {
    let upper = line.to_ascii_uppercase();
    if upper.contains(" ERROR ") {
        "error"
    } else if upper.contains(" WARN ") {
        "warn"
    } else if upper.contains(" DEBUG ") || upper.contains(" TRACE ") {
        "debug"
    } else {
        "info"
    }
}
