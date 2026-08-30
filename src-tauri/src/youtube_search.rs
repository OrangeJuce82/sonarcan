//! Fast, bounded yt-dlp metadata search supervision.

use std::{
    collections::HashMap,
    io::{self, Read},
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use tracing::info;

use crate::{
    error::AppError,
    importer::{self, CandidateKind, ImportCandidate},
};

const MAX_CONCURRENT_SEARCHES: usize = 2;
const MAX_QUERY_BYTES: usize = 180;
const MAX_STDOUT_BYTES: usize = 512 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone, Default)]
pub struct YoutubeSearchService {
    inner: Arc<SearchInner>,
}

#[derive(Default)]
struct SearchInner {
    generation: AtomicU64,
    next_child_id: AtomicU64,
    children: Mutex<HashMap<u64, Arc<Mutex<Child>>>>,
    active_count: Mutex<usize>,
    slot_available: Condvar,
}

struct SearchPermit {
    inner: Arc<SearchInner>,
}

impl Drop for SearchPermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.inner.active_count.lock() {
            *active = active.saturating_sub(1);
            self.inner.slot_available.notify_one();
        }
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl YoutubeSearchService {
    pub fn begin(&self) -> u64 {
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if let Ok(mut children) = self.inner.children.lock() {
            for child in children.drain().map(|(_, child)| child) {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
        }
        self.inner.slot_available.notify_all();
        generation
    }

    pub fn resolve(&self, query: &str, generation: u64) -> Result<Vec<ImportCandidate>, AppError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(AppError::BackgroundTask(
                "YouTube search must contain between 1 and 180 bytes".into(),
            ));
        }
        ensure_current(&self.inner, generation)?;
        let _permit = acquire_slot(Arc::clone(&self.inner), generation)?;
        ensure_current(&self.inner, generation)?;

        let started = Instant::now();
        let tool = importer::ytdlp_command()?;
        let mut child = tool
            .command()
            .args([
                "--ignore-config",
                "--flat-playlist",
                "--dump-json",
                "--playlist-end",
                "5",
                "--",
            ])
            .arg(format!("ytsearch5:{query}"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AppError::BackgroundTask(format!("could not start fast yt-dlp search: {error}"))
            })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp search output is unavailable".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp search diagnostics are unavailable".into())
        })?;
        let child_id = self.inner.next_child_id.fetch_add(1, Ordering::Relaxed);
        let child = Arc::new(Mutex::new(child));
        self.inner
            .children
            .lock()
            .map_err(|_| AppError::BackgroundTask("YouTube search state is unavailable".into()))?
            .insert(child_id, Arc::clone(&child));
        if ensure_current(&self.inner, generation).is_err() {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }

        let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let status = wait_for_child(&child, SEARCH_TIMEOUT);
        clear_child(&self.inner, child_id);
        let stdout = stdout_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search output reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search diagnostic reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        ensure_current(&self.inner, generation)?;
        let status = status?;
        if stdout.exceeded {
            return Err(AppError::BackgroundTask(
                "yt-dlp search output exceeded 512 KiB".into(),
            ));
        }
        if !status.success() {
            let diagnostic = String::from_utf8_lossy(&stderr.bytes).trim().to_owned();
            return Err(AppError::BackgroundTask(if diagnostic.is_empty() {
                "yt-dlp search failed without diagnostics".into()
            } else {
                diagnostic
            }));
        }
        let candidates = parse_candidates(&stdout.bytes, query);
        info!(
            elapsed_ms = started.elapsed().as_millis(),
            result_count = candidates.len(),
            "YouTube search completed"
        );
        Ok(candidates)
    }
}

fn acquire_slot(inner: Arc<SearchInner>, generation: u64) -> Result<SearchPermit, AppError> {
    let mut active = inner
        .active_count
        .lock()
        .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    while *active >= MAX_CONCURRENT_SEARCHES {
        ensure_current(&inner, generation)?;
        active = inner
            .slot_available
            .wait(active)
            .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    }
    ensure_current(&inner, generation)?;
    *active += 1;
    drop(active);
    Ok(SearchPermit { inner })
}

fn ensure_current(inner: &SearchInner, generation: u64) -> Result<(), AppError> {
    (inner.generation.load(Ordering::Acquire) == generation)
        .then_some(())
        .ok_or_else(|| AppError::BackgroundTask("YouTube search was cancelled or replaced".into()))
}

fn wait_for_child(
    child: &Arc<Mutex<Child>>,
    timeout: Duration,
) -> Result<std::process::ExitStatus, AppError> {
    let started = Instant::now();
    loop {
        let status = child
            .lock()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search process is unavailable".into()))?
            .try_wait()
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        if let Some(status) = status {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            return Err(AppError::BackgroundTask(
                "YouTube search timed out after 12 seconds".into(),
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn clear_child(inner: &SearchInner, child_id: u64) {
    if let Ok(mut children) = inner.children.lock() {
        children.remove(&child_id);
    }
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(32 * 1024));
    let mut exceeded = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    Ok(BoundedOutput { bytes, exceeded })
}

fn parse_candidates(stdout: &[u8], query: &str) -> Vec<ImportCandidate> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let id = value.get("id")?.as_str()?;
            let title = value
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or(query);
            let channel = value
                .get("channel")
                .or_else(|| value.get("uploader"))
                .and_then(|value| value.as_str())
                .unwrap_or("YouTube");
            Some(ImportCandidate {
                input: format!("https://www.youtube.com/watch?v={id}"),
                title: title.into(),
                detail: channel.into(),
                kind: CandidateKind::Video,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_bounded_valid_video_results() {
        let output = br#"{"id":"abc","title":"Song","channel":"Artist"}
{"title":"Missing id"}
not json
"#;
        let candidates = parse_candidates(output, "fallback");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input, "https://www.youtube.com/watch?v=abc");
        assert_eq!(candidates[0].title, "Song");
        assert_eq!(candidates[0].detail, "Artist");
    }

    #[test]
    fn a_new_generation_invalidates_previous_searches() {
        let service = YoutubeSearchService::default();
        let previous = service.begin();
        let current = service.begin();
        assert!(ensure_current(&service.inner, previous).is_err());
        assert!(ensure_current(&service.inner, current).is_ok());
    }
}
