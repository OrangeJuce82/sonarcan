//! LV-Chordia process supervision, validation, cancellation, and caching.

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
use tauri::AppHandle;
use uuid::Uuid;

#[cfg(not(debug_assertions))]
use crate::python_runtime;
use crate::{
    chord_contract::{ChordAnalysis, WorkerAnalysis},
    error::AppError,
};

const CACHE_VERSION: u32 = 15;
const MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;

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
        let source = source_identity(media_path)?;
        if let Some(cached) = load_cached(package_path, track_id, source)? {
            return Ok(cached);
        }

        let worker = resolve_worker(app)?;
        let downbeat_model = resolve_downbeat_model(app)?;
        let mut child = Command::new(&worker.executable)
            .args(&worker.prefix_arguments)
            .arg("--downbeat-model")
            .arg(downbeat_model)
            .arg(media_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not start LV-Chordia worker: {error}"))
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::ChordAnalysis("LV-Chordia stdout is unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::ChordAnalysis("LV-Chordia stderr is unavailable".into()))?;
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
            .map_err(|_| AppError::ChordAnalysis("LV-Chordia stdout reader failed".into()))?
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not read LV-Chordia stdout: {error}"))
            })?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AppError::ChordAnalysis("LV-Chordia stderr reader failed".into()))?
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not read LV-Chordia stderr: {error}"))
            })?;
        ensure_current(&self.generation, generation)?;
        if !status.success() {
            return Err(AppError::ChordAnalysis(format!(
                "LV-Chordia failed: {}",
                String::from_utf8_lossy(&stderr.bytes).trim()
            )));
        }
        if stdout.exceeded {
            return Err(AppError::ChordAnalysis(
                "LV-Chordia output exceeded 8 MiB".into(),
            ));
        }
        let worker: WorkerAnalysis = serde_json::from_slice(&stdout.bytes).map_err(|error| {
            AppError::ChordAnalysis(format!("invalid LV-Chordia JSON: {error}"))
        })?;
        let analysis = worker.validate(track_id, CACHE_VERSION)?;
        ensure_current(&self.generation, generation)?;
        if analysis.warnings.is_empty() {
            store(package_path, source, &analysis)?;
        }
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
                AppError::ChordAnalysis(format!("could not wait for LV-Chordia: {error}"))
            })?;
        if let Some(status) = status {
            return Ok(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn ensure_current(active: &AtomicU64, generation: u64) -> Result<(), AppError> {
    (active.load(Ordering::Acquire) == generation)
        .then_some(())
        .ok_or_else(|| AppError::ChordAnalysis("chord analysis was cancelled or superseded".into()))
}

fn resolve_worker(app: &AppHandle) -> Result<WorkerCommand, AppError> {
    #[cfg(debug_assertions)]
    {
        let _ = app;
        Ok(development_worker())
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        if let Some(executable) = python_runtime::bundled_python_313() {
            return Ok(WorkerCommand {
                executable,
                prefix_arguments: vec!["-m".into(), "sonarcan_chord_worker.worker".into()],
            });
        }
        Err(AppError::ChordAnalysis(
            "the bundled LV-Chordia runtime is unavailable".into(),
        ))
    }
}

fn resolve_downbeat_model(app: &AppHandle) -> Result<PathBuf, AppError> {
    #[cfg(debug_assertions)]
    {
        let _ = app;
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models/beat-this/final0.ckpt");
        path.is_file().then_some(path).ok_or_else(|| {
            AppError::ChordAnalysis(
                "the Beat This! model is unavailable; run npm run chords:downbeat-model".into(),
            )
        })
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        return crate::python_runtime::resource_path("models/beat-this/final0.ckpt")
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                AppError::ChordAnalysis("the bundled Beat This! model is unavailable".into())
            });
    }
}

#[cfg(debug_assertions)]
fn development_worker() -> WorkerCommand {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .join("tools/sonarcan-chord-worker");
    WorkerCommand {
        executable: PathBuf::from("uv"),
        prefix_arguments: vec![
            "run".into(),
            "--project".into(),
            project.to_string_lossy().into_owned(),
            "--locked".into(),
            "python".into(),
            "-m".into(),
            "sonarcan_chord_worker.worker".into(),
        ],
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
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
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

    #[test]
    fn development_uses_the_current_source_worker() {
        let worker = development_worker();
        assert_eq!(worker.executable, PathBuf::from("uv"));
        assert!(worker
            .prefix_arguments
            .windows(2)
            .any(|arguments| arguments[0] == "--project"
                && arguments[1].ends_with("tools/sonarcan-chord-worker")));
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
