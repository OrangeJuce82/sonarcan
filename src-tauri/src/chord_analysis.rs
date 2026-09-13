//! LV-Chordia process supervision, validation, cancellation, and caching.

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

#[cfg(not(debug_assertions))]
use crate::python_runtime;
use crate::{
    chord_contract::{ChordAnalysis, ChordMode, WorkerAnalysis},
    error::AppError,
};

const CACHE_VERSION: u32 = 16;
const MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Default)]
pub struct ChordAnalysisService {
    generation: AtomicU64,
    worker: Mutex<Option<ResidentWorker>>,
    active_child: Mutex<Option<Arc<Mutex<Child>>>>,
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

struct ResidentWorker {
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Serialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum WorkerRequest<'a> {
    Analyze {
        audio: &'a Path,
        mode: ChordMode,
        #[serde(rename = "includeRhythm")]
        include_rhythm: bool,
    },
    SelfTest,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WorkerResponse {
    Analysis(WorkerAnalysis),
    SelfTest { accelerated: bool, backend: String },
    Error { error: String },
}

impl ChordAnalysisService {
    pub fn begin(&self) -> u64 {
        self.cancel();
        self.generation.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut slot) = self.active_child.lock() {
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
        requested_mode: ChordMode,
    ) -> Result<ChordAnalysis, AppError> {
        ensure_current(&self.generation, generation)?;
        let source = source_identity(media_path)?;
        let cached = load_cached(package_path, track_id, source)?;
        if cached
            .as_ref()
            .is_some_and(|analysis| analysis.modes.contains_key(requested_mode.as_str()))
        {
            return Ok(cached.unwrap());
        }

        let response = self.request(
            app,
            &WorkerRequest::Analyze {
                audio: media_path,
                mode: requested_mode,
                include_rhythm: cached.is_none(),
            },
        )?;
        ensure_current(&self.generation, generation)?;
        let WorkerResponse::Analysis(worker) = response else {
            return Err(invalid_worker_response(response));
        };
        let analysis =
            worker.validate_and_merge(track_id, CACHE_VERSION, requested_mode, cached)?;
        ensure_current(&self.generation, generation)?;
        if analysis.warnings.is_empty() {
            store(package_path, source, &analysis)?;
        }
        Ok(analysis)
    }

    fn request(
        &self,
        app: &AppHandle,
        request: &WorkerRequest<'_>,
    ) -> Result<WorkerResponse, AppError> {
        self.request_with_timeout(app, request, None)
    }

    fn request_with_timeout(
        &self,
        app: &AppHandle,
        request: &WorkerRequest<'_>,
        timeout: Option<Duration>,
    ) -> Result<WorkerResponse, AppError> {
        let mut slot = self
            .worker
            .lock()
            .map_err(|_| AppError::ChordAnalysis("analysis worker state is unavailable".into()))?;
        if slot.is_none() {
            *slot = Some(spawn_worker(app)?);
        }
        let Some(worker) = slot.as_mut() else {
            return Err(AppError::ChordAnalysis(
                "analysis worker could not be initialized".into(),
            ));
        };
        *self.active_child.lock().map_err(|_| {
            AppError::ChordAnalysis("analysis process state is unavailable".into())
        })? = Some(Arc::clone(&worker.child));
        let completed = timeout.map(|duration| {
            let completed = Arc::new(AtomicBool::new(false));
            let watchdog_completed = Arc::clone(&completed);
            let child = Arc::clone(&worker.child);
            std::thread::spawn(move || {
                std::thread::sleep(duration);
                if !watchdog_completed.load(Ordering::Acquire) {
                    if let Ok(mut child) = child.lock() {
                        let _ = child.kill();
                    }
                }
            });
            completed
        });
        let result = worker.send(request);
        if let Some(completed) = completed {
            completed.store(true, Ordering::Release);
        }
        clear_active_child(&self.active_child, &worker.child);
        if result.is_err() {
            *slot = None;
        }
        result
    }

    pub fn accelerator_self_test(&self, app: &AppHandle) -> bool {
        matches!(
            self.request_with_timeout(
                app,
                &WorkerRequest::SelfTest,
                Some(Duration::from_secs(30)),
            ),
            Ok(WorkerResponse::SelfTest { accelerated: true, backend }) if backend == "MPS"
        )
    }

    pub fn shutdown(&self) {
        self.cancel();
        if let Ok(mut worker) = self.worker.lock() {
            *worker = None;
        }
    }
}

impl ResidentWorker {
    fn send(&mut self, request: &WorkerRequest<'_>) -> Result<WorkerResponse, AppError> {
        serde_json::to_writer(&mut self.stdin, request)?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|error| {
                AppError::ChordAnalysis(format!("could not write to LV-Chordia worker: {error}"))
            })?;
        let bytes = read_bounded_line(&mut self.stdout, MAX_STDOUT_BYTES).map_err(|error| {
            AppError::ChordAnalysis(format!("could not read LV-Chordia worker: {error}"))
        })?;
        if bytes.is_empty() {
            return Err(AppError::ChordAnalysis(
                "LV-Chordia worker stopped unexpectedly".into(),
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| AppError::ChordAnalysis(format!("invalid LV-Chordia JSON: {error}")))
    }
}

impl Drop for ResidentWorker {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn read_bounded_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let read = {
        let mut limited = std::io::Read::take(&mut *reader, (limit + 1) as u64);
        limited.read_until(b'\n', &mut bytes)?
    };
    if read > limit || (!bytes.ends_with(b"\n") && read != 0) {
        let mut discarded = Vec::new();
        reader.read_until(b'\n', &mut discarded)?;
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "worker response exceeded 8 MiB",
        ));
    }
    Ok(bytes)
}

fn clear_active_child(slot: &Mutex<Option<Arc<Mutex<Child>>>>, completed: &Arc<Mutex<Child>>) {
    if let Ok(mut slot) = slot.lock() {
        if slot
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, completed))
        {
            *slot = None;
        }
    }
}

fn invalid_worker_response(response: WorkerResponse) -> AppError {
    match response {
        WorkerResponse::Error { error }
            if error.len() <= 512 && !error.is_empty() && !error.chars().any(char::is_control) =>
        {
            AppError::ChordAnalysis(format!("LV-Chordia failed: {error}"))
        }
        _ => AppError::ChordAnalysis("invalid response from LV-Chordia worker".into()),
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
    let path = crate::model_install::beat_this_path(app)?;
    path.is_file().then_some(path).ok_or_else(|| {
        AppError::ChordAnalysis("the verified Beat This! model is not installed".into())
    })
}

fn spawn_worker(app: &AppHandle) -> Result<ResidentWorker, AppError> {
    let worker = resolve_worker(app)?;
    let downbeat_model = resolve_downbeat_model(app)?;
    let mut child = Command::new(&worker.executable)
        .args(&worker.prefix_arguments)
        .arg("--serve")
        .arg("--device")
        .arg("mps")
        .arg("--downbeat-model")
        .arg(downbeat_model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            AppError::ChordAnalysis(format!("could not start LV-Chordia worker: {error}"))
        })?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::ChordAnalysis("LV-Chordia stdin is unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::ChordAnalysis("LV-Chordia stdout is unavailable".into()))?;
    Ok(ResidentWorker {
        child: Arc::new(Mutex::new(child)),
        stdin,
        stdout: BufReader::new(stdout),
    })
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
    fn bounded_line_reader_rejects_an_oversized_message() {
        let mut input = &b"abcdef\nnext\n"[..];
        assert!(read_bounded_line(&mut input, 4).is_err());
        assert_eq!(read_bounded_line(&mut input, 5).unwrap(), b"next\n");
    }
}
