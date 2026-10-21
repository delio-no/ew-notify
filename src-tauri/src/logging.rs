//! In-memory log ring for the UI console (also tees to app.log).

use serde::Serialize;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// `app.log` next to the executable (same directory as config.json).
pub fn get_log_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join("app.log");
        }
    }
    PathBuf::from("app.log")
}

const MAX_LINES: usize = 500;

#[derive(Clone, Serialize)]
pub struct LogLine {
    pub time: String,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogLine>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES))),
        }
    }

    pub fn push_raw(&self, chunk: &str) {
        for line in chunk.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                continue;
            }
            let cleaned = strip_ansi(trimmed);
            if cleaned.is_empty() {
                continue;
            }
            let entry = LogLine {
                time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                message: cleaned,
            };
            if let Ok(mut q) = self.inner.lock() {
                q.push_back(entry);
                while q.len() > MAX_LINES {
                    q.pop_front();
                }
            }
        }
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.inner
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut q) = self.inner.lock() {
            q.clear();
        }
    }
}

/// Writer that appends to the ring buffer and optionally to a file.
pub struct TeeWriter {
    buffer: LogBuffer,
    file: Option<Arc<Mutex<File>>>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.buffer.push_raw(&text);
        if let Some(file) = &self.file {
            if let Ok(mut f) = file.lock() {
                let _ = f.write_all(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &self.file {
            if let Ok(mut f) = file.lock() {
                let _ = f.flush();
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct TeeMakeWriter {
    buffer: LogBuffer,
    file: Option<Arc<Mutex<File>>>,
}

impl TeeMakeWriter {
    pub fn new(buffer: LogBuffer, file: Option<File>) -> Self {
        Self {
            buffer,
            file: file.map(|f| Arc::new(Mutex::new(f))),
        }
    }
}

impl<'a> MakeWriter<'a> for TeeMakeWriter {
    type Writer = TeeWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TeeWriter {
            buffer: self.buffer.clone(),
            file: self.file.clone(),
        }
    }
}

pub fn init(buffer: LogBuffer) {
    let path = get_log_path();
    let file = File::create(&path).ok();
    let maker = TeeMakeWriter::new(buffer, file);
    let _ = tracing_subscriber::fmt()
        .with_writer(maker)
        .with_ansi(false)
        .try_init();
    tracing::info!("Log file: {}", path.display());
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for x in chars.by_ref() {
                    if x.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}
