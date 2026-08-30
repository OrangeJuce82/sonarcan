//! Supervision, validation and versioned caching for librosa chord features.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    chord_engine::{self, ChordAnalysis, ExtractedChordFeatures},
    error::AppError,
    stems,
};

const CACHE_VERSION: u32 = 7;
const FEATURE_VERSION: u32 = 3;
const MAX_SEGMENTS: usize = 4_096;
const MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DURATION_SECONDS: f64 = 24.0 * 60.0 * 60.0;

#[derive(Default)]
pub struct ChordAnalysisService {
    generation: AtomicU64,
    child: Mutex<Option<Arc<Mutex<Child>>>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheEnvelope {
    cache_version: u32,
    source_size: u64,
    source_modified_ns: u128,
    stem_assisted: bool,
    analysis: ChordAnalysis,
}

struct WorkerCommand {
    executable: PathBuf,
    prefix_arguments: Vec<String>,
}

impl ChordAnalysisService {
    pub fn begin(&self) -> u64 {
        self.cancel();
        self.generation.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.take() {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
        }
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
        let source_identity = source_identity(media_path)?;
        let chord_stems = stems::chord_analysis_stems(package_path, track_id, media_path);
        let stem_assisted = chord_stems.is_some();
        if let Some(cached) = load_cached(package_path, track_id, source_identity, stem_assisted)? {
            return Ok(cached);
        }

        let worker = resolve_worker(app)?;
        let mut command = Command::new(&worker.executable);
        command
            .args(&worker.prefix_arguments)
            .arg(media_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(paths) = chord_stems {
            command.arg("--stems").args(paths);
        }
        let mut child = command.spawn().map_err(|error| {
            AppError::ChordAnalysis(format!("could not start librosa worker: {error}"))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::ChordAnalysis("librosa stdout is unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::ChordAnalysis("librosa stderr is unavailable".into()))?;
        let child = Arc::new(Mutex::new(child));
        *self.child.lock().map_err(|_| {
            AppError::ChordAnalysis("analysis process state is unavailable".into())
        })? = Some(Arc::clone(&child));

        let stdout_reader = std::thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = std::thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let status = wait_for_child(&child)?;
        clear_child(&self.child, &child);
        let stdout = stdout_reader
            .join()
            .map_err(|_| AppError::ChordAnalysis("librosa stdout reader failed".into()))?
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not read librosa stdout: {error}"))
            })?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AppError::ChordAnalysis("librosa stderr reader failed".into()))?
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not read librosa stderr: {error}"))
            })?;
        ensure_current(&self.generation, generation)?;
        if !status.success() {
            return Err(AppError::ChordAnalysis(format!(
                "librosa feature extraction failed: {}",
                String::from_utf8_lossy(&stderr.bytes).trim()
            )));
        }
        if stdout.exceeded {
            return Err(AppError::ChordAnalysis(
                "librosa feature output exceeded 8 MiB".into(),
            ));
        }
        let features: ExtractedChordFeatures =
            serde_json::from_slice(&stdout.bytes).map_err(|error| {
                AppError::ChordAnalysis(format!("invalid librosa feature output: {error}"))
            })?;
        validate_features(&features)?;
        let analysis = chord_engine::decode(track_id, CACHE_VERSION, &features);
        ensure_current(&self.generation, generation)?;
        store(package_path, source_identity, stem_assisted, &analysis)?;
        Ok(analysis)
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
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

fn clear_child(slot: &Mutex<Option<Arc<Mutex<Child>>>>, completed: &Arc<Mutex<Child>>) {
    if let Ok(mut slot) = slot.lock() {
        if slot
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, completed))
        {
            *slot = None;
        }
    }
}

fn wait_for_child(child: &Arc<Mutex<Child>>) -> Result<std::process::ExitStatus, AppError> {
    loop {
        let status = child
            .lock()
            .map_err(|_| AppError::ChordAnalysis("analysis process state is unavailable".into()))?
            .try_wait()
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not wait for librosa worker: {error}"))
            })?;
        if let Some(status) = status {
            return Ok(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn ensure_current(active: &AtomicU64, generation: u64) -> Result<(), AppError> {
    if active.load(Ordering::Acquire) == generation {
        Ok(())
    } else {
        Err(AppError::ChordAnalysis(
            "chord analysis was cancelled or superseded".into(),
        ))
    }
}

fn validate_features(features: &ExtractedChordFeatures) -> Result<(), AppError> {
    if features.feature_version != FEATURE_VERSION
        || !features.duration_seconds.is_finite()
        || !(0.0..=MAX_DURATION_SECONDS).contains(&features.duration_seconds)
        || features.segments.len() > MAX_SEGMENTS
        || features.key_root.is_some_and(|root| root >= 12)
    {
        return Err(AppError::ChordAnalysis(
            "librosa feature header is outside accepted bounds".into(),
        ));
    }
    let mut previous_end = 0.0;
    for segment in &features.segments {
        let scalars_valid = segment.start_seconds.is_finite()
            && segment.end_seconds.is_finite()
            && segment.silence.is_finite()
            && segment.ambiguity.is_finite()
            && segment.bass_strength.is_finite()
            && segment.start_seconds >= previous_end - 0.001
            && segment.start_seconds >= 0.0
            && segment.end_seconds > segment.start_seconds
            && segment.end_seconds <= features.duration_seconds + 0.05
            && !matches!(segment.key_root, Some(root) if root >= 12)
            && (0.0..=1.0).contains(&segment.silence)
            && (0.0..=1.0).contains(&segment.ambiguity)
            && (0.0..=1.0).contains(&segment.bass_strength);
        let vectors_valid = segment
            .chroma
            .iter()
            .chain(segment.bass_chroma.iter())
            .all(|value| value.is_finite() && (0.0..=1.001).contains(value));
        if !scalars_valid || !vectors_valid {
            return Err(AppError::ChordAnalysis(
                "librosa segment is outside accepted bounds".into(),
            ));
        }
        previous_end = segment.end_seconds;
    }
    Ok(())
}

fn resolve_worker(app: &AppHandle) -> Result<WorkerCommand, AppError> {
    if let Ok(resources) = app.path().resource_dir() {
        for relative in [
            "mlx-runtime/bin/python3",
            "mlx-runtime/bin/python",
            "mlx-runtime/python.exe",
        ] {
            let executable = resources.join(relative);
            if executable.is_file() {
                return Ok(WorkerCommand {
                    executable,
                    prefix_arguments: vec!["-m".into(), "sonarcan_mlx_worker.chords".into()],
                });
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("tools/sonarcan-mlx-worker");
        Ok(WorkerCommand {
            executable: PathBuf::from("uv"),
            prefix_arguments: vec![
                "run".into(),
                "--project".into(),
                project.to_string_lossy().into_owned(),
                "--locked".into(),
                "python".into(),
                "-m".into(),
                "sonarcan_mlx_worker.chords".into(),
            ],
        })
    }
    #[cfg(not(debug_assertions))]
    Err(AppError::ChordAnalysis(
        "the bundled librosa runtime is unavailable".into(),
    ))
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
    stem_assisted: bool,
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
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(path, error)),
    };
    let Some(cached) = serde_json::from_slice::<CacheEnvelope>(&bytes).ok() else {
        return Ok(None);
    };
    Ok((cached.cache_version == CACHE_VERSION
        && cached.analysis.cache_version == CACHE_VERSION
        && cached.analysis.track_id == track_id
        && cached.stem_assisted == stem_assisted
        && (cached.source_size, cached.source_modified_ns) == source)
        .then_some(cached.analysis))
}

fn store(
    package_path: &Path,
    source: (u64, u128),
    stem_assisted: bool,
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
        stem_assisted,
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
    use crate::chord_engine::FeatureSegment;

    #[test]
    fn rejects_unbounded_or_non_finite_worker_features() {
        let mut features = ExtractedChordFeatures {
            feature_version: FEATURE_VERSION,
            duration_seconds: 2.0,
            key_root: Some(0),
            key_minor: Some(false),
            segments: vec![FeatureSegment {
                start_seconds: 0.0,
                end_seconds: 2.0,
                chroma: [0.0; 12],
                bass_chroma: [0.0; 12],
                bass_strength: 0.0,
                silence: 1.0,
                ambiguity: 1.0,
                key_root: None,
                key_minor: None,
            }],
        };
        assert!(validate_features(&features).is_ok());
        features.segments[0].chroma[3] = f32::NAN;
        assert!(validate_features(&features).is_err());
        features.segments[0].chroma[3] = 0.0;
        features.segments[0].end_seconds = 3.0;
        assert!(validate_features(&features).is_err());
    }

    #[test]
    fn cache_is_tied_to_source_identity_and_version() {
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir(temporary.path().join("Analysis")).unwrap();
        let track_id = Uuid::new_v4();
        let analysis = ChordAnalysis {
            cache_version: CACHE_VERSION,
            track_id,
            chords: Vec::new(),
            simple_chords: Vec::new(),
        };
        store(temporary.path(), (12, 34), false, &analysis).unwrap();
        assert_eq!(
            load_cached(temporary.path(), track_id, (12, 34), false).unwrap(),
            Some(analysis)
        );
        assert_eq!(
            load_cached(temporary.path(), track_id, (13, 34), false).unwrap(),
            None
        );
        assert_eq!(
            load_cached(temporary.path(), track_id, (12, 34), true).unwrap(),
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

    #[test]
    fn bounded_reader_drains_but_retains_only_the_limit() {
        let output = read_bounded(&b"abcdef"[..], 4).unwrap();
        assert_eq!(output.bytes, b"abcd");
        assert!(output.exceeded);
    }
}
