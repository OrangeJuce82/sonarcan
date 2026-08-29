use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    error::AppError,
    preferences::{ConversionFormat, PreferencesStore, UserPreferences},
    project,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub id: Uuid,
    pub label: String,
    pub state: JobState,
    pub progress: f32,
    pub error: Option<String>,
    pub suggestion: Option<String>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Downloading,
    Converting,
    Importing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub package_path: PathBuf,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub input: String,
    pub title: String,
    pub detail: String,
    pub kind: CandidateKind,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateKind {
    Local,
    Video,
    Playlist,
    Search,
}

#[derive(Default)]
pub struct ImportService {
    inner: Arc<ImportInner>,
}
#[derive(Default)]
struct ImportInner {
    jobs: Mutex<Vec<ImportJob>>,
    work: Mutex<HashMap<Uuid, ImportWork>>,
    manager_running: Mutex<bool>,
    project_write: Mutex<()>,
}

#[derive(Clone)]
struct ImportWork {
    package_path: PathBuf,
    preferences: UserPreferences,
    cancelled: Arc<AtomicBool>,
}

impl ImportService {
    pub fn jobs(&self) -> Vec<ImportJob> {
        self.inner
            .jobs
            .lock()
            .map(|jobs| jobs.clone())
            .unwrap_or_default()
    }

    pub fn enqueue(
        &self,
        request: ImportRequest,
        preferences: UserPreferences,
    ) -> Result<Vec<ImportJob>, AppError> {
        if !request.package_path.is_dir() {
            return Err(AppError::InvalidProjectPackage(request.package_path));
        }
        let inputs: Vec<_> = request
            .inputs
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect();
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let added: Vec<_> = inputs
            .into_iter()
            .map(|input| ImportJob {
                id: Uuid::new_v4(),
                label: input,
                state: JobState::Queued,
                progress: 0.0,
                error: None,
                suggestion: None,
                diagnostic: None,
            })
            .collect();
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::BackgroundTask("import queue is unavailable".into()))?
            .extend(added.clone());
        {
            let mut work =
                self.inner.work.lock().map_err(|_| {
                    AppError::BackgroundTask("import work queue is unavailable".into())
                })?;
            for job in &added {
                work.insert(
                    job.id,
                    ImportWork {
                        package_path: request.package_path.clone(),
                        preferences: preferences.clone(),
                        cancelled: Arc::new(AtomicBool::new(false)),
                    },
                );
            }
        }
        let mut running = self
            .inner
            .manager_running
            .lock()
            .map_err(|_| AppError::BackgroundTask("import manager is unavailable".into()))?;
        if !*running {
            *running = true;
            let inner = Arc::clone(&self.inner);
            thread::Builder::new()
                .name("sonarcan-import-manager".into())
                .spawn(move || run_manager(inner))
                .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        }
        Ok(added)
    }

    pub fn cancel(&self, id: Uuid) -> Result<(), AppError> {
        let exists = self
            .inner
            .jobs
            .lock()
            .map_err(|_| AppError::BackgroundTask("import queue is unavailable".into()))?
            .iter()
            .any(|job| job.id == id);
        if !exists {
            return Ok(());
        }
        if let Some(work) = self
            .inner
            .work
            .lock()
            .map_err(|_| AppError::BackgroundTask("import work queue is unavailable".into()))?
            .get(&id)
        {
            work.cancelled.store(true, Ordering::Relaxed);
        }
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::BackgroundTask("import queue is unavailable".into()))?
            .retain(|job| job.id != id);
        self.inner
            .work
            .lock()
            .map_err(|_| AppError::BackgroundTask("import work queue is unavailable".into()))?
            .remove(&id);
        Ok(())
    }

    pub fn remove(&self, id: Uuid) -> Result<(), AppError> {
        self.inner
            .jobs
            .lock()
            .map_err(|_| AppError::BackgroundTask("import queue is unavailable".into()))?
            .retain(|job| job.id != id);
        self.inner
            .work
            .lock()
            .map_err(|_| AppError::BackgroundTask("import work queue is unavailable".into()))?
            .remove(&id);
        Ok(())
    }
}

fn run_manager(inner: Arc<ImportInner>) {
    loop {
        let concurrency = inner
            .work
            .lock()
            .ok()
            .and_then(|work| {
                work.values()
                    .next()
                    .map(|item| item.preferences.concurrent_downloads)
            })
            .unwrap_or(1);
        let ids: Vec<Uuid> = inner
            .jobs
            .lock()
            .map(|jobs| {
                jobs.iter()
                    .filter(|job| job.state == JobState::Queued)
                    .take(concurrency)
                    .map(|job| job.id)
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            let Ok(mut running) = inner.manager_running.lock() else {
                break;
            };
            let has_queued = inner
                .jobs
                .lock()
                .map(|jobs| jobs.iter().any(|job| job.state == JobState::Queued))
                .unwrap_or(false);
            if has_queued {
                continue;
            }
            *running = false;
            break;
        }
        let handles: Vec<_> = ids
            .into_iter()
            .map(|id| {
                let inner = Arc::clone(&inner);
                thread::spawn(move || run_job(&inner, id))
            })
            .collect();
        for handle in handles {
            let _ = handle.join();
        }
    }
}

fn run_job(inner: &Arc<ImportInner>, id: Uuid) {
    let input = inner
        .jobs
        .lock()
        .ok()
        .and_then(|jobs| {
            jobs.iter()
                .find(|job| job.id == id)
                .map(|job| job.label.clone())
        })
        .unwrap_or_default();
    let Some(work) = inner
        .work
        .lock()
        .ok()
        .and_then(|work| work.get(&id).cloned())
    else {
        fail(inner, id, "import work item is unavailable", None);
        return;
    };
    if is_cancelled(&work.cancelled) {
        return;
    }
    let result = if let Some(path) = local_input_path(&input) {
        let needs_conversion = needs_local_conversion(&path, &work.preferences);
        if needs_conversion {
            update(inner, id, JobState::Converting, 0.15, None);
        } else {
            update(inner, id, JobState::Importing, 0.7, None);
        }
        import_local(
            &work.cancelled,
            &inner.project_write,
            &work.package_path,
            &path,
            &work.preferences,
        )
    } else {
        download_remote(
            inner,
            id,
            &work.cancelled,
            &inner.project_write,
            &work.package_path,
            &input,
            &work.preferences,
        )
    };
    match result {
        Ok(()) => update(inner, id, JobState::Completed, 1.0, None),
        Err(error) => {
            warn!(job_id = %id, %error, "import job failed");
            fail(inner, id, &error.to_string(), None);
        }
    }
    if let Ok(mut work) = inner.work.lock() {
        work.remove(&id);
    }
}

fn import_local(
    cancelled: &AtomicBool,
    project_write: &Mutex<()>,
    package: &Path,
    source: &Path,
    preferences: &UserPreferences,
) -> Result<(), AppError> {
    if is_cancelled(cancelled) {
        return Ok(());
    }
    let Some(converted) = convert_local(cancelled, source, preferences)? else {
        return Ok(());
    };
    if is_cancelled(cancelled) {
        cleanup_temporary_file(&converted, source);
        return Ok(());
    }
    let imported = import_project_audio(
        cancelled,
        project_write,
        package,
        std::slice::from_ref(&converted),
    );
    cleanup_temporary_file(&converted, source);
    imported.map(|_| ())
}

fn convert_local(
    cancelled: &AtomicBool,
    source: &Path,
    preferences: &UserPreferences,
) -> Result<Option<PathBuf>, AppError> {
    if !needs_local_conversion(source, preferences) {
        return Ok(Some(source.to_path_buf()));
    }
    let extension = match preferences.conversion_format {
        ConversionFormat::Wav => "wav",
        ConversionFormat::Flac => "flac",
        _ => "mp3",
    };
    let output =
        std::env::temp_dir().join(format!("sonarcan-import-{}.{}", Uuid::new_v4(), extension));
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        AppError::BackgroundTask(
            "FFmpeg is required for this local audio conversion. Install FFmpeg or choose Keep supported formats.".into(),
        )
    })?;
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source);
    apply_conversion_args(&mut command, preferences);
    command.arg(&output);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::BackgroundTask(format!("could not start FFmpeg: {error}")))?;
    loop {
        if is_cancelled(cancelled) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&output);
            return Ok(None);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?
        {
            if !status.success() {
                let _ = fs::remove_file(&output);
                return Err(AppError::BackgroundTask("FFmpeg conversion failed".into()));
            }
            return Ok(Some(output));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn needs_local_conversion(source: &Path, preferences: &UserPreferences) -> bool {
    if matches!(preferences.conversion_format, ConversionFormat::Keep) {
        return project::AudioFormat::from_path(source).is_err();
    }
    let extension = match preferences.conversion_format {
        ConversionFormat::Wav => "wav",
        ConversionFormat::Flac => "flac",
        _ => "mp3",
    };
    let format_differs = !source
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension));
    let metadata = crate::audio::probe(source).ok();
    let audio_shape_differs = matches!(
        preferences.sample_rate,
        crate::preferences::SampleRatePreference::Hz44100
    ) && metadata.and_then(|value| value.sample_rate) != Some(44_100)
        || matches!(
            preferences.channels,
            crate::preferences::ChannelPreference::Stereo
        ) && metadata.and_then(|value| value.channels) != Some(2)
        || matches!(
            preferences.channels,
            crate::preferences::ChannelPreference::Mono
        ) && metadata.and_then(|value| value.channels) != Some(1)
        || matches!(
            preferences.sample_rate,
            crate::preferences::SampleRatePreference::Hz48000
        ) && metadata.and_then(|value| value.sample_rate) != Some(48_000);
    format_differs || audio_shape_differs
}

fn find_ffmpeg() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("ffmpeg")];
    #[cfg(target_os = "macos")]
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
    ]);
    #[cfg(target_os = "windows")]
    candidates.push(PathBuf::from("ffmpeg.exe"));
    candidates.into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

fn download_remote(
    inner: &Arc<ImportInner>,
    id: Uuid,
    cancelled: &AtomicBool,
    project_write: &Mutex<()>,
    package: &Path,
    input: &str,
    preferences: &UserPreferences,
) -> Result<(), AppError> {
    let tool = ensure_ytdlp()?;
    let staging = package.join("Cache").join("Downloads").join(id.to_string());
    fs::create_dir_all(&staging).map_err(|error| AppError::io(&staging, error))?;
    update(inner, id, JobState::Downloading, 0.01, None);
    let target = youtube_download_target(input);
    let mut command = Command::new(tool);
    command
        .args([
            "--ignore-config",
            "--newline",
            "--no-warnings",
            "--playlist-end",
            "10",
            "--progress-template",
            "download:%(progress._percent_str)s",
            "--print",
            "after_move:sonarcan-file:%(filepath)s",
            "-P",
        ])
        .arg(&staging);
    command.args([
        "-o",
        "%(playlist_index&{} - |)s%(title).180B [%(id)s].%(ext)s",
        "-x",
        "--audio-format",
    ]);
    command.arg(match preferences.conversion_format {
        ConversionFormat::Wav => "wav",
        ConversionFormat::Flac => "flac",
        _ => "mp3",
    });
    if matches!(
        preferences.conversion_format,
        ConversionFormat::Mp3 | ConversionFormat::Keep
    ) {
        command.args(["--audio-quality", mp3_quality(preferences)]);
    }
    command
        .arg("--")
        .arg(target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    info!(job_id = %id, input = %input, "starting yt-dlp import");
    let mut child = command
        .spawn()
        .map_err(|error| AppError::BackgroundTask(format!("could not start yt-dlp: {error}")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::BackgroundTask("yt-dlp diagnostics are unavailable".into()))?;
    let stderr_reader = thread::spawn(move || {
        let mut text = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if text.len() < 32_000 {
                text.push_str(&line);
                text.push('\n');
            }
        }
        text
    });
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::BackgroundTask("yt-dlp output is unavailable".into()))?;
    let (line_sender, line_receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line_sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut files = Vec::new();
    loop {
        if is_cancelled(cancelled) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            let _ = fs::remove_dir_all(&staging);
            return Ok(());
        }
        let line = match line_receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if let Some(value) = line.strip_prefix("download:") {
            if let Ok(percent) = value.trim().trim_end_matches('%').parse::<f32>() {
                update(
                    inner,
                    id,
                    JobState::Downloading,
                    percent / 100.0 * 0.78,
                    None,
                );
            }
        } else if let Some(path) = line.strip_prefix("sonarcan-file:") {
            files.push(PathBuf::from(path));
        }
    }
    let _ = stdout_reader.join();
    let exit = child
        .wait()
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
    let diagnostic = stderr_reader.join().unwrap_or_default();
    if is_cancelled(cancelled) {
        let _ = fs::remove_dir_all(&staging);
        return Ok(());
    }
    if !exit.success() {
        tracing::warn!(job_id = %id, diagnostic = %diagnostic, "yt-dlp process failed");
        let (message, suggestion) = classify_ytdlp_error(&diagnostic);
        fail(inner, id, &message, Some((&suggestion, &diagnostic)));
        return Err(AppError::BackgroundTask(message));
    }
    update(inner, id, JobState::Importing, 0.9, None);
    if files.is_empty() {
        files = fs::read_dir(&staging)
            .map_err(|error| AppError::io(&staging, error))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| project::AudioFormat::from_path(path).is_ok())
            .collect();
    }
    if is_cancelled(cancelled) {
        let _ = fs::remove_dir_all(&staging);
        return Ok(());
    }
    let imported = import_project_audio(cancelled, project_write, package, &files);
    let _ = fs::remove_dir_all(staging);
    imported.map(|_| ())
}

fn youtube_download_target(input: &str) -> String {
    if is_url(input) {
        input.to_owned()
    } else {
        format!("ytsearch1:{input}")
    }
}

fn import_project_audio(
    cancelled: &AtomicBool,
    project_write: &Mutex<()>,
    package: &Path,
    source_paths: &[PathBuf],
) -> Result<(), AppError> {
    let guard = project_write
        .lock()
        .map_err(|_| AppError::BackgroundTask("project import is unavailable".into()))?;
    if is_cancelled(cancelled) {
        drop(guard);
        return Ok(());
    }
    project::import_audio(package, source_paths).map(|_| ())
}

fn is_cancelled(cancelled: &AtomicBool) -> bool {
    cancelled.load(Ordering::Relaxed)
}

fn cleanup_temporary_file(converted: &Path, source: &Path) {
    if converted != source && converted.starts_with(std::env::temp_dir()) {
        let _ = fs::remove_file(converted);
    }
}

fn apply_conversion_args(command: &mut Command, preferences: &UserPreferences) {
    match preferences.channels {
        crate::preferences::ChannelPreference::Stereo => {
            command.args(["-ac", "2"]);
        }
        crate::preferences::ChannelPreference::Mono => {
            command.args(["-ac", "1"]);
        }
        _ => {}
    }
    match preferences.sample_rate {
        crate::preferences::SampleRatePreference::Hz44100 => {
            command.args(["-ar", "44100"]);
        }
        crate::preferences::SampleRatePreference::Hz48000 => {
            command.args(["-ar", "48000"]);
        }
        _ => {}
    }
    if matches!(preferences.conversion_format, ConversionFormat::Mp3) {
        command.args(["-q:a", mp3_quality(preferences)]);
    }
}
fn mp3_quality(preferences: &UserPreferences) -> &'static str {
    match preferences.mp3_quality {
        crate::preferences::Mp3Quality::VbrHigh => "0",
        crate::preferences::Mp3Quality::Kbps320 => "320K",
        crate::preferences::Mp3Quality::Kbps256 => "256K",
        crate::preferences::Mp3Quality::Kbps192 => "192K",
    }
}
fn local_input_path(input: &str) -> Option<PathBuf> {
    let mut value = input.trim();
    while let Some(stripped) = value.strip_prefix("file://") {
        value = stripped;
    }
    // Some pasted or exported sources escape the @ in account names as `\@`. A file
    // URI uses the literal @, while a real backslash must be percent-encoded.
    let normalized = value.replace("\\@", "@");
    let value = normalized.as_str();
    let decoded = if let Some(stripped) = value.strip_prefix("localhost/") {
        percent_decode(&format!("/{stripped}"))?
    } else {
        percent_decode(value)?
    };
    let path = PathBuf::from(decoded);
    path.is_file().then_some(path)
}

fn percent_decode(value: &str) -> Option<String> {
    fn hex_digit(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
fn is_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
fn update(
    inner: &Arc<ImportInner>,
    id: Uuid,
    state: JobState,
    progress: f32,
    error: Option<String>,
) {
    if let Ok(mut jobs) = inner.jobs.lock() {
        if let Some(job) = jobs.iter_mut().find(|job| job.id == id) {
            job.state = state;
            job.progress = progress.clamp(0.0, 1.0);
            job.error = error;
        }
    }
}
fn fail(inner: &Arc<ImportInner>, id: Uuid, message: &str, details: Option<(&str, &str)>) {
    if let Ok(mut jobs) = inner.jobs.lock() {
        if let Some(job) = jobs.iter_mut().find(|job| job.id == id) {
            job.state = JobState::Failed;
            job.progress = 0.0;
            job.error = Some(message.into());
            if let Some((suggestion, diagnostic)) = details {
                job.suggestion = Some(suggestion.into());
                job.diagnostic = Some(diagnostic.into());
            }
        }
    }
}
fn classify_ytdlp_error(log: &str) -> (String, String) {
    let lower = log.to_ascii_lowercase();
    if lower.contains("ffmpeg") && (lower.contains("not found") || lower.contains("not installed"))
    {
        return (
            "FFmpeg is required to convert this audio.".into(),
            "Install FFmpeg or select a format that does not require conversion, then retry."
                .into(),
        );
    }
    if lower.contains("sign in") || lower.contains("cookies") || lower.contains("authentication") {
        return ("YouTube requires authentication for this content.".into(), "Open the video in your browser and verify that your account is allowed to access it. Browser-cookie support can be added later.".into());
    }
    if lower.contains("private video")
        || lower.contains("video unavailable")
        || lower.contains("has been removed")
    {
        return (
            "This video is private, removed, or unavailable.".into(),
            "Check the URL and availability in YouTube, then choose another source.".into(),
        );
    }
    if lower.contains("not available in your country") || lower.contains("geo") {
        return (
            "This content is not available in the current region.".into(),
            "Choose another authorized source available in your region.".into(),
        );
    }
    if lower.contains("javascript runtime") || lower.contains("ejs") || lower.contains("deno") {
        return (
            "YouTube extraction needs an updated JavaScript runtime.".into(),
            "Update yt-dlp from SonArcan's tool manager and retry.".into(),
        );
    }
    if lower.contains("http error 403") || lower.contains("forbidden") {
        return (
            "YouTube refused the download request.".into(),
            "Retry later, update yt-dlp, or verify the video in your browser.".into(),
        );
    }
    if lower.contains("timed out") || lower.contains("network") || lower.contains("connection") {
        return (
            "The download was interrupted by a network error.".into(),
            "Check the connection and retry; completed project imports are preserved.".into(),
        );
    }
    ("yt-dlp could not import this item.".into(), "Open the technical details for the exact yt-dlp output, then retry after updating the tool.".into())
}

pub fn parse_text(text: &str) -> Vec<ImportCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        for token in line.split_whitespace() {
            let clean = token.trim_matches(|character: char| {
                matches!(
                    character,
                    ',' | ';' | ':' | '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\''
                )
            });
            if !clean.starts_with("http://") && !clean.starts_with("https://") {
                continue;
            }
            let playlist = clean.contains("list=") || clean.contains("/playlist");
            let candidate = ImportCandidate {
                input: clean.into(),
                title: clean.into(),
                detail: if playlist {
                    "YouTube playlist"
                } else {
                    "YouTube URL"
                }
                .into(),
                kind: if playlist {
                    CandidateKind::Playlist
                } else {
                    CandidateKind::Video
                },
            };
            if seen.insert(analysis_candidate_key(&candidate)) {
                candidates.push(candidate);
            }
        }
        if !line.contains("http://") && !line.contains("https://") {
            let path = local_input_path(line);
            if path.is_some() || line.len() <= 180 {
                let candidate = ImportCandidate {
                    input: line.into(),
                    title: path
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or(line)
                        .into(),
                    detail: if path.is_some() {
                        "Local file"
                    } else {
                        "YouTube search"
                    }
                    .into(),
                    kind: if path.is_some() {
                        CandidateKind::Local
                    } else {
                        CandidateKind::Search
                    },
                };
                if seen.insert(analysis_candidate_key(&candidate)) {
                    candidates.push(candidate);
                }
            }
        }
    }
    candidates
}

fn analysis_candidate_key(candidate: &ImportCandidate) -> String {
    match candidate.kind {
        CandidateKind::Local => local_input_path(&candidate.input)
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| format!("file:{}", name.to_string_lossy().to_ascii_lowercase()))
            .unwrap_or_else(|| format!("file:{}", candidate.input.to_ascii_lowercase())),
        CandidateKind::Video | CandidateKind::Playlist => normalized_url_key(&candidate.input),
        CandidateKind::Search => format!(
            "search:{}",
            candidate
                .input
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        ),
    }
}

fn normalized_url_key(input: &str) -> String {
    let without_fragment = input.split('#').next().unwrap_or(input);
    let Some((scheme, remainder)) = without_fragment.split_once("://") else {
        return format!("url:{}", without_fragment.trim_end_matches('/'));
    };
    let authority_end = remainder.find(['/', '?']).unwrap_or(remainder.len());
    let authority = remainder[..authority_end].to_ascii_lowercase();
    let path_and_query = &remainder[authority_end..];
    let query = path_and_query
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();
    let parameter = |name: &str| {
        query
            .split('&')
            .find_map(|pair| pair.split_once('=').filter(|(key, _)| *key == name))
            .map(|(_, value)| value)
    };
    if authority == "youtu.be" {
        if let Some(id) = path_and_query
            .trim_start_matches('/')
            .split(['?', '/'])
            .next()
            .filter(|value| !value.is_empty())
        {
            return format!("youtube-video:{id}");
        }
    }
    if authority == "youtube.com" || authority == "www.youtube.com" || authority == "m.youtube.com"
    {
        if let Some(id) = parameter("list") {
            return format!("youtube-playlist:{id}");
        }
        if let Some(id) = parameter("v") {
            return format!("youtube-video:{id}");
        }
    }
    format!(
        "url:{}://{}{}",
        scheme.to_ascii_lowercase(),
        authority,
        path_and_query.trim_end_matches('/')
    )
}

pub fn resolve_search(query: &str) -> Result<Vec<ImportCandidate>, AppError> {
    let tool = ensure_ytdlp()?;
    let output = Command::new(tool)
        .args([
            "--ignore-config",
            "--flat-playlist",
            "--dump-json",
            "--playlist-end",
            "5",
            "--",
        ])
        .arg(format!("ytsearch5:{query}"))
        .output()
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
    if !output.status.success() {
        return Err(AppError::BackgroundTask(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
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
        .collect())
}

fn ensure_ytdlp() -> Result<PathBuf, AppError> {
    if Command::new("yt-dlp")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(PathBuf::from("yt-dlp"));
    }
    let directory = ProjectDirs::from("music", "SonArcan", "SonArcan")
        .ok_or_else(|| AppError::BackgroundTask("tool directory is unavailable".into()))?
        .data_local_dir()
        .join("tools");
    fs::create_dir_all(&directory).map_err(|error| AppError::io(&directory, error))?;
    let (filename, url) = platform_release()?;
    let path = directory.join(filename);
    if path.is_file() {
        return Ok(path);
    }
    let bytes = reqwest::blocking::get(url)
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::bytes)
        .map_err(|error| AppError::BackgroundTask(format!("could not download yt-dlp: {error}")))?;
    verify_release(filename, &bytes)?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, &bytes).map_err(|error| AppError::io(&temporary, error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))
            .map_err(|error| AppError::io(&temporary, error))?;
    }
    fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
    Ok(path)
}
fn platform_release() -> Result<(&'static str, &'static str), AppError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Ok((
            "yt-dlp_macos",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos",
        )),
        ("windows", "x86_64") => Ok((
            "yt-dlp.exe",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
        )),
        ("windows", "aarch64") => Ok((
            "yt-dlp_arm64.exe",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_arm64.exe",
        )),
        ("linux", "aarch64") => Ok((
            "yt-dlp_linux_aarch64",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64",
        )),
        ("linux", "x86_64") => Ok((
            "yt-dlp_linux",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux",
        )),
        _ => Err(AppError::BackgroundTask(
            "no yt-dlp binary is available for this platform".into(),
        )),
    }
}
fn verify_release(filename: &str, bytes: &[u8]) -> Result<(), AppError> {
    let sums = reqwest::blocking::get(
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS",
    )
    .and_then(reqwest::blocking::Response::error_for_status)
    .and_then(reqwest::blocking::Response::text)
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
    let expected = sums
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let hash = fields.next()?;
            let name = fields.next()?.trim_start_matches('*');
            (name == filename).then_some(hash)
        })
        .ok_or_else(|| AppError::BackgroundTask("yt-dlp checksum is unavailable".into()))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(AppError::BackgroundTask(
            "yt-dlp checksum verification failed".into(),
        ));
    }
    Ok(())
}

pub fn preferences_from_store(store: &PreferencesStore) -> UserPreferences {
    store.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn unresolved_youtube_searches_use_one_result() {
        assert_eq!(
            youtube_download_target("artist song"),
            "ytsearch1:artist song"
        );
        assert_eq!(
            youtube_download_target("https://youtu.be/example"),
            "https://youtu.be/example"
        );
    }

    #[test]
    fn text_analysis_extracts_every_embedded_url_without_a_batch_limit() {
        let text = (0..25)
            .map(|index| format!("Song {index}: (https://example.com/watch?v={index}),"))
            .collect::<Vec<_>>()
            .join("\n");
        let candidates = parse_text(&text);
        assert_eq!(candidates.len(), 25);
        assert_eq!(candidates[0].input, "https://example.com/watch?v=0");
        assert_eq!(candidates[24].input, "https://example.com/watch?v=24");
    }

    #[test]
    fn text_analysis_deduplicates_urls_but_keeps_multiple_distinct_urls() {
        let candidates = parse_text(
            "First https://example.com/a and https://example.com/b\nAgain https://example.com/a",
        );
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn text_analysis_deduplicates_youtube_url_variants() {
        let candidates = parse_text(
            "https://youtu.be/AbC123\nhttps://www.youtube.com/watch?v=AbC123&feature=share",
        );

        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn text_analysis_deduplicates_local_files_by_filename() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_path = first.path().join("Same Track.mp3");
        let second_path = second.path().join("same track.MP3");
        fs::write(&first_path, b"first").unwrap();
        fs::write(&second_path, b"second").unwrap();

        let candidates = parse_text(&format!(
            "file://{}\nfile://{}",
            first_path.display(),
            second_path.display()
        ));

        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn text_analysis_detects_file_uris_and_decodes_their_paths() {
        let directory = tempfile::Builder::new()
            .prefix("sonarcan file uri ")
            .tempdir()
            .unwrap();
        let account_directory = directory.path().join("GoogleDrive-test@gmail.com");
        fs::create_dir_all(&account_directory).unwrap();
        let path = account_directory.join("track name.mp3");
        fs::write(&path, b"audio").unwrap();
        let encoded_path = path.to_string_lossy().replace(' ', "%20");

        for input in [
            format!("file://{encoded_path}"),
            format!("file://file://{encoded_path}"),
            format!("file://localhost{encoded_path}"),
            format!("file://{}", encoded_path.replace('@', "\\@")),
        ] {
            let candidates = parse_text(&input);
            assert_eq!(candidates.len(), 1, "input: {input}");
            assert!(matches!(candidates[0].kind, CandidateKind::Local));
        }
    }

    #[test]
    fn text_analysis_keeps_local_files_with_long_paths() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("a".repeat(190));
        fs::create_dir_all(&nested).unwrap();
        let path = nested.join("track.mp3");
        fs::write(&path, b"audio").unwrap();

        let candidates = parse_text(&format!("file://{}", path.display()));
        assert_eq!(candidates.len(), 1);
        assert!(matches!(candidates[0].kind, CandidateKind::Local));
    }

    #[test]
    fn cancelling_a_job_removes_it_and_signals_its_worker() {
        let service = ImportService::default();
        let id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        service.inner.jobs.lock().unwrap().push(ImportJob {
            id,
            label: "long import".into(),
            state: JobState::Downloading,
            progress: 0.4,
            error: None,
            suggestion: None,
            diagnostic: None,
        });
        service.inner.work.lock().unwrap().insert(
            id,
            ImportWork {
                package_path: PathBuf::from("project.sac"),
                preferences: UserPreferences::default(),
                cancelled: Arc::clone(&cancelled),
            },
        );

        service.cancel(id).unwrap();

        assert!(cancelled.load(Ordering::Relaxed));
        assert!(service.jobs().is_empty());
        assert!(service.inner.work.lock().unwrap().is_empty());
    }
}
