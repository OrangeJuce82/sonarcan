//! Fast, bounded yt-dlp metadata search supervision.

use std::{
    collections::{HashMap, HashSet},
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    error::AppError,
    importer::{self, CandidateKind, ImportCandidate},
    python_runtime,
};

const MAX_CONCURRENT_SEARCHES: usize = 2;
const MAX_QUERY_BYTES: usize = 180;
const MAX_STDOUT_BYTES: usize = 512 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(12);
const PLAYLIST_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_START_TIMEOUT: Duration = Duration::from_secs(3);
const SEARCH_RESULT_COUNT: usize = 10;
const PUBLISHED_RESULT_COUNT: usize = 5;
const SEARCH_PRINT_TEMPLATE: &str =
    "%(.{id,title,track,channel,uploader,artist,creator,album_artist,view_count,channel_is_verified,webpage_url,url})j";
const PLAYLIST_PRINT_TEMPLATE: &str =
    "%(.{id,title,track,channel,uploader,artist,creator,album_artist,webpage_url,url,format_id})j";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchProvider {
    Youtube,
    Soundcloud,
}

impl SearchProvider {
    fn prefix(self) -> &'static str {
        match self {
            Self::Youtube => "ytsearch",
            Self::Soundcloud => "scsearch",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Youtube => "YouTube",
            Self::Soundcloud => "SoundCloud",
        }
    }
}

#[derive(Clone, Default)]
pub struct YoutubeSearchService {
    inner: Arc<SearchInner>,
}

struct SearchInner {
    generation: AtomicU64,
    next_child_id: AtomicU64,
    children: Mutex<HashMap<u64, Arc<Mutex<Child>>>>,
    active_slots: Mutex<[bool; MAX_CONCURRENT_SEARCHES]>,
    workers: [Mutex<Option<ResidentWorker>>; MAX_CONCURRENT_SEARCHES],
    slot_available: Condvar,
}

struct SearchPermit {
    inner: Arc<SearchInner>,
    slot: usize,
}

impl Drop for SearchPermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.inner.active_slots.lock() {
            active[self.slot] = false;
            self.inner.slot_available.notify_one();
        }
    }
}

impl Default for SearchInner {
    fn default() -> Self {
        Self {
            generation: AtomicU64::default(),
            next_child_id: AtomicU64::default(),
            children: Mutex::default(),
            active_slots: Mutex::new([false; MAX_CONCURRENT_SEARCHES]),
            workers: std::array::from_fn(|_| Mutex::new(None)),
            slot_available: Condvar::default(),
        }
    }
}

struct ResidentWorker {
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    stdout: Receiver<io::Result<Vec<u8>>>,
}

impl ResidentWorker {
    fn is_running(&self) -> bool {
        self.child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok())
            .is_some_and(|status| status.is_none())
    }

    fn search(
        &mut self,
        query: &str,
        provider: SearchProvider,
    ) -> Result<WorkerResponse, AppError> {
        let mut request = serde_json::to_vec(&WorkerRequest {
            query,
            provider: provider.prefix(),
        })
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        request.push(b'\n');
        self.stdin
            .write_all(&request)
            .and_then(|_| self.stdin.flush())
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        let response = receive_worker_line(&self.stdout, SEARCH_TIMEOUT)?;
        serde_json::from_slice(&response).map_err(|error| {
            AppError::BackgroundTask(format!("invalid yt-dlp worker response: {error}"))
        })
    }

    fn stop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ResidentWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    query: &'a str,
    provider: &'a str,
}

#[derive(Deserialize)]
struct WorkerResponse {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    entries: Vec<serde_json::Value>,
    error: Option<String>,
}

struct ResidentWorkerCommand {
    python: PathBuf,
    archive: PathBuf,
    script: PathBuf,
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

    pub fn resolve(
        &self,
        query: &str,
        generation: u64,
        provider: SearchProvider,
    ) -> Result<Vec<ImportCandidate>, AppError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(AppError::BackgroundTask(
                "Provider search must contain between 1 and 180 bytes".into(),
            ));
        }
        ensure_current(&self.inner, generation)?;
        let permit = acquire_slot(Arc::clone(&self.inner), generation)?;
        ensure_current(&self.inner, generation)?;

        let started = Instant::now();
        let stdout = if resident_worker_command().is_some() {
            self.resolve_with_worker(query, generation, provider, permit.slot)?
        } else {
            self.resolve_with_cli(query, generation, provider)?
        };
        let candidates = parse_provider_candidates(&stdout, query, provider);
        info!(
            elapsed_ms = started.elapsed().as_millis(),
            result_count = candidates.len(),
            provider = provider.label(),
            "provider search completed"
        );
        Ok(candidates)
    }

    pub fn resolve_playlist(
        &self,
        url: &str,
        generation: u64,
    ) -> Result<Vec<ImportCandidate>, AppError> {
        let url = url.trim();
        if url.is_empty()
            || url.len() > 4_096
            || !(url.starts_with("https://") || url.starts_with("http://"))
        {
            return Err(AppError::BackgroundTask(
                "Import URL must be a bounded HTTP or HTTPS URL".into(),
            ));
        }
        ensure_current(&self.inner, generation)?;
        let _permit = acquire_slot(Arc::clone(&self.inner), generation)?;
        ensure_current(&self.inner, generation)?;

        let started = Instant::now();
        let stdout = self.resolve_playlist_with_cli(url, generation)?;
        let candidates = parse_playlist_candidates(&stdout, url);
        if candidates.is_empty() {
            return Err(AppError::BackgroundTask(
                "The URL does not contain any supported public track".into(),
            ));
        }
        info!(
            elapsed_ms = started.elapsed().as_millis(),
            result_count = candidates.len(),
            provider = importer::remote_provider_name(url),
            "provider URL expanded"
        );
        Ok(candidates)
    }

    fn resolve_with_worker(
        &self,
        query: &str,
        generation: u64,
        provider: SearchProvider,
        slot: usize,
    ) -> Result<Vec<u8>, AppError> {
        let mut worker = self.inner.workers[slot]
            .lock()
            .map_err(|_| AppError::BackgroundTask("yt-dlp worker is unavailable".into()))?;
        if worker.as_ref().map_or(true, |worker| !worker.is_running()) {
            match start_resident_worker() {
                Ok(started) => *worker = Some(started),
                Err(_) => {
                    warn!("resident yt-dlp worker unavailable; using CLI fallback");
                    drop(worker);
                    return self.resolve_with_cli(query, generation, provider);
                }
            }
        }
        let child = Arc::clone(
            &worker
                .as_ref()
                .ok_or_else(|| AppError::BackgroundTask("yt-dlp worker is unavailable".into()))?
                .child,
        );
        let child_id = register_child(&self.inner, child)?;
        if ensure_current(&self.inner, generation).is_err() {
            if let Some(worker) = worker.as_mut() {
                worker.stop();
            }
        }
        let response = worker
            .as_mut()
            .ok_or_else(|| AppError::BackgroundTask("yt-dlp worker is unavailable".into()))?
            .search(query, provider);
        clear_child(&self.inner, child_id);
        ensure_current(&self.inner, generation)?;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                *worker = None;
                return Err(error);
            }
        };
        if !response.ok {
            return Err(AppError::BackgroundTask(response.error.unwrap_or_else(
                || "yt-dlp search failed without diagnostics".into(),
            )));
        }
        compact_entries_as_lines(response.entries)
    }

    fn resolve_with_cli(
        &self,
        query: &str,
        generation: u64,
        provider: SearchProvider,
    ) -> Result<Vec<u8>, AppError> {
        let tool = importer::ytdlp_command()?;
        let mut command = tool.command();
        command.args([
            "--ignore-config",
            "--flat-playlist",
            "--lazy-playlist",
            "--print",
            SEARCH_PRINT_TEMPLATE,
            "--playlist-end",
            "10",
        ]);
        if matches!(provider, SearchProvider::Youtube) {
            command.args(["--extractor-args", "youtubetab:skip=webpage"]);
        }
        let mut child = command
            .arg("--")
            .arg(format!(
                "{}{SEARCH_RESULT_COUNT}:{query}",
                provider.prefix()
            ))
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
        let child = Arc::new(Mutex::new(child));
        let child_id = register_child(&self.inner, Arc::clone(&child))?;
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
        Ok(stdout.bytes)
    }

    fn resolve_playlist_with_cli(&self, url: &str, generation: u64) -> Result<Vec<u8>, AppError> {
        let tool = importer::ytdlp_command()?;
        let mut child = tool
            .command()
            .args(playlist_cli_arguments(url))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AppError::BackgroundTask(format!("could not start yt-dlp URL analysis: {error}"))
            })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp URL analysis output is unavailable".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp URL analysis diagnostics are unavailable".into())
        })?;
        let child = Arc::new(Mutex::new(child));
        let child_id = register_child(&self.inner, Arc::clone(&child))?;
        if ensure_current(&self.inner, generation).is_err() {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }

        let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let status = wait_for_child(&child, PLAYLIST_TIMEOUT);
        clear_child(&self.inner, child_id);
        let stdout = stdout_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp URL output reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp URL diagnostic reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        ensure_current(&self.inner, generation)?;
        let status = status?;
        if stdout.exceeded {
            return Err(AppError::BackgroundTask(
                "yt-dlp URL metadata exceeded 512 KiB".into(),
            ));
        }
        let diagnostic = String::from_utf8_lossy(&stderr.bytes).trim().to_owned();
        if !status.success() {
            return Err(AppError::BackgroundTask(
                metadata_analysis_error(&diagnostic).unwrap_or_else(|| {
                    if diagnostic.is_empty() {
                        "yt-dlp URL analysis failed without diagnostics".into()
                    } else {
                        diagnostic.clone()
                    }
                }),
            ));
        }
        if stdout.bytes.iter().all(u8::is_ascii_whitespace) {
            if let Some(error) = metadata_analysis_error(&diagnostic) {
                return Err(AppError::BackgroundTask(error));
            }
        }
        Ok(stdout.bytes)
    }
}

fn metadata_analysis_error(diagnostic: &str) -> Option<String> {
    importer::classify_known_ytdlp_error(diagnostic)
        .map(|(message, suggestion)| format!("{message} {suggestion}"))
}

fn playlist_cli_arguments(url: &str) -> Vec<&str> {
    let mut arguments = vec!["--ignore-config"];
    if importer::remote_provider_name(url) != "YouTube" {
        arguments.extend([
            "--lazy-playlist",
            "--no-warnings",
            "--skip-download",
            "--ignore-errors",
            "--ignore-no-formats-error",
        ]);
    } else {
        arguments.extend(["--flat-playlist", "--lazy-playlist", "--no-warnings"]);
    }
    arguments.extend(["--print", PLAYLIST_PRINT_TEMPLATE, "--", url]);
    arguments
}

fn acquire_slot(inner: Arc<SearchInner>, generation: u64) -> Result<SearchPermit, AppError> {
    let mut active = inner
        .active_slots
        .lock()
        .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    loop {
        ensure_current(&inner, generation)?;
        if let Some(slot) = active.iter().position(|in_use| !in_use) {
            active[slot] = true;
            drop(active);
            return Ok(SearchPermit { inner, slot });
        }
        active = inner
            .slot_available
            .wait(active)
            .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    }
}

fn ensure_current(inner: &SearchInner, generation: u64) -> Result<(), AppError> {
    (inner.generation.load(Ordering::Acquire) == generation)
        .then_some(())
        .ok_or_else(|| AppError::BackgroundTask("YouTube search was cancelled or replaced".into()))
}

fn register_child(inner: &SearchInner, child: Arc<Mutex<Child>>) -> Result<u64, AppError> {
    let child_id = inner.next_child_id.fetch_add(1, Ordering::Relaxed);
    inner
        .children
        .lock()
        .map_err(|_| AppError::BackgroundTask("YouTube search state is unavailable".into()))?
        .insert(child_id, child);
    Ok(child_id)
}

fn resident_worker_command() -> Option<ResidentWorkerCommand> {
    let configured = || {
        Some(ResidentWorkerCommand {
            python: python_runtime::bundled_python_313()?,
            archive: python_runtime::resource_path("ytdlp-search/yt-dlp")?,
            script: python_runtime::resource_path("ytdlp-search/search_worker.py")?,
        })
    };
    if let Some(command) = configured().filter(ResidentWorkerCommand::is_available) {
        return Some(command);
    }

    #[cfg(debug_assertions)]
    {
        let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let command = ResidentWorkerCommand {
            python: resources.join("python-runtime/runtime/bin/python3.13"),
            archive: resources.join("ytdlp-search/yt-dlp"),
            script: resources.join("ytdlp-search/search_worker.py"),
        };
        if command.is_available() {
            return Some(command);
        }
    }
    None
}

impl ResidentWorkerCommand {
    fn is_available(&self) -> bool {
        self.python.is_file() && self.archive.is_file() && self.script.is_file()
    }
}

fn start_resident_worker() -> Result<ResidentWorker, AppError> {
    let command = resident_worker_command()
        .ok_or_else(|| AppError::BackgroundTask("resident yt-dlp worker is unavailable".into()))?;
    let mut process = Command::new(command.python);
    importer::suppress_console_window(&mut process);
    let mut child = process
        .arg(command.script)
        .env("PYTHONPATH", command.archive)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            AppError::BackgroundTask(format!("could not start resident yt-dlp worker: {error}"))
        })?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::BackgroundTask("yt-dlp worker input is unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::BackgroundTask("yt-dlp worker output is unavailable".into()))?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::BackgroundTask("yt-dlp worker diagnostics are unavailable".into())
    })?;
    let child = Arc::new(Mutex::new(child));
    let stdout = spawn_worker_stdout_reader(stdout);
    thread::spawn(move || {
        let _ = read_bounded(stderr, MAX_STDERR_BYTES);
    });
    let mut worker = ResidentWorker {
        child,
        stdin,
        stdout,
    };
    let ready = receive_worker_line(&worker.stdout, WORKER_START_TIMEOUT).and_then(|line| {
        serde_json::from_slice::<serde_json::Value>(&line).map_err(|error| {
            AppError::BackgroundTask(format!("invalid yt-dlp worker startup: {error}"))
        })
    });
    if !ready
        .as_ref()
        .ok()
        .and_then(|value| value.get("ready"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        worker.stop();
        return Err(ready.err().unwrap_or_else(|| {
            AppError::BackgroundTask("yt-dlp worker did not become ready".into())
        }));
    }
    Ok(worker)
}

fn spawn_worker_stdout_reader(stdout: ChildStdout) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            let result = reader
                .by_ref()
                .take(MAX_STDOUT_BYTES as u64 + 1)
                .read_until(b'\n', &mut line);
            match result {
                Ok(0) => {
                    let _ = sender.send(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "yt-dlp worker stopped unexpectedly",
                    )));
                    break;
                }
                Ok(_) if line.len() > MAX_STDOUT_BYTES => {
                    let _ = sender.send(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "yt-dlp worker output exceeded 512 KiB",
                    )));
                    break;
                }
                Ok(_) => {
                    if sender.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

fn receive_worker_line(
    receiver: &Receiver<io::Result<Vec<u8>>>,
    timeout: Duration,
) -> Result<Vec<u8>, AppError> {
    match receiver.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(error)) => Err(AppError::BackgroundTask(error.to_string())),
        Err(RecvTimeoutError::Timeout) => Err(AppError::BackgroundTask(format!(
            "yt-dlp worker timed out after {} seconds",
            timeout.as_secs()
        ))),
        Err(RecvTimeoutError::Disconnected) => Err(AppError::BackgroundTask(
            "yt-dlp worker output is unavailable".into(),
        )),
    }
}

fn compact_entries_as_lines(entries: Vec<serde_json::Value>) -> Result<Vec<u8>, AppError> {
    let mut output = Vec::new();
    for entry in entries.into_iter().take(SEARCH_RESULT_COUNT) {
        serde_json::to_writer(&mut output, &entry)
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        output.push(b'\n');
        if output.len() > MAX_STDOUT_BYTES {
            return Err(AppError::BackgroundTask(
                "yt-dlp worker output exceeded 512 KiB".into(),
            ));
        }
    }
    Ok(output)
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
            return Err(AppError::BackgroundTask(format!(
                "yt-dlp metadata request timed out after {} seconds",
                timeout.as_secs()
            )));
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

#[cfg(test)]
fn parse_candidates(stdout: &[u8], query: &str) -> Vec<ImportCandidate> {
    parse_provider_candidates(stdout, query, SearchProvider::Youtube)
}

fn parse_provider_candidates(
    stdout: &[u8],
    query: &str,
    provider: SearchProvider,
) -> Vec<ImportCandidate> {
    let mut candidates = String::from_utf8_lossy(stdout)
        .lines()
        .enumerate()
        .filter_map(|(search_index, line)| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let id = value.get("id")?.as_str()?;
            if id.is_empty()
                || id.len() > 64
                || !id.bytes().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_')
                })
            {
                return None;
            }
            let title = value
                .get("title")
                .or_else(|| value.get("track"))
                .and_then(|value| value.as_str())
                .filter(|title| !title.trim().is_empty())?;
            let title = clean_youtube_title(&bounded_text(title, 256), id);
            let channel = value
                .get("channel")
                .or_else(|| value.get("uploader"))
                .or_else(|| value.get("artist"))
                .or_else(|| value.get("creator"))
                .or_else(|| value.get("album_artist"))
                .and_then(|value| value.as_str())
                .filter(|channel| !channel.trim().is_empty())?;
            let channel = bounded_text(channel, 160);
            let views = value.get("view_count").and_then(|value| value.as_u64());
            let verified = value
                .get("channel_is_verified")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let score = relevance_score(query, &title, &channel, views, verified, search_index);
            let source_url = match provider {
                SearchProvider::Youtube => format!("https://www.youtube.com/watch?v={id}"),
                SearchProvider::Soundcloud => value
                    .get("webpage_url")
                    .or_else(|| value.get("url"))
                    .and_then(|value| value.as_str())
                    .filter(|url| importer::remote_provider_name(url) == "SoundCloud")?
                    .to_owned(),
            };
            Some((
                score,
                ImportCandidate {
                    input: source_url.clone(),
                    title,
                    detail: channel,
                    kind: CandidateKind::Video,
                    match_score: Some(score),
                    thumbnail_url: matches!(provider, SearchProvider::Youtube)
                        .then(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
                    video_id: matches!(provider, SearchProvider::Youtube).then(|| id.to_owned()),
                    provider: Some(provider.label().to_ascii_lowercase()),
                    source_url: Some(source_url),
                    blocked: false,
                },
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen_titles = HashSet::new();
    candidates
        .into_iter()
        .filter(|(_, candidate)| {
            let normalized_title = tokens(&candidate.title).join("\u{1f}");
            normalized_title.is_empty() || seen_titles.insert(normalized_title)
        })
        .take(PUBLISHED_RESULT_COUNT)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn parse_playlist_candidates(stdout: &[u8], playlist_url: &str) -> Vec<ImportCandidate> {
    let provider = importer::remote_provider_name(playlist_url);
    let mut seen = HashSet::new();
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let id = value.get("id").and_then(serde_json::Value::as_str);
            let video_id = if provider == "YouTube" {
                let id = id.filter(|id| {
                    !id.is_empty()
                        && id.len() <= 64
                        && id.bytes().all(|character| {
                            character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_')
                        })
                })?;
                Some(id.to_owned())
            } else {
                None
            };
            let source_url = if let Some(video_id) = &video_id {
                format!("https://www.youtube.com/watch?v={video_id}")
            } else {
                value
                    .get("webpage_url")
                    .or_else(|| value.get("url"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|url| importer::remote_provider_name(url) == provider)?
                    .to_owned()
            };
            if !seen.insert(source_url.clone()) {
                return None;
            }
            let title = value
                .get("title")
                .or_else(|| value.get("track"))
                .and_then(serde_json::Value::as_str)
                .filter(|title| !title.trim().is_empty())
                .map(|title| bounded_text(title, 256))
                .or_else(|| {
                    (provider == "YouTube")
                        .then(|| id.map(|id| bounded_text(id, 256)))
                        .flatten()
                })
                .unwrap_or_else(|| format!("{provider} track"));
            let detail = bounded_text(
                value
                    .get("channel")
                    .or_else(|| value.get("uploader"))
                    .or_else(|| value.get("artist"))
                    .or_else(|| value.get("creator"))
                    .or_else(|| value.get("album_artist"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(provider),
                160,
            );
            let blocked = provider != "YouTube"
                && value
                    .get("format_id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|format_id| !format_id.is_empty())
                    .is_none();
            Some(ImportCandidate {
                input: source_url.clone(),
                title,
                detail,
                kind: CandidateKind::Video,
                match_score: None,
                thumbnail_url: video_id
                    .as_ref()
                    .map(|id| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
                video_id,
                provider: Some(provider.to_ascii_lowercase()),
                source_url: Some(source_url),
                blocked,
            })
        })
        .collect()
}

fn clean_youtube_title(title: &str, video_id: &str) -> String {
    let suffix = format!(" [{video_id}]");
    title
        .strip_suffix(&suffix)
        .unwrap_or(title)
        .trim()
        .to_owned()
}

fn bounded_text(value: &str, maximum_characters: usize) -> String {
    value.chars().take(maximum_characters).collect()
}

fn relevance_score(
    query: &str,
    title: &str,
    channel: &str,
    views: Option<u64>,
    verified: bool,
    search_index: usize,
) -> f64 {
    let query_tokens = tokens(query);
    let title_tokens = tokens(title);
    let channel_tokens = tokens(channel);
    let mut combined_tokens = title_tokens.clone();
    combined_tokens.extend(channel_tokens.iter().cloned());

    let combined_match = token_recall(&query_tokens, &combined_tokens);
    let title_match = token_recall(&query_tokens, &title_tokens);
    let title_precision = token_recall(&title_tokens, &query_tokens);
    let channel_match = token_recall(&query_tokens, &channel_tokens);
    let structured = split_artist_and_title(query);
    let normalized_channel = channel.to_lowercase();
    let official_name_hint = ["official", "vevo", "topic"]
        .iter()
        .any(|marker| normalized_channel.contains(marker));
    let popularity = views
        .map(|count| ((count as f64 + 1.0).log10() / 9.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let position_hint = (1.0 - search_index as f64 / SEARCH_RESULT_COUNT as f64).max(0.0);

    // yt-dlp maps both YouTube's verification check and Official Artist Channel
    // music-note badge to `channel_is_verified` in flat search results.
    let metadata_score = 0.03 * f64::from(verified)
        + 0.005 * f64::from(official_name_hint)
        + 0.01 * popularity
        + 0.005 * position_hint;
    let mut score = if let Some((artist, expected_title)) = structured {
        let artist_tokens = tokens(artist);
        let artist_match = token_recall(&artist_tokens, &title_tokens)
            .max(token_recall(&artist_tokens, &channel_tokens));
        0.42 * token_recall(&tokens(expected_title), &title_tokens)
            + 0.24 * artist_match
            + 0.17 * title_match
            + 0.07 * combined_match
            + 0.05 * title_precision
            + metadata_score
    } else {
        0.58 * title_match
            + 0.24 * title_precision
            + 0.10 * combined_match
            + 0.03 * channel_match
            + metadata_score
    };

    const VERSION_MARKERS: &[&str] = &[
        "cover",
        "karaoke",
        "tutorial",
        "reaction",
        "remix",
        "nightcore",
        "sped",
        "slowed",
        "live",
        "instrumental",
    ];
    let mismatch_count = VERSION_MARKERS
        .iter()
        .filter(|marker| {
            !query_tokens.iter().any(|token| token == **marker)
                && title_tokens.iter().any(|token| token == **marker)
        })
        .count();
    score -= (mismatch_count as f64 * 0.12).min(0.36);
    score.clamp(0.01, 0.99)
}

fn split_artist_and_title(query: &str) -> Option<(&str, &str)> {
    [" - ", " – ", " — "]
        .iter()
        .find_map(|separator| query.split_once(separator))
        .filter(|(artist, title)| !artist.trim().is_empty() && !title.trim().is_empty())
}

fn tokens(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_characters = 0;
    for character in value.to_lowercase().chars() {
        if character.is_alphanumeric() {
            if current_characters < 64 {
                current.push(character);
                current_characters += 1;
            }
        } else if !matches!(character, '\'' | '’' | '‘' | 'ʼ') {
            push_token(&mut result, &mut current);
            current_characters = 0;
            if result.len() == 32 {
                return result;
            }
        }
    }
    push_token(&mut result, &mut current);
    result
}

fn push_token(result: &mut Vec<String>, current: &mut String) {
    if !current.is_empty()
        && !matches!(
            current.as_str(),
            "official" | "audio" | "video" | "hd" | "4k"
        )
        && result.len() < 32
    {
        result.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn token_recall(expected: &[String], actual: &[String]) -> f64 {
    if expected.is_empty() {
        return 0.0;
    }
    expected
        .iter()
        .map(|expected_token| {
            actual
                .iter()
                .map(|actual_token| token_similarity(expected_token, actual_token))
                .fold(0.0, f64::max)
        })
        .sum::<f64>()
        / expected.len() as f64
}

fn token_similarity(left: &str, right: &str) -> f64 {
    if left == right {
        return 1.0;
    }
    let longest = left.chars().count().max(right.chars().count());
    if longest < 4 {
        return 0.0;
    }
    let distance = levenshtein(left, right);
    let similarity = 1.0 - distance as f64 / longest as f64;
    if similarity >= 0.72 {
        similarity
    } else {
        0.0
    }
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_character) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_character) in right.iter().enumerate() {
            current.push(
                (current[right_index] + 1)
                    .min(previous[right_index + 1] + 1)
                    .min(previous[right_index] + usize::from(left_character != *right_character)),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_worker_entries_preserve_only_the_bounded_search_batch() {
        let entries = (0..11)
            .map(|index| {
                serde_json::json!({
                    "id": format!("video{index}"),
                    "title": format!("Song {index}"),
                    "channel": "Artist",
                    "view_count": 42,
                    "channel_is_verified": true,
                })
            })
            .collect();
        let output = compact_entries_as_lines(entries).unwrap();
        let lines = String::from_utf8(output.clone()).unwrap();

        assert_eq!(lines.lines().count(), SEARCH_RESULT_COUNT);
        let candidates = parse_candidates(&output, "Artist Song");
        assert_eq!(candidates.len(), PUBLISHED_RESULT_COUNT);
        assert_eq!(candidates[0].detail, "Artist");
    }

    #[test]
    fn parses_only_bounded_valid_video_results() {
        let output = br#"{"id":"abc","title":"Song","channel":"Artist"}
{"title":"Missing id"}
{"id":"missing-title","channel":"Artist"}
{"id":"missing-artist","title":"Song"}
not json
"#;
        let candidates = parse_candidates(output, "fallback");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input, "https://www.youtube.com/watch?v=abc");
        assert_eq!(candidates[0].title, "Song");
        assert_eq!(candidates[0].detail, "Artist");
        assert_eq!(
            candidates[0].thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/vi/abc/mqdefault.jpg")
        );
        assert_eq!(candidates[0].video_id.as_deref(), Some("abc"));
        assert!(candidates[0].match_score.is_some());
    }

    #[test]
    fn soundcloud_results_keep_their_provider_url_and_identity() {
        let output = br#"{"id":"12345","title":"Song","uploader":"Artist","webpage_url":"https://soundcloud.com/artist/song"}"#;
        let candidates =
            parse_provider_candidates(output, "Artist Song", SearchProvider::Soundcloud);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input, "https://soundcloud.com/artist/song");
        assert_eq!(candidates[0].provider.as_deref(), Some("soundcloud"));
        assert_eq!(
            candidates[0].source_url.as_deref(),
            Some("https://soundcloud.com/artist/song")
        );
        assert!(candidates[0].video_id.is_none());
        assert!(candidates[0].thumbnail_url.is_none());
    }

    #[test]
    fn playlist_entries_are_exposed_as_individual_import_candidates() {
        let output = br#"{"id":"one","title":"First","uploader":"Artist","webpage_url":"https://soundcloud.com/artist/first"}
{"id":"two","title":"Second","uploader":"Artist","webpage_url":"https://soundcloud.com/artist/second"}
"#;

        let candidates =
            parse_playlist_candidates(output, "https://soundcloud.com/artist/sets/collection");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].title, "First");
        assert_eq!(candidates[0].input, "https://soundcloud.com/artist/first");
        assert_eq!(candidates[1].input, "https://soundcloud.com/artist/second");
        assert!(candidates.iter().all(|candidate| {
            candidate.kind == CandidateKind::Video
                && candidate.provider.as_deref() == Some("soundcloud")
        }));
    }

    #[test]
    fn non_youtube_metadata_retains_titles_without_authentication_or_audio_downloads() {
        for url in [
            "https://soundcloud.com/artist/sets/collection",
            "https://audiomack.com/artist/album/collection",
            "https://www.beatport.com/track/title/123",
            "https://hearthis.at/artist/title/",
            "https://www.jamendo.com/track/1",
            "https://www.reverbnation.com/artist/song/title",
        ] {
            let arguments = playlist_cli_arguments(url);

            assert!(!arguments.contains(&"--flat-playlist"));
            assert!(arguments.contains(&"--skip-download"));
            assert!(arguments.contains(&"--ignore-no-formats-error"));
            assert!(!arguments.contains(&"--username"));
            assert!(!arguments.contains(&"--password"));
            assert!(!arguments.contains(&"--cookies"));
        }
    }

    #[test]
    fn metadata_analysis_exposes_drm_before_import() {
        assert_eq!(
            metadata_analysis_error(
                "ERROR: [soundcloud] 224371784: This video is DRM protected"
            )
            .as_deref(),
            Some(
                "This content is protected by DRM and cannot be imported. Choose another public, DRM-free source."
            )
        );
    }

    #[test]
    fn metadata_without_a_public_format_keeps_its_title_but_is_blocked() {
        let output = br#"{"id":"163684297","title":"Say My Name (feat. Zyra)","artist":"ODESZA featuring Zyra","webpage_url":"https://soundcloud.com/odesza/say_my_name"}"#;
        let candidates =
            parse_playlist_candidates(output, "https://soundcloud.com/odesza/say_my_name");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Say My Name (feat. Zyra)");
        assert_eq!(candidates[0].detail, "ODESZA featuring Zyra");
        assert!(candidates[0].blocked);
    }

    #[test]
    fn metadata_with_a_public_format_remains_importable() {
        let output = br#"{"id":"90301462","title":"Disfigure - Blank [NCS Release]","artist":"Disfigure","webpage_url":"https://soundcloud.com/nocopyrightsounds/disfigure-blank","format_id":"hls_aac_160k"}"#;
        let candidates = parse_playlist_candidates(
            output,
            "https://soundcloud.com/nocopyrightsounds/disfigure-blank",
        );

        assert_eq!(candidates.len(), 1);
        assert!(!candidates[0].blocked);
    }

    #[test]
    fn metadata_analysis_exposes_unavailable_videos_before_import() {
        assert_eq!(
            metadata_analysis_error(
                "ERROR: [youtube] 3URh7kJ6dtQ: This video is not available"
            )
            .as_deref(),
            Some(
                "This video is private, removed, or unavailable. Check the source URL and choose another public item."
            )
        );
    }

    #[test]
    fn soundcloud_entries_never_present_the_platform_id_as_the_title() {
        let output = br#"{"id":"1781794326","webpage_url":"https://soundcloud.com/artist/song"}"#;
        let candidates =
            parse_playlist_candidates(output, "https://soundcloud.com/artist/sets/collection");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "SoundCloud track");
    }

    #[test]
    fn cli_metadata_templates_request_one_compact_json_object_per_line() {
        assert!(SEARCH_PRINT_TEMPLATE.ends_with("})j"));
        assert!(PLAYLIST_PRINT_TEMPLATE.ends_with("})j"));
        assert!(!SEARCH_PRINT_TEMPLATE.ends_with("})#j"));
        assert!(!PLAYLIST_PRINT_TEMPLATE.ends_with("})#j"));
    }

    #[test]
    fn youtube_playlist_entries_receive_stable_watch_urls() {
        let output = br#"{"id":"AbC_123-x","title":"Song","channel":"Artist"}"#;
        let candidates =
            parse_playlist_candidates(output, "https://www.youtube.com/playlist?list=PLexample");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].input,
            "https://www.youtube.com/watch?v=AbC_123-x"
        );
        assert_eq!(candidates[0].video_id.as_deref(), Some("AbC_123-x"));
        assert!(
            playlist_cli_arguments("https://www.youtube.com/playlist?list=PLexample")
                .contains(&"--flat-playlist")
        );
    }

    #[test]
    fn removes_only_a_trailing_bracketed_copy_of_the_video_id() {
        assert_eq!(
            clean_youtube_title("Bold as Love [9qIunneAx6Y]", "9qIunneAx6Y"),
            "Bold as Love"
        );
        assert_eq!(
            clean_youtube_title("Bold as Love [Live]", "9qIunneAx6Y"),
            "Bold as Love [Live]"
        );
        assert_eq!(
            clean_youtube_title("[9qIunneAx6Y] Bold as Love", "9qIunneAx6Y"),
            "[9qIunneAx6Y] Bold as Love"
        );
    }

    #[test]
    fn ranks_an_artist_channel_above_an_unrequested_cover() {
        let output = br#"{"id":"cover","title":"Enjoy the Silence cover","channel":"Random Guitar","view_count":9000000}
{"id":"official","title":"Enjoy the Silence","channel":"Depeche Mode","channel_is_verified":true,"view_count":1000}
"#;
        let candidates = parse_candidates(output, "Depeche Mode - Enjoy the Silence");

        assert_eq!(
            candidates[0].input,
            "https://www.youtube.com/watch?v=official"
        );
        assert!(candidates[0].match_score > candidates[1].match_score);
    }

    #[test]
    fn exact_title_matches_ignore_case_and_punctuation() {
        let score = relevance_score("The Pot", "tHE, pOT!", "Unrelated channel", None, false, 9);

        assert!(score > 0.9);
    }

    #[test]
    fn apostrophes_have_no_effect_on_an_exact_match() {
        let score = relevance_score(
            "Guns N' Roses - Don't Cry",
            "GUNS N ROSES — DONT CRY",
            "Guns N' Roses",
            None,
            false,
            9,
        );

        assert!(score > 0.9);
    }

    #[test]
    fn artist_and_title_order_does_not_distort_relevance() {
        let canonical = relevance_score(
            "Tool - The Pot",
            "TOOL - The Pot (Official Audio)",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );
        let reversed = relevance_score(
            "Tool - The Pot",
            "The Pot - TOOL",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );

        assert!(canonical > 0.95);
        assert!(reversed > 0.95);
        assert!((canonical - reversed).abs() < f64::EPSILON);
    }

    #[test]
    fn a_verified_artist_badge_outweighs_an_official_word_in_the_channel_name() {
        let verified = relevance_score(
            "Tool - The Pot",
            "Tool - The Pot",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );
        let claimed = relevance_score(
            "Tool - The Pot",
            "Tool - The Pot",
            "Tool Official Uploads",
            Some(1_000_000),
            false,
            0,
        );

        assert!(verified > claimed);
    }

    #[test]
    fn collapses_titles_that_differ_only_by_case_punctuation_or_presentation_markers() {
        let output =
            br#"{"id":"mirror","title":"tool  the pot (Official Audio)","channel":"Mirror"}
{"id":"official","title":"Tool - The Pot","channel":"Tool","channel_is_verified":true}
{"id":"reversed","title":"The Pot - TOOL","channel":"Tool"}
"#;
        let candidates = parse_candidates(output, "Tool - The Pot");

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].input,
            "https://www.youtube.com/watch?v=official"
        );
        assert_eq!(
            candidates[1].input,
            "https://www.youtube.com/watch?v=reversed"
        );
    }

    #[test]
    fn publishes_only_the_five_highest_scoring_results() {
        let output = (0..10)
            .map(|index| format!(r#"{{"id":"{index}","title":"Song {index}","channel":"Artist"}}"#))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(parse_candidates(output.as_bytes(), "Artist Song").len(), 5);
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
