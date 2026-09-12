//! LV-Chordia process supervision, validation, cancellation, and caching.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::info;
use uuid::Uuid;

use crate::{
    audio_engine::decode_stem_file,
    beat_inference,
    beat_preprocessing::mono_22050,
    chord_contract::{ChordAnalysis, WorkerAnalysis},
    error::AppError,
    inference_backend::{preferred_backends, select_first_available, InferenceBackend},
    lv_cqt::{estimate_tuning_36, hybrid_cqt_22050},
    lv_decoder, lv_inference, model_install, resource_paths,
};

const CACHE_VERSION: u32 = 15;
const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;
const BACKEND_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const LV_MODEL_VERSION: &str = "lv-chordia@9d7de7bbf45efa6731ec8dc62d35280f141c0702";
const BEAT_MODEL_VERSION: &str = "beat-this@1.1.0:final0";

#[derive(Default)]
pub struct ChordAnalysisService {
    generation: AtomicU64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheEnvelope {
    cache_version: u32,
    source_size: u64,
    source_modified_ns: u128,
    analysis: ChordAnalysis,
}

impl ChordAnalysisService {
    pub fn begin(&self) -> u64 {
        self.cancel();
        self.generation.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn analyze(
        &self,
        app: &AppHandle,
        package_path: &Path,
        track_id: Uuid,
        media_path: &Path,
        generation: u64,
    ) -> Result<ChordAnalysis, AppError> {
        ensure_current(&self.generation, generation)?;
        let source = source_identity(media_path)?;
        if let Some(cached) = load_cached(package_path, track_id, source)? {
            return Ok(cached);
        }

        let worker = native_analyze(app, media_path)?;
        let analysis = worker.validate(track_id, CACHE_VERSION)?;
        ensure_current(&self.generation, generation)?;
        if analysis.warnings.is_empty() {
            store(package_path, source, &analysis)?;
        }
        Ok(analysis)
    }
}

fn ensure_current(active: &AtomicU64, generation: u64) -> Result<(), AppError> {
    (active.load(Ordering::Acquire) == generation)
        .then_some(())
        .ok_or_else(|| AppError::ChordAnalysis("chord analysis was cancelled or superseded".into()))
}

fn native_analyze(app: &AppHandle, media_path: &Path) -> Result<WorkerAnalysis, AppError> {
    let started = Instant::now();
    let worker = resolve_worker()?;
    let decoded = decode_stem_file(media_path)?;
    let duration = Duration::from_secs_f64(decoded.frames as f64 / f64::from(decoded.sample_rate));
    let mono = mono_22050(&decoded.samples, decoded.channels, decoded.sample_rate)?;
    let beat_spectrogram = crate::beat_preprocessing::log_mel_22050(&mono)?;
    let priorities = preferred_backends(
        std::env::consts::OS,
        std::env::consts::ARCH,
        option_env!("SONARCAN_GPU_BACKEND") == Some("nvidia"),
    );
    let beat_programs = model_install::beat_programs(
        app,
        beat_spectrogram.frames >= beat_inference::RETAINED_FRAMES,
    )?;
    let beat_backend =
        select_first_available(&worker, &priorities, &beat_programs, BACKEND_PROBE_TIMEOUT)?;
    let scratch = ScratchDirectory::new(app)?;
    let rhythm = beat_inference::infer_fixed_windows(
        &beat_backend.backend,
        &beat_backend.program,
        &beat_spectrogram,
        &scratch.path,
        duration,
    )?;

    let tuning = estimate_tuning_36(&mono)?;
    let cqt = hybrid_cqt_22050(&mono, &model_install::lv_cqt_kernel(app, tuning)?)?;
    let lv_programs = model_install::lv_program_roots(app)?;
    let lv_backend =
        select_first_available(&worker, &priorities, &lv_programs, BACKEND_PROBE_TIMEOUT)?;
    let lv_root = model_install::lv_program_root(app, lv_backend.backend.kind())?;
    let ensemble =
        lv_inference::infer_ensemble(&lv_backend.backend, &lv_root, &cqt, &scratch.path, duration)?;
    let modes = lv_decoder::decode_modes(&ensemble.probabilities, ensemble.frames)?;
    let beat_inference_ms = rhythm
        .measurements
        .iter()
        .map(|measurement| measurement.inference_time_ms)
        .sum::<f64>();
    let chord_inference_ms = ensemble
        .measurements
        .iter()
        .map(|measurement| measurement.inference_time_ms)
        .sum::<f64>();
    info!(
        beat_backend = beat_backend.backend.kind().label(),
        chord_backend = lv_backend.backend.kind().label(),
        beat_inference_ms,
        chord_inference_ms,
        wall_time_ms = started.elapsed().as_secs_f64() * 1_000.0,
        tuning,
        "native chord and rhythm analysis completed"
    );
    Ok(WorkerAnalysis::native(
        LV_MODEL_VERSION.into(),
        BEAT_MODEL_VERSION.into(),
        rhythm.timeline.bpm,
        rhythm.timeline.beats,
        rhythm.timeline.downbeats,
        rhythm.dbn_timeline.bpm,
        rhythm.dbn_timeline.beats,
        rhythm.dbn_timeline.downbeats,
        modes,
    ))
}

fn resolve_worker() -> Result<PathBuf, AppError> {
    let path = resource_paths::resource_path("executorch-runtime/sonarcan-executorch-worker")
        .or_else(|| {
            #[cfg(debug_assertions)]
            {
                Some(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("resources/executorch-runtime/sonarcan-executorch-worker"),
                )
            }
            #[cfg(not(debug_assertions))]
            {
                None
            }
        })
        .ok_or_else(|| AppError::ChordAnalysis("ExecuTorch worker path is unavailable".into()))?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| AppError::io(&path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::ChordAnalysis(
            "ExecuTorch worker is not a regular file".into(),
        ));
    }
    Ok(path)
}

pub fn accelerator_self_test(_app: &AppHandle) -> bool {
    let Ok(worker) = resolve_worker() else {
        return false;
    };
    let Ok(output) = Command::new(worker).arg("--backends").output() else {
        return false;
    };
    let Ok(backends) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return backends.get("MLXBackend").and_then(|value| value.as_bool()) == Some(true);
    }
    if cfg!(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        return backends
            .get("CudaBackend")
            .and_then(|value| value.as_bool())
            == Some(true)
            && Command::new("nvidia-smi")
                .arg("-L")
                .output()
                .is_ok_and(|probe| probe.status.success() && !probe.stdout.is_empty());
    }
    false
}

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new(app: &AppHandle) -> Result<Self, AppError> {
        let root = model_install::native_model_root(app)?.join(".scratch");
        fs::create_dir_all(&root).map_err(|error| AppError::io(&root, error))?;
        let metadata = fs::symlink_metadata(&root).map_err(|error| AppError::io(&root, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AppError::ChordAnalysis(
                "native inference scratch root is invalid".into(),
            ));
        }
        let path = root.join(Uuid::new_v4().to_string());
        fs::create_dir(&path).map_err(|error| AppError::io(&path, error))?;
        Ok(Self { path })
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn source_identity(path: &Path) -> Result<(u64, u128), AppError> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io(path, error))?;
    let modified = metadata
        .modified()
        .map_err(|error| AppError::io(path, error))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            AppError::ChordAnalysis(format!("invalid audio modification time: {error}"))
        })?
        .as_nanos();
    Ok((metadata.len(), modified))
}

fn load_cached(
    package_path: &Path,
    track_id: Uuid,
    source: (u64, u128),
) -> Result<Option<ChordAnalysis>, AppError> {
    let path = cache_path(package_path, track_id)?;
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.len() > MAX_CACHE_BYTES)
    {
        return Ok(None);
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(path, error)),
    };
    let Some(cached) = serde_json::from_slice::<CacheEnvelope>(&bytes).ok() else {
        return Ok(None);
    };
    Ok((cached.cache_version == CACHE_VERSION
        && cached.analysis.cache_version == CACHE_VERSION
        && cached.analysis.track_id == track_id
        && (cached.source_size, cached.source_modified_ns) == source)
        .then_some(cached.analysis))
}

fn store(
    package_path: &Path,
    source: (u64, u128),
    analysis: &ChordAnalysis,
) -> Result<(), AppError> {
    let path = cache_path(package_path, analysis.track_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::ChordAnalysis("chord cache path has no parent".into()))?;
    let temporary = parent.join(format!(".{}-{}.tmp", analysis.track_id, Uuid::new_v4()));
    let envelope = CacheEnvelope {
        cache_version: CACHE_VERSION,
        source_size: source.0,
        source_modified_ns: source.1,
        analysis: analysis.clone(),
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| AppError::io(&temporary, error))?;
    file.write_all(&serde_json::to_vec(&envelope)?)
        .map_err(|error| AppError::io(&temporary, error))?;
    file.sync_all()
        .map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
    Ok(())
}

fn cache_path(package_path: &Path, track_id: Uuid) -> Result<PathBuf, AppError> {
    let canonical_package = package_path
        .canonicalize()
        .map_err(|error| AppError::io(package_path, error))?;
    let analysis_directory = package_path.join("Analysis");
    let canonical_analysis = analysis_directory
        .canonicalize()
        .map_err(|error| AppError::io(&analysis_directory, error))?;
    if !canonical_analysis.starts_with(&canonical_package) {
        return Err(AppError::AnalysisCacheOutsideProject(analysis_directory));
    }
    let chord_directory = canonical_analysis.join("chords");
    if !chord_directory.exists() {
        fs::create_dir(&chord_directory).map_err(|error| AppError::io(&chord_directory, error))?;
    }
    let canonical_chords = chord_directory
        .canonicalize()
        .map_err(|error| AppError::io(&chord_directory, error))?;
    if !canonical_chords.starts_with(&canonical_analysis) {
        return Err(AppError::AnalysisCacheOutsideProject(chord_directory));
    }
    let path = canonical_chords.join(format!("{track_id}.json"));
    if path.exists() {
        let canonical_path = path
            .canonicalize()
            .map_err(|error| AppError::io(&path, error))?;
        if !canonical_path.starts_with(&canonical_chords) {
            return Err(AppError::AnalysisCacheOutsideProject(path));
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn analysis(track_id: Uuid) -> ChordAnalysis {
        ChordAnalysis {
            cache_version: CACHE_VERSION,
            track_id,
            model_version: "lv-chordia@test".into(),
            downbeat_model_version: "beat-this@test".into(),
            bpm: Some(120.0),
            beats: vec![0.5, 1.0, 1.5, 2.0, 2.5],
            downbeats: vec![0.5, 2.5],
            dbn_bpm: Some(120.0),
            dbn_beats: vec![0.5, 1.0, 1.5, 2.0, 2.5],
            dbn_downbeats: vec![0.5, 2.5],
            modes: BTreeMap::new(),
            warnings: vec![],
        }
    }

    #[test]
    fn cache_is_tied_to_source_identity_and_version() {
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir(temporary.path().join("Analysis")).unwrap();
        let track_id = Uuid::new_v4();
        let analysis = analysis(track_id);
        store(temporary.path(), (12, 34), &analysis).unwrap();
        assert_eq!(
            load_cached(temporary.path(), track_id, (12, 34)).unwrap(),
            Some(analysis)
        );
        assert_eq!(
            load_cached(temporary.path(), track_id, (13, 34)).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_chord_cache_symlinked_outside_the_project() {
        use std::os::unix::fs::symlink;
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), project.path().join("Analysis")).unwrap();
        assert!(matches!(
            cache_path(project.path(), Uuid::new_v4()),
            Err(AppError::AnalysisCacheOutsideProject(_))
        ));
    }
}
