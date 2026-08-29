//! Supervision of the pinned `demucs-mlx` six-stem worker.
//! Inference and file I/O never run on the CPAL callback.

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app_log,
    audio_engine::{decode_stem_file, AudioEngine, DecodedAudio},
    error::AppError,
    project,
    stem_contract::{MODEL_NAME, MODEL_REVISION, STEM_COUNT, STEM_NAMES},
};

const CACHE_VERSION: u32 = 2;
const STEM_MAGIC: &[u8; 8] = b"SACSTM02";
const MAX_PROTOCOL_LINE: usize = 16 * 1024;
const MAX_STEM_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemStatus {
    pub state: StemState,
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
    model_dir: PathBuf,
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
        app_log::push_external("mlx", "info", "stem separation disabled");
    }

    pub fn start(
        &self,
        app: AppHandle,
        package_path: PathBuf,
        track_id: Uuid,
    ) -> Result<(), AppError> {
        ensure_supported_platform()?;
        if self.running.swap(true, Ordering::AcqRel) {
            return Err(AppError::StemSeparation(
                "another stem separation is already running".into(),
            ));
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        set_status(&self.status, active_status(track_id, 0.0, "checkingCache"));
        let status = Arc::clone(&self.status);
        let running = Arc::clone(&self.running);
        let child = Arc::clone(&self.child);
        let active_generation = Arc::clone(&self.generation);
        std::thread::Builder::new()
            .name("sonarcan-demucs-mlx".into())
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
                        warn!(%track_id, %error, "demucs-mlx separation failed");
                        app_log::push_external("mlx", "error", &error.to_string());
                        set_status(
                            &status,
                            StemStatus {
                                state: StemState::Failed,
                                progress: 0.0,
                                stage: "failed".into(),
                                track_id: Some(track_id),
                                cached: false,
                                error: Some(error.to_string()),
                                compute_backend: Some("MLX".into()),
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

fn ensure_supported_platform() -> Result<(), AppError> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok(())
    } else {
        Err(AppError::StemSeparation(
            "demucs-mlx requires an Apple-silicon Mac".into(),
        ))
    }
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
        set_status(status, ready_status(track_id, true));
        return Ok(());
    }
    let worker = resolve_worker(app)?;
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
        set_status(status, active_status(track_id, 0.985, "validatingStems"));
        let stems = load_worker_stems(&output_dir, &source)?;
        store_cache(
            package_path,
            track_id,
            metadata.len(),
            modified_ns(&metadata),
            &stems,
        )?;
        app.state::<AudioEngine>()
            .activate_stems(&media_path, stems)?;
        set_status(status, ready_status(track_id, false));
        info!(%track_id, model = MODEL_NAME, "six-stem MLX cache is ready");
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
    let mut child = command.spawn().map_err(|error| {
        AppError::StemSeparation(format!(
            "could not start the pinned MLX runtime at {}: {error}",
            worker.executable.display()
        ))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::StemSeparation("MLX stdout is unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::StemSeparation("MLX stderr is unavailable".into()))?;
    let child = Arc::new(Mutex::new(child));
    *current_child
        .lock()
        .map_err(|_| AppError::StemSeparation("MLX process state is unavailable".into()))? =
        Some(Arc::clone(&child));
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                let message: String = line.chars().take(8_192).collect();
                app_log::push_external("mlx", "info", &message);
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
                "MLX worker emitted an oversized protocol message".into(),
            ));
        }
        match serde_json::from_str::<WorkerEvent>(&line) {
            Ok(WorkerEvent::Stage { stage, progress })
            | Ok(WorkerEvent::Progress { stage, progress }) => {
                set_status(status, active_status(track_id, progress, &stage))
            }
            Ok(WorkerEvent::Log { level, message }) => {
                app_log::push_external("mlx", safe_level(&level), &message)
            }
            Ok(WorkerEvent::Complete { stems }) => {
                complete = stems == STEM_NAMES.map(str::to_owned);
                if !complete {
                    worker_error = Some("MLX worker returned an invalid stem contract".into());
                }
            }
            Ok(WorkerEvent::Error { message }) => worker_error = Some(message),
            Ok(WorkerEvent::Unknown) => {}
            Err(error) => app_log::push_external(
                "mlx",
                "warn",
                &format!("ignored malformed worker event: {error}"),
            ),
        }
    }
    let exit = child
        .lock()
        .map_err(|_| AppError::StemSeparation("MLX process state is unavailable".into()))?
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
            || format!("MLX worker exited with status {exit}"),
        )));
    }
    Ok(())
}

fn resolve_worker(_app: &AppHandle) -> Result<WorkerCommand, AppError> {
    #[cfg(debug_assertions)]
    {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| AppError::StemSeparation("repository root is unavailable".into()))?;
        let worker_root = root.join("tools/sonarcan-mlx-worker");
        let executable = std::env::var_os("SONARCAN_MLX_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| worker_root.join(".venv/bin/python"));
        let model_dir = std::env::var_os("SONARCAN_MLX_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("src-tauri/resources/models/demucs-mlx"));
        validated_worker(
            executable,
            vec!["-m".into(), "sonarcan_mlx_worker".into()],
            model_dir,
        )
    }
    #[cfg(not(debug_assertions))]
    {
        let resources = _app
            .path()
            .resource_dir()
            .map_err(|error| AppError::StemSeparation(error.to_string()))?;
        validated_worker(
            resources.join("mlx-runtime/runtime/bin/python3.13"),
            vec!["-m".into(), "sonarcan_mlx_worker".into()],
            resources.join("models/demucs-mlx"),
        )
    }
}

fn validated_worker(
    executable: PathBuf,
    prefix_arguments: Vec<String>,
    model_dir: PathBuf,
) -> Result<WorkerCommand, AppError> {
    let executable = executable.canonicalize().map_err(|_| {
        AppError::StemSeparation(format!(
            "the pinned MLX runtime is missing; run `npm run mlx:sync` in development or install a release containing it ({})",
            executable.display()
        ))
    })?;
    if !executable.is_file() {
        return Err(AppError::StemSeparation(format!(
            "the pinned MLX runtime is missing; run `npm run mlx:sync` in development or install a release containing it ({})", executable.display()
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
        model_dir,
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

fn active_status(track_id: Uuid, progress: f32, stage: &str) -> StemStatus {
    StemStatus {
        state: StemState::Separating,
        progress: progress.clamp(0.0, 1.0),
        stage: stage.chars().take(64).collect(),
        track_id: Some(track_id),
        cached: false,
        error: None,
        compute_backend: Some("MLX".into()),
    }
}
fn ready_status(track_id: Uuid, cached: bool) -> StemStatus {
    StemStatus {
        state: StemState::Ready,
        progress: 1.0,
        stage: "ready".into(),
        track_id: Some(track_id),
        cached,
        error: None,
        compute_backend: Some("MLX".into()),
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
        let mut file = File::create(&temporary).map_err(|error| AppError::io(&temporary, error))?;
        file.write_all(STEM_MAGIC)
            .and_then(|_| file.write_all(&stem.sample_rate.to_le_bytes()))
            .and_then(|_| file.write_all(&(stem.frames as u64).to_le_bytes()))
            .map_err(|error| AppError::io(&temporary, error))?;
        for sample in &stem.samples {
            file.write_all(&sample.to_le_bytes())
                .map_err(|error| AppError::io(&temporary, error))?;
        }
        file.sync_all()
            .map_err(|error| AppError::io(&temporary, error))?;
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
    let manifest: StemManifest =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).ok()?).ok()?;
    if manifest.cache_version != CACHE_VERSION
        || manifest.model_revision != MODEL_REVISION
        || manifest.track_id != track_id
        || manifest.source_size != source_size
        || manifest.source_modified_ns != source_modified_ns
    {
        return None;
    }
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

fn set_status(target: &Arc<Mutex<StemStatus>>, value: StemStatus) {
    if let Ok(mut status) = target.lock() {
        *status = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
