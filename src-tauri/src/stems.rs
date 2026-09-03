//! Supervision of the pinned `demucs-mlx` six-stem worker.
//! Inference and file I/O never run on the CPAL callback.

use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app_log,
    audio_engine::{decode_stem_file, AudioEngine, DecodedAudio},
    error::AppError,
    ffmpeg,
    preferences::{Mp3Quality, UserPreferences},
    project,
    stem_contract::{MODEL_NAME, MODEL_REVISION, STEM_COUNT, STEM_NAMES},
};

const CACHE_VERSION: u32 = 2;
const STEM_MAGIC: &[u8; 8] = b"SACSTM02";
const MAX_PROTOCOL_LINE: usize = 16 * 1024;
const MAX_STEM_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const PCM_HEADER_BYTES: u64 = 8 + 4 + 8;
const STEM_EXPORT_ORDER: [usize; STEM_COUNT] = [0, 1, 2, 4, 5, 3];

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StemExportFormat {
    Wav,
    Mp3,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemStatus {
    pub state: StemState,
    pub enabled: bool,
    pub progress: f32,
    pub stage: String,
    pub track_id: Option<Uuid>,
    pub cached: bool,
    pub error: Option<String>,
    pub compute_backend: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StemState {
    Disabled,
    Ready,
    Separating,
    Failed,
}

impl Default for StemStatus {
    fn default() -> Self {
        Self {
            state: StemState::Disabled,
            enabled: false,
            progress: 0.0,
            stage: "disabled".into(),
            track_id: None,
            cached: false,
            error: None,
            compute_backend: None,
        }
    }
}

#[derive(Default)]
pub struct StemService {
    status: Arc<Mutex<StemStatus>>,
    running: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    child: Arc<Mutex<Option<Arc<Mutex<Child>>>>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StemManifest {
    cache_version: u32,
    model_revision: String,
    track_id: Uuid,
    source_size: u64,
    source_modified_ns: u64,
    sample_rate: u32,
    frames: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WorkerEvent {
    Stage {
        stage: String,
        progress: f32,
    },
    Progress {
        stage: String,
        progress: f32,
    },
    Log {
        level: String,
        message: String,
    },
    Complete {
        stems: Vec<String>,
    },
    Error {
        message: String,
    },
    #[serde(other)]
    Unknown,
}

struct WorkerCommand {
    executable: PathBuf,
    prefix_arguments: Vec<String>,
    additional_arguments: Vec<String>,
    model_dir: PathBuf,
    backend: StemBackend,
}

#[derive(Debug, Clone, Copy)]
enum StemBackend {
    Mlx,
    Torch,
}

impl StemBackend {
    fn preferred() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Self::Mlx
        } else {
            Self::Torch
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mlx => "MLX",
            Self::Torch => "Torch",
        }
    }

    fn log_source(self) -> &'static str {
        match self {
            Self::Mlx => "mlx",
            Self::Torch => "torch",
        }
    }
}

impl StemService {
    pub fn status(&self) -> StemStatus {
        self.status
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| StemStatus {
                state: StemState::Failed,
                error: Some("stem service state is unavailable".into()),
                ..Default::default()
            })
    }

    pub fn disable(&self, engine: &AudioEngine) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut current) = self.child.lock() {
            if let Some(child) = current.take() {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
        }
        engine.disable_stems();
        set_status(&self.status, StemStatus::default());
        app_log::push_external("stems", "info", "stem separation disabled");
    }

    pub fn set_enabled(&self, engine: &AudioEngine, enabled: bool) -> bool {
        let ready = self
            .status
            .lock()
            .is_ok_and(|status| status.state == StemState::Ready);
        let enabled = ready && engine.set_stems_enabled(enabled);
        if let Ok(mut status) = self.status.lock() {
            status.enabled = enabled;
        }
        app_log::push_external(
            "stems",
            "info",
            if enabled {
                "stem mix enabled; original audio remains loaded"
            } else {
                "stem mix bypassed; separated buffers remain loaded"
            },
        );
        enabled
    }

    pub fn start(
        &self,
        app: AppHandle,
        package_path: PathBuf,
        track_id: Uuid,
    ) -> Result<(), AppError> {
        if self.running.swap(true, Ordering::AcqRel) {
            return Err(AppError::StemSeparation(
                "another stem separation is already running".into(),
            ));
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let backend = StemBackend::preferred();
        set_status(
            &self.status,
            active_status(track_id, 0.0, "checkingCache", backend.label()),
        );
        let status = Arc::clone(&self.status);
        let running = Arc::clone(&self.running);
        let child = Arc::clone(&self.child);
        let active_generation = Arc::clone(&self.generation);
        std::thread::Builder::new()
            .name("sonarcan-stem-worker".into())
            .spawn(move || {
                let result = separate_or_load(
                    &app,
                    &package_path,
                    track_id,
                    generation,
                    &active_generation,
                    &status,
                    &child,
                );
                if active_generation.load(Ordering::Acquire) == generation {
                    if let Err(error) = result {
                        warn!(%track_id, %error, "stem separation failed");
                        app_log::push_external(backend.log_source(), "error", &error.to_string());
                        set_status(
                            &status,
                            StemStatus {
                                state: StemState::Failed,
                                enabled: false,
                                progress: 0.0,
                                stage: "failed".into(),
                                track_id: Some(track_id),
                                cached: false,
                                error: Some(error.to_string()),
                                compute_backend: Some(backend.label().into()),
                            },
                        );
                    }
                }
                running.store(false, Ordering::Release);
            })
            .map_err(|error| {
                self.running.store(false, Ordering::Release);
                AppError::StemSeparation(error.to_string())
            })?;
        Ok(())
    }
}

pub fn export(
    package_path: &Path,
    track_id: Uuid,
    destination: &Path,
    format: StemExportFormat,
    display_names: &[String],
    preferences: &UserPreferences,
) -> Result<(), AppError> {
    if display_names.len() != STEM_COUNT
        || display_names.iter().any(|name| name.chars().count() > 40)
    {
        return Err(AppError::StemSeparation(
            "the stem export names are invalid".into(),
        ));
    }
    if !destination.is_absolute() || destination.file_name().is_none() || destination.exists() {
        return Err(AppError::StemSeparation(format!(
            "the stem export destination is invalid or already exists: {}",
            destination.display()
        )));
    }
    let parent = destination.parent().ok_or_else(|| {
        AppError::StemSeparation("the stem export parent directory is unavailable".into())
    })?;
    let parent = parent
        .canonicalize()
        .map_err(|error| AppError::io(parent, error))?;
    if !parent.is_dir() {
        return Err(AppError::StemSeparation(
            "the stem export parent is not a directory".into(),
        ));
    }
    let destination_name = destination.file_name().ok_or_else(|| {
        AppError::StemSeparation("the stem export directory name is unavailable".into())
    })?;
    let destination = parent.join(destination_name);
    if destination.exists() {
        return Err(AppError::StemSeparation(format!(
            "the stem export destination already exists: {}",
            destination.display()
        )));
    }

    let media_path = project::track_media_path(package_path, track_id)?;
    let source_metadata = media_path
        .metadata()
        .map_err(|error| AppError::io(&media_path, error))?;
    let source_directory = validated_export_cache_dir(package_path, track_id)?;
    let manifest = read_valid_manifest_from_dir(
        &source_directory,
        track_id,
        source_metadata.len(),
        modified_ns(&source_metadata),
    )
    .ok_or_else(|| {
        AppError::StemSeparation(
            "the six stems are not available or no longer match the selected track".into(),
        )
    })?;

    let temporary = parent.join(format!(".sonarcan-stems-{}.tmp", Uuid::new_v4()));
    fs::create_dir(&temporary).map_err(|error| AppError::io(&temporary, error))?;
    let result = export_into_directory(
        &source_directory,
        &temporary,
        format,
        display_names,
        preferences.mp3_quality,
        &manifest,
    )
    .and_then(|()| publish_export_directory(&temporary, &destination));
    if result.is_err() && temporary.exists() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result?;
    app_log::push_external(
        "rust",
        "info",
        &format!(
            "exported six stems as {}",
            match format {
                StemExportFormat::Wav => "WAV",
                StemExportFormat::Mp3 => "MP3",
            }
        ),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn export_into_directory(
    source_directory: &Path,
    destination: &Path,
    format: StemExportFormat,
    display_names: &[String],
    mp3_quality: Mp3Quality,
    manifest: &StemManifest,
) -> Result<(), AppError> {
    let ffmpeg = if matches!(format, StemExportFormat::Mp3) {
        Some(ffmpeg::find().ok_or_else(|| {
            AppError::StemSeparation(
                "FFmpeg is required to export MP3 stems. Install FFmpeg or choose WAV.".into(),
            )
        })?)
    } else {
        None
    };
    let mut used_names = Vec::with_capacity(STEM_COUNT);
    for (position, stem_index) in STEM_EXPORT_ORDER.iter().copied().enumerate() {
        let safe_name = unique_export_name(
            sanitize_export_name(&display_names[stem_index]),
            &mut used_names,
        );
        let base_name = format!("{:02} - {safe_name}", position + 1);
        let wave_path = destination.join(format!("{base_name}.wav"));
        copy_pcm_cache_to_wave(
            &source_directory.join(format!("{}.pcm", STEM_NAMES[stem_index])),
            &wave_path,
            manifest.sample_rate,
            manifest.frames,
        )?;
        if let (StemExportFormat::Mp3, Some(ffmpeg)) = (format, ffmpeg.as_ref()) {
            let mp3_path = destination.join(format!("{base_name}.mp3"));
            let mut command = Command::new(ffmpeg);
            command
                .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
                .arg(&wave_path)
                .args(["-map_metadata", "-1"]);
            ffmpeg::apply_mp3_quality(&mut command, mp3_quality);
            let status = command
                .arg(&mp3_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|error| {
                    AppError::StemSeparation(format!("could not start FFmpeg: {error}"))
                })?;
            if !status.success() {
                return Err(AppError::StemSeparation(format!(
                    "FFmpeg could not encode stem {} as MP3",
                    position + 1
                )));
            }
            fs::remove_file(&wave_path).map_err(|error| AppError::io(&wave_path, error))?;
        }
    }
    Ok(())
}

fn validated_export_cache_dir(package_path: &Path, track_id: Uuid) -> Result<PathBuf, AppError> {
    let package = package_path
        .canonicalize()
        .map_err(|error| AppError::io(package_path, error))?;
    let stem_root_path = package.join("Stems");
    let stem_root = stem_root_path
        .canonicalize()
        .map_err(|error| AppError::io(&stem_root_path, error))?;
    let cache_path = stem_root.join(track_id.to_string());
    let cache = cache_path
        .canonicalize()
        .map_err(|error| AppError::io(&cache_path, error))?;
    if !stem_root.starts_with(&package) || !cache.starts_with(&stem_root) || !cache.is_dir() {
        return Err(AppError::StemSeparation(
            "the stem cache is outside the selected project".into(),
        ));
    }
    Ok(cache)
}

fn publish_export_directory(temporary: &Path, destination: &Path) -> Result<(), AppError> {
    fs::create_dir(destination).map_err(|error| AppError::io(destination, error))?;
    let result = (|| {
        for entry in fs::read_dir(temporary).map_err(|error| AppError::io(temporary, error))? {
            let entry = entry.map_err(|error| AppError::io(temporary, error))?;
            let source = entry.path();
            let target = destination.join(entry.file_name());
            fs::rename(&source, &target).map_err(|error| AppError::io(&target, error))?;
        }
        fs::remove_dir(temporary).map_err(|error| AppError::io(temporary, error))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

fn sanitize_export_name(value: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .filter(|character| !character.is_control() && !matches!(character, '/' | '\\' | ':'))
        .take(60)
        .collect();
    let sanitized =
        sanitized.trim_matches(|character: char| character == '.' || character.is_whitespace());
    if sanitized.is_empty() {
        "Stem".into()
    } else {
        sanitized.into()
    }
}

fn unique_export_name(candidate: String, used: &mut Vec<String>) -> String {
    let mut result = candidate.clone();
    let mut suffix = 2;
    while used.iter().any(|value| value.eq_ignore_ascii_case(&result)) {
        result = format!("{candidate} {suffix}");
        suffix += 1;
    }
    used.push(result.clone());
    result
}

fn copy_pcm_cache_to_wave(
    source: &Path,
    destination: &Path,
    expected_sample_rate: u32,
    expected_frames: usize,
) -> Result<(), AppError> {
    let source_metadata =
        fs::symlink_metadata(source).map_err(|error| AppError::io(source, error))?;
    if !source_metadata.file_type().is_file()
        || source_metadata.len() > PCM_HEADER_BYTES + MAX_STEM_BYTES
    {
        return Err(AppError::StemSeparation(format!(
            "the cached stem is not a valid regular file: {}",
            source.display()
        )));
    }
    let mut source_file = File::open(source).map_err(|error| AppError::io(source, error))?;
    let mut magic = [0; 8];
    let mut rate_bytes = [0; 4];
    let mut frame_bytes = [0; 8];
    source_file
        .read_exact(&mut magic)
        .and_then(|_| source_file.read_exact(&mut rate_bytes))
        .and_then(|_| source_file.read_exact(&mut frame_bytes))
        .map_err(|error| AppError::io(source, error))?;
    let sample_rate = u32::from_le_bytes(rate_bytes);
    let frames = u64::from_le_bytes(frame_bytes);
    let data_bytes = frames
        .checked_mul(2)
        .and_then(|samples| samples.checked_mul(size_of::<f32>() as u64))
        .filter(|size| *size <= MAX_STEM_BYTES)
        .ok_or_else(|| AppError::StemSeparation("the stem PCM size is invalid".into()))?;
    let expected_file_size = PCM_HEADER_BYTES
        .checked_add(data_bytes)
        .ok_or_else(|| AppError::StemSeparation("the stem PCM size is invalid".into()))?;
    let actual_file_size = source_metadata.len();
    if magic != *STEM_MAGIC
        || sample_rate != expected_sample_rate
        || frames != expected_frames as u64
        || actual_file_size != expected_file_size
    {
        return Err(AppError::StemSeparation(format!(
            "the cached stem is invalid: {}",
            source.display()
        )));
    }
    let data_size = u32::try_from(data_bytes)
        .map_err(|_| AppError::StemSeparation("the stem is too large for WAV export".into()))?;
    let file = File::create(destination).map_err(|error| AppError::io(destination, error))?;
    let mut output = BufWriter::with_capacity(1024 * 1024, file);
    write_float_wave_header(&mut output, sample_rate, frames as u32, data_size)
        .map_err(|error| AppError::io(destination, error))?;
    let copied = io::copy(&mut source_file, &mut output)
        .map_err(|error| AppError::io(destination, error))?;
    if copied != data_bytes {
        return Err(AppError::StemSeparation(
            "the cached stem ended before the expected PCM frame count".into(),
        ));
    }
    output
        .flush()
        .map_err(|error| AppError::io(destination, error))
}

fn write_float_wave_header(
    output: &mut impl Write,
    sample_rate: u32,
    frames: u32,
    data_size: u32,
) -> io::Result<()> {
    let byte_rate = sample_rate.saturating_mul(2 * size_of::<f32>() as u32);
    output.write_all(b"RIFF")?;
    output.write_all(&(48_u32.saturating_add(data_size)).to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16_u32.to_le_bytes())?;
    output.write_all(&3_u16.to_le_bytes())?;
    output.write_all(&2_u16.to_le_bytes())?;
    output.write_all(&sample_rate.to_le_bytes())?;
    output.write_all(&byte_rate.to_le_bytes())?;
    output.write_all(&8_u16.to_le_bytes())?;
    output.write_all(&32_u16.to_le_bytes())?;
    output.write_all(b"fact")?;
    output.write_all(&4_u32.to_le_bytes())?;
    output.write_all(&frames.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&data_size.to_le_bytes())
}

#[allow(clippy::too_many_arguments)]
fn separate_or_load(
    app: &AppHandle,
    package_path: &Path,
    track_id: Uuid,
    generation: u64,
    active_generation: &AtomicU64,
    status: &Arc<Mutex<StemStatus>>,
    current_child: &Arc<Mutex<Option<Arc<Mutex<Child>>>>>,
) -> Result<(), AppError> {
    let backend = StemBackend::preferred();
    let media_path = project::track_media_path(package_path, track_id)?;
    let metadata = media_path
        .metadata()
        .map_err(|error| AppError::io(&media_path, error))?;
    if let Some(stems) = load_cache(
        package_path,
        track_id,
        metadata.len(),
        modified_ns(&metadata),
    ) {
        app.state::<AudioEngine>()
            .activate_stems(&media_path, stems)?;
        set_status(status, ready_status(track_id, true, backend.label()));
        app_log::push_external(
            backend.log_source(),
            "info",
            &format!("{MODEL_NAME}: loaded the verified six-stem cache"),
        );
        return Ok(());
    }
    let worker = resolve_worker(app)?;
    set_status(
        status,
        active_status(track_id, 0.0, "separatingStems", worker.backend.label()),
    );
    let separation_started = Instant::now();
    app_log::push_external(
        worker.backend.log_source(),
        "info",
        &format!(
            "{MODEL_NAME}: starting six-stem generation with {}",
            worker.backend.label()
        ),
    );
    let source = app
        .state::<AudioEngine>()
        .decoded_for_analysis(&media_path)?;
    let work_parent = package_path.join("Cache").join("stem-working");
    fs::create_dir_all(&work_parent).map_err(|error| AppError::io(&work_parent, error))?;
    let output_dir = work_parent.join(format!("{track_id}-{generation}"));
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).map_err(|error| AppError::io(&output_dir, error))?;
    }
    let result = run_worker(
        &worker,
        &media_path,
        &output_dir,
        track_id,
        generation,
        active_generation,
        status,
        current_child,
    )
    .and_then(|()| {
        if active_generation.load(Ordering::Acquire) != generation {
            return Err(AppError::StemSeparation("separation cancelled".into()));
        }
        set_status(
            status,
            active_status(track_id, 0.985, "validatingStems", worker.backend.label()),
        );
        let validation_started = Instant::now();
        let stems = load_worker_stems(&output_dir, &source)?;
        app_log::push_external(
            worker.backend.log_source(),
            "info",
            &format!(
                "{MODEL_NAME}: six stems decoded and validated in {:.2}s",
                validation_started.elapsed().as_secs_f64()
            ),
        );
        set_status(
            status,
            active_status(track_id, 0.995, "cachingStems", worker.backend.label()),
        );
        let cache_started = Instant::now();
        store_cache(
            package_path,
            track_id,
            metadata.len(),
            modified_ns(&metadata),
            &stems,
        )?;
        app_log::push_external(
            worker.backend.log_source(),
            "info",
            &format!(
                "{MODEL_NAME}: six-stem cache written in {:.2}s",
                cache_started.elapsed().as_secs_f64()
            ),
        );
        app.state::<AudioEngine>()
            .activate_stems(&media_path, stems)?;
        set_status(
            status,
            ready_status(track_id, false, worker.backend.label()),
        );
        app_log::push_external(
            worker.backend.log_source(),
            "info",
            &format!(
                "{MODEL_NAME}: six-stem generation completed in {:.2}s",
                separation_started.elapsed().as_secs_f64()
            ),
        );
        info!(%track_id, model = MODEL_NAME, backend = worker.backend.label(), "six-stem cache is ready");
        Ok(())
    });
    if output_dir.starts_with(&work_parent) && output_dir.exists() {
        if let Err(error) = fs::remove_dir_all(&output_dir) {
            warn!(path = %output_dir.display(), %error, "could not remove stem working directory");
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_worker(
    worker: &WorkerCommand,
    input: &Path,
    output: &Path,
    track_id: Uuid,
    generation: u64,
    active_generation: &AtomicU64,
    status: &Arc<Mutex<StemStatus>>,
    current_child: &Arc<Mutex<Option<Arc<Mutex<Child>>>>>,
) -> Result<(), AppError> {
    let mut command = Command::new(&worker.executable);
    command
        .args(&worker.prefix_arguments)
        .arg("separate")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--model-dir")
        .arg(&worker.model_dir)
        .args(&worker.additional_arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
    let mut child = command.spawn().map_err(|error| {
        AppError::StemSeparation(format!(
            "could not start the pinned {} runtime at {}: {error}",
            worker.backend.label(),
            worker.executable.display()
        ))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::StemSeparation("stem worker stdout is unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::StemSeparation("stem worker stderr is unavailable".into()))?;
    let child = Arc::new(Mutex::new(child));
    *current_child
        .lock()
        .map_err(|_| AppError::StemSeparation("stem process state is unavailable".into()))? =
        Some(Arc::clone(&child));
    let log_source = worker.backend.log_source();
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                let message: String = line.chars().take(8_192).collect();
                app_log::push_external(log_source, "info", &message);
            }
        }
    });
    let mut complete = false;
    let mut worker_error = None;
    for line in BufReader::new(stdout).lines() {
        if active_generation.load(Ordering::Acquire) != generation {
            break;
        }
        let line = line.map_err(|error| AppError::StemSeparation(error.to_string()))?;
        if line.len() > MAX_PROTOCOL_LINE {
            return Err(AppError::StemSeparation(
                "stem worker emitted an oversized protocol message".into(),
            ));
        }
        match serde_json::from_str::<WorkerEvent>(&line) {
            Ok(WorkerEvent::Stage { stage, progress })
            | Ok(WorkerEvent::Progress { stage, progress }) => set_status(
                status,
                active_status(track_id, progress, &stage, worker.backend.label()),
            ),
            Ok(WorkerEvent::Log { level, message }) => {
                app_log::push_external(worker.backend.log_source(), safe_level(&level), &message)
            }
            Ok(WorkerEvent::Complete { stems }) => {
                complete = stems == STEM_NAMES.map(str::to_owned);
                if !complete {
                    worker_error = Some("stem worker returned an invalid stem contract".into());
                }
            }
            Ok(WorkerEvent::Error { message }) => worker_error = Some(message),
            Ok(WorkerEvent::Unknown) => {}
            Err(error) => app_log::push_external(
                worker.backend.log_source(),
                "warn",
                &format!("ignored malformed worker event: {error}"),
            ),
        }
    }
    let exit = child
        .lock()
        .map_err(|_| AppError::StemSeparation("stem process state is unavailable".into()))?
        .wait()
        .map_err(|error| AppError::StemSeparation(error.to_string()))?;
    let _ = stderr_thread.join();
    if let Ok(mut current) = current_child.lock() {
        current.take();
    }
    if active_generation.load(Ordering::Acquire) != generation {
        return Err(AppError::StemSeparation("separation cancelled".into()));
    }
    if !exit.success() || !complete {
        return Err(AppError::StemSeparation(worker_error.unwrap_or_else(
            || format!("stem worker exited with status {exit}"),
        )));
    }
    Ok(())
}

fn resolve_worker(_app: &AppHandle) -> Result<WorkerCommand, AppError> {
    let backend = StemBackend::preferred();
    #[cfg(debug_assertions)]
    {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| AppError::StemSeparation("repository root is unavailable".into()))?;
        let (worker_root, environment_name, module) = match backend {
            StemBackend::Mlx => (
                root.join("tools/sonarcan-mlx-worker"),
                "SONARCAN_MLX_PYTHON",
                "sonarcan_mlx_worker",
            ),
            StemBackend::Torch => (
                root.join("tools/sonarcan-torch-worker"),
                "SONARCAN_TORCH_PYTHON",
                "sonarcan_torch_worker",
            ),
        };
        let executable = std::env::var_os(environment_name)
            .map(PathBuf::from)
            .unwrap_or_else(|| development_python(&worker_root));
        let model_dir = std::env::var_os("SONARCAN_MLX_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("src-tauri/resources/models/demucs-mlx"));
        let additional_arguments = portable_worker_arguments(backend)?;
        validated_worker(
            executable,
            vec!["-m".into(), module.into()],
            additional_arguments,
            model_dir,
            backend,
        )
    }
    #[cfg(not(debug_assertions))]
    {
        let resources = _app
            .path()
            .resource_dir()
            .map_err(|error| AppError::StemSeparation(error.to_string()))?;
        let (runtime, module) = match backend {
            StemBackend::Mlx => ("mlx-runtime", "sonarcan_mlx_worker"),
            StemBackend::Torch => ("stem-runtime", "sonarcan_torch_worker"),
        };
        validated_worker(
            bundled_python(&resources.join(runtime).join("runtime"), backend),
            vec!["-m".into(), module.into()],
            portable_worker_arguments(backend)?,
            resources.join("models/demucs-mlx"),
            backend,
        )
    }
}

#[cfg(debug_assertions)]
fn development_python(worker_root: &Path) -> PathBuf {
    if cfg!(windows) {
        worker_root.join(".venv/Scripts/python.exe")
    } else {
        worker_root.join(".venv/bin/python")
    }
}

#[cfg(not(debug_assertions))]
fn bundled_python(runtime: &Path, backend: StemBackend) -> PathBuf {
    if cfg!(windows) {
        runtime.join("python.exe")
    } else {
        runtime.join(match backend {
            StemBackend::Mlx => "bin/python3.13",
            StemBackend::Torch => "bin/python3.12",
        })
    }
}

fn portable_worker_arguments(backend: StemBackend) -> Result<Vec<String>, AppError> {
    if matches!(backend, StemBackend::Mlx) {
        return Ok(Vec::new());
    }
    let executable = ffmpeg::find().ok_or_else(|| {
        AppError::StemSeparation(
            "FFmpeg is required by the portable stem backend but is unavailable".into(),
        )
    })?;
    Ok(vec![
        "--ffmpeg".into(),
        executable.to_string_lossy().into_owned(),
    ])
}

fn validated_worker(
    executable: PathBuf,
    prefix_arguments: Vec<String>,
    additional_arguments: Vec<String>,
    model_dir: PathBuf,
    backend: StemBackend,
) -> Result<WorkerCommand, AppError> {
    // Preserve the final virtualenv symlink. Python uses the invoked path to
    // discover pyvenv.cfg; resolving it would silently run uv's base Python
    // without the pinned environment in development.
    let file_name = executable.file_name().ok_or_else(|| {
        AppError::StemSeparation("the pinned stem runtime path is invalid".into())
    })?;
    let executable = executable
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .map(|parent| parent.join(file_name))
        .ok_or_else(|| {
        AppError::StemSeparation(format!(
            "the pinned {} runtime is missing; prepare its development environment or install a release containing it ({})",
            backend.label(),
            executable.display()
        ))
    })?;
    if !executable.is_file() {
        return Err(AppError::StemSeparation(format!(
            "the pinned {} runtime is missing; prepare its development environment or install a release containing it ({})", backend.label(), executable.display()
        )));
    }
    if !model_dir.is_dir() || model_dir.is_symlink() {
        return Err(AppError::StemSeparation(format!(
            "the bundled {MODEL_NAME} model is missing from {}",
            model_dir.display()
        )));
    }
    Ok(WorkerCommand {
        executable,
        prefix_arguments,
        additional_arguments,
        model_dir,
        backend,
    })
}

fn load_worker_stems(
    output: &Path,
    source: &DecodedAudio,
) -> Result<[Arc<DecodedAudio>; STEM_COUNT], AppError> {
    let mut stems = Vec::with_capacity(STEM_COUNT);
    for name in STEM_NAMES {
        let path = output.join(format!("{name}.wav"));
        let metadata = fs::symlink_metadata(&path).map_err(|error| AppError::io(&path, error))?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_STEM_BYTES {
            return Err(AppError::StemSeparation(format!(
                "MLX produced an invalid {name} stem"
            )));
        }
        stems.push(Arc::new(align_stem(
            decode_stem_file(&path)?,
            source.sample_rate,
            source.frames,
        )));
    }
    stems
        .try_into()
        .map_err(|_| AppError::StemSeparation("MLX returned an invalid stem count".into()))
}

fn align_stem(stem: DecodedAudio, target_rate: u32, target_frames: usize) -> DecodedAudio {
    if stem.channels == 2 && stem.sample_rate == target_rate && stem.frames == target_frames {
        return stem;
    }
    let channels = stem.channels.max(1);
    let mut samples = vec![0.0; target_frames.saturating_mul(2)];
    if stem.frames > 0 && stem.sample_rate > 0 && target_rate > 0 {
        let ratio = stem.sample_rate as f64 / target_rate as f64;
        for frame in 0..target_frames {
            let position = frame as f64 * ratio;
            let first = (position.floor() as usize).min(stem.frames - 1);
            let second = (first + 1).min(stem.frames - 1);
            let fraction = (position - first as f64) as f32;
            for channel in 0..2 {
                let source_channel = channel.min(channels - 1);
                let a = stem.samples[first * channels + source_channel];
                let b = stem.samples[second * channels + source_channel];
                samples[frame * 2 + channel] = a + (b - a) * fraction;
            }
        }
    }
    DecodedAudio {
        samples,
        channels: 2,
        sample_rate: target_rate,
        frames: target_frames,
    }
}

fn active_status(track_id: Uuid, progress: f32, stage: &str, backend: &str) -> StemStatus {
    StemStatus {
        state: StemState::Separating,
        enabled: true,
        progress: progress.clamp(0.0, 1.0),
        stage: stage.chars().take(64).collect(),
        track_id: Some(track_id),
        cached: false,
        error: None,
        compute_backend: Some(backend.into()),
    }
}
fn ready_status(track_id: Uuid, cached: bool, backend: &str) -> StemStatus {
    StemStatus {
        state: StemState::Ready,
        enabled: true,
        progress: 1.0,
        stage: "ready".into(),
        track_id: Some(track_id),
        cached,
        error: None,
        compute_backend: Some(backend.into()),
    }
}
fn safe_level(level: &str) -> &str {
    match level {
        "debug" | "info" | "warn" | "error" => level,
        _ => "info",
    }
}
fn cache_dir(package: &Path, track_id: Uuid) -> PathBuf {
    package.join("Stems").join(track_id.to_string())
}

fn modified_ns(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn store_cache(
    package: &Path,
    track_id: Uuid,
    source_size: u64,
    source_modified_ns: u64,
    stems: &[Arc<DecodedAudio>; STEM_COUNT],
) -> Result<(), AppError> {
    let directory = cache_dir(package, track_id);
    fs::create_dir_all(&directory).map_err(|error| AppError::io(&directory, error))?;
    for (name, stem) in STEM_NAMES.iter().zip(stems) {
        let path = directory.join(format!("{name}.pcm"));
        let temporary = path.with_extension("pcm.tmp");
        let file = File::create(&temporary).map_err(|error| AppError::io(&temporary, error))?;
        let mut file = BufWriter::with_capacity(1024 * 1024, file);
        file.write_all(STEM_MAGIC)
            .and_then(|_| file.write_all(&stem.sample_rate.to_le_bytes()))
            .and_then(|_| file.write_all(&(stem.frames as u64).to_le_bytes()))
            .map_err(|error| AppError::io(&temporary, error))?;
        const CACHE_WRITE_SAMPLES: usize = 256 * 1024;
        let mut encoded = Vec::with_capacity(CACHE_WRITE_SAMPLES * size_of::<f32>());
        for samples in stem.samples.chunks(CACHE_WRITE_SAMPLES) {
            encoded.clear();
            for sample in samples {
                encoded.extend_from_slice(&sample.to_le_bytes());
            }
            file.write_all(&encoded)
                .map_err(|error| AppError::io(&temporary, error))?;
        }
        file.flush()
            .map_err(|error| AppError::io(&temporary, error))?;
        drop(file);
        fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
    }
    let manifest = StemManifest {
        cache_version: CACHE_VERSION,
        model_revision: MODEL_REVISION.into(),
        track_id,
        source_size,
        source_modified_ns,
        sample_rate: stems[0].sample_rate,
        frames: stems[0].frames,
    };
    let path = directory.join("manifest.json");
    let temporary = directory.join("manifest.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&manifest)?)
        .map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))
}

fn load_cache(
    package: &Path,
    track_id: Uuid,
    source_size: u64,
    source_modified_ns: u64,
) -> Option<[Arc<DecodedAudio>; STEM_COUNT]> {
    let directory = cache_dir(package, track_id);
    let manifest = read_valid_manifest(package, track_id, source_size, source_modified_ns)?;
    let mut loaded = Vec::with_capacity(STEM_COUNT);
    for name in STEM_NAMES {
        let mut file = File::open(directory.join(format!("{name}.pcm"))).ok()?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic).ok()?;
        if &magic != STEM_MAGIC {
            return None;
        }
        let mut u32_buffer = [0; 4];
        let mut u64_buffer = [0; 8];
        file.read_exact(&mut u32_buffer).ok()?;
        file.read_exact(&mut u64_buffer).ok()?;
        let sample_rate = u32::from_le_bytes(u32_buffer);
        let frames = u64::from_le_bytes(u64_buffer) as usize;
        let sample_count = frames.checked_mul(2)?;
        if sample_rate != manifest.sample_rate
            || frames != manifest.frames
            || sample_count > MAX_STEM_BYTES as usize / size_of::<f32>()
        {
            return None;
        }
        let mut bytes = vec![0; sample_count.checked_mul(size_of::<f32>())?];
        file.read_exact(&mut bytes).ok()?;
        if file.read(&mut [0; 1]).ok()? != 0 {
            return None;
        }
        let samples = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes([v[0], v[1], v[2], v[3]]))
            .collect();
        loaded.push(Arc::new(DecodedAudio {
            samples,
            channels: 2,
            sample_rate,
            frames,
        }));
    }
    loaded.try_into().ok()
}

fn read_valid_manifest(
    package: &Path,
    track_id: Uuid,
    source_size: u64,
    source_modified_ns: u64,
) -> Option<StemManifest> {
    read_valid_manifest_from_dir(
        &cache_dir(package, track_id),
        track_id,
        source_size,
        source_modified_ns,
    )
}

fn read_valid_manifest_from_dir(
    directory: &Path,
    track_id: Uuid,
    source_size: u64,
    source_modified_ns: u64,
) -> Option<StemManifest> {
    let path = directory.join("manifest.json");
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let file = File::open(path).ok()?;
    let manifest: StemManifest =
        serde_json::from_reader(BufReader::new(file.take(MAX_MANIFEST_BYTES))).ok()?;
    (manifest.cache_version == CACHE_VERSION
        && manifest.model_revision == MODEL_REVISION
        && manifest.track_id == track_id
        && manifest.source_size == source_size
        && manifest.source_modified_ns == source_modified_ns)
        .then_some(manifest)
}

fn set_status(target: &Arc<Mutex<StemStatus>>, value: StemStatus) {
    if let Ok(mut status) = target.lock() {
        *status = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn preserves_virtualenv_python_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let bin = directory.path().join(".venv/bin");
        let model = directory.path().join("model");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir(&model).unwrap();
        let base_python = directory.path().join("base-python");
        fs::write(&base_python, b"python").unwrap();
        let virtualenv_python = bin.join("python");
        symlink(&base_python, &virtualenv_python).unwrap();

        let worker = validated_worker(
            virtualenv_python.clone(),
            Vec::new(),
            Vec::new(),
            model,
            StemBackend::Mlx,
        )
        .unwrap();

        assert_eq!(worker.executable.file_name(), virtualenv_python.file_name());
        assert!(fs::symlink_metadata(&worker.executable)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_ne!(worker.executable, base_python.canonicalize().unwrap());
    }

    #[test]
    fn cache_round_trips_six_stems() {
        let project = tempfile::tempdir().unwrap();
        let track_id = Uuid::new_v4();
        let stems = std::array::from_fn(|index| {
            Arc::new(DecodedAudio {
                samples: vec![index as f32, 0.25, 0.5, 0.75],
                channels: 2,
                sample_rate: 48_000,
                frames: 2,
            })
        });
        store_cache(project.path(), track_id, 123, 456, &stems).unwrap();
        let loaded = load_cache(project.path(), track_id, 123, 456).unwrap();
        assert_eq!(loaded[0].samples, stems[0].samples);
        assert_eq!(loaded[5].samples, stems[5].samples);
        assert!(load_cache(project.path(), track_id, 124, 456).is_none());
    }

    #[test]
    fn aligns_mono_stem() {
        let aligned = align_stem(
            DecodedAudio {
                samples: vec![0.0, 1.0, 0.0],
                channels: 1,
                sample_rate: 3,
                frames: 3,
            },
            6,
            6,
        );
        assert_eq!(aligned.channels, 2);
        assert_eq!(aligned.frames, 6);
        assert_eq!(aligned.samples[0], aligned.samples[1]);
        assert!((aligned.samples[4] - 1.0).abs() < 0.001);
    }

    #[test]
    fn exports_cached_float_pcm_as_a_bounded_wave_file() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("vocals.pcm");
        let destination = directory.path().join("vocals.wav");
        let samples = [0.0_f32, 0.25, -0.5, 1.0];
        let mut cache = File::create(&source).unwrap();
        cache.write_all(STEM_MAGIC).unwrap();
        cache.write_all(&48_000_u32.to_le_bytes()).unwrap();
        cache.write_all(&2_u64.to_le_bytes()).unwrap();
        for sample in samples {
            cache.write_all(&sample.to_le_bytes()).unwrap();
        }
        drop(cache);

        copy_pcm_cache_to_wave(&source, &destination, 48_000, 2).unwrap();

        let wave = fs::read(destination).unwrap();
        assert_eq!(&wave[0..4], b"RIFF");
        assert_eq!(&wave[8..12], b"WAVE");
        assert_eq!(&wave[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([wave[20], wave[21]]), 3);
        assert_eq!(&wave[36..40], b"fact");
        assert_eq!(&wave[48..52], b"data");
        assert_eq!(u32::from_le_bytes(wave[52..56].try_into().unwrap()), 16);
        assert_eq!(&wave[56..], samples.map(f32::to_le_bytes).concat());
    }

    #[test]
    fn stem_export_names_are_safe_and_unique() {
        assert_eq!(sanitize_export_name("  Guitar/Bass: \n"), "GuitarBass");
        assert_eq!(sanitize_export_name("../"), "Stem");
        let mut used = Vec::new();
        assert_eq!(unique_export_name("Voice".into(), &mut used), "Voice");
        assert_eq!(unique_export_name("voice".into(), &mut used), "voice 2");
    }

    #[test]
    fn publishing_an_export_never_replaces_an_existing_directory() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary");
        let destination = directory.path().join("destination");
        fs::create_dir(&temporary).unwrap();
        fs::write(temporary.join("01 - Voice.wav"), b"new").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep.txt"), b"keep").unwrap();

        assert!(publish_export_directory(&temporary, &destination).is_err());
        assert_eq!(fs::read(destination.join("keep.txt")).unwrap(), b"keep");
        assert!(temporary.join("01 - Voice.wav").exists());
    }

    #[cfg(unix)]
    #[test]
    fn export_rejects_a_stem_cache_symlinked_outside_the_project() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("project.sac");
        let outside = directory.path().join("outside");
        let track_id = Uuid::new_v4();
        fs::create_dir(&package).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(outside.join(track_id.to_string())).unwrap();
        symlink(&outside, package.join("Stems")).unwrap();

        assert!(validated_export_cache_dir(&package, track_id).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn wave_export_rejects_a_symlinked_pcm_file() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside.pcm");
        let linked = directory.path().join("linked.pcm");
        let destination = directory.path().join("stem.wav");
        fs::write(&outside, b"not a stem").unwrap();
        symlink(&outside, &linked).unwrap();

        assert!(copy_pcm_cache_to_wave(&linked, &destination, 48_000, 2).is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn exports_six_cached_stems_to_mp3_when_ffmpeg_is_available() {
        if ffmpeg::find().is_none() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path().join("cache");
        let output = directory.path().join("output");
        fs::create_dir(&cache).unwrap();
        fs::create_dir(&output).unwrap();
        let frames = 800_usize;
        for (index, name) in STEM_NAMES.iter().enumerate() {
            let mut file = File::create(cache.join(format!("{name}.pcm"))).unwrap();
            file.write_all(STEM_MAGIC).unwrap();
            file.write_all(&8_000_u32.to_le_bytes()).unwrap();
            file.write_all(&(frames as u64).to_le_bytes()).unwrap();
            for frame in 0..frames {
                let sample = ((frame as f32 * 0.03) + index as f32).sin() * 0.2;
                file.write_all(&sample.to_le_bytes()).unwrap();
                file.write_all(&sample.to_le_bytes()).unwrap();
            }
        }
        let manifest = StemManifest {
            cache_version: CACHE_VERSION,
            model_revision: MODEL_REVISION.into(),
            track_id: Uuid::new_v4(),
            source_size: 0,
            source_modified_ns: 0,
            sample_rate: 8_000,
            frames,
        };
        let names = ["Voice", "Drums", "Bass", "Other", "Guitar", "Piano"].map(str::to_owned);

        export_into_directory(
            &cache,
            &output,
            StemExportFormat::Mp3,
            &names,
            Mp3Quality::VbrHigh,
            &manifest,
        )
        .unwrap();

        let files: Vec<PathBuf> = fs::read_dir(output)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), STEM_COUNT);
        assert!(files
            .iter()
            .all(|path| path.extension().is_some_and(|extension| extension == "mp3")));
        assert!(files.iter().all(|path| path.metadata().unwrap().len() > 0));
    }
}
