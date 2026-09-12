//! Backend-neutral native inference boundary.
//!
//! Model-specific DSP owns tensor preparation and result interpretation. This
//! module only selects a backend, validates the file boundary, invokes the
//! pinned ExecuTorch worker, and records bounded runtime measurements.

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const MAX_PROCESS_OUTPUT_BYTES: usize = 256 * 1024;
const DEFAULT_INFERENCE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_ERROR_DETAIL_CHARS: usize = 4 * 1024;
const SESSION_REQUEST_MAGIC: &[u8; 8] = b"SACREQ01";
const SESSION_RESPONSE_MAGIC: &[u8; 8] = b"SACRES01";
const MAX_SESSION_PATH_BYTES: usize = 32 * 1024;
const TENSOR_MAGIC: &[u8; 8] = b"SACTEN01";
const MAX_TENSOR_RANK: usize = 8;
const MAX_TENSOR_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    Mlx,
    Cuda,
}

impl BackendKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mlx => "MLX",
            Self::Cuda => "CUDA",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelFamily {
    ChordRhythm,
    StemSeparation,
}

/// Return a model-specific native fallback order.
///
/// Apple Silicon always has an integrated GPU, so MLX is the production
/// backend. Linux is qualified only with NVIDIA CUDA. No CPU backend can be
/// represented by the production application boundary.
pub fn preferred_backends_for(
    model: ModelFamily,
    operating_system: &str,
    architecture: &str,
    nvidia_available: bool,
) -> Vec<BackendKind> {
    match (model, operating_system, architecture) {
        (_, "macos", "aarch64") => vec![BackendKind::Mlx],
        (_, "linux", "x86_64" | "aarch64") if nvidia_available => {
            vec![BackendKind::Cuda]
        }
        _ => Vec::new(),
    }
}

pub fn preferred_backends(
    operating_system: &str,
    architecture: &str,
    nvidia_available: bool,
) -> Vec<BackendKind> {
    preferred_backends_for(
        ModelFamily::ChordRhythm,
        operating_system,
        architecture,
        nvidia_available,
    )
}

#[derive(Debug)]
pub struct InferenceRequest<'a> {
    pub program: &'a Path,
    pub inputs: &'a [PathBuf],
    pub output_directory: &'a Path,
    pub audio_duration: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct NativeTensor {
    pub dimensions: Vec<usize>,
    pub values: Vec<f32>,
}

pub fn write_tensor_file(
    path: &Path,
    dimensions: &[usize],
    values: &[f32],
) -> Result<(), AppError> {
    validate_tensor_shape(dimensions, values.len())?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(AppError::NativeInference(
            "native tensor input contains a non-finite value".into(),
        ));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| AppError::io(path, error))?;
    file.write_all(TENSOR_MAGIC)
        .and_then(|_| file.write_all(&(dimensions.len() as u32).to_le_bytes()))
        .map_err(|error| AppError::io(path, error))?;
    for &dimension in dimensions {
        file.write_all(&(dimension as u64).to_le_bytes())
            .map_err(|error| AppError::io(path, error))?;
    }
    for value in values {
        file.write_all(&value.to_le_bytes())
            .map_err(|error| AppError::io(path, error))?;
    }
    file.sync_all().map_err(|error| AppError::io(path, error))
}

pub fn read_tensor_file(path: &Path) -> Result<NativeTensor, AppError> {
    validate_regular_file(path, "native tensor")?;
    let bytes = fs::read(path).map_err(|error| AppError::io(path, error))?;
    if bytes.len() < TENSOR_MAGIC.len() + size_of::<u32>()
        || &bytes[..TENSOR_MAGIC.len()] != TENSOR_MAGIC
    {
        return Err(AppError::NativeInference(
            "native tensor has an invalid format".into(),
        ));
    }
    let mut offset = TENSOR_MAGIC.len();
    let rank = read_u32(&bytes, &mut offset)? as usize;
    if rank > MAX_TENSOR_RANK {
        return Err(AppError::NativeInference(
            "native tensor rank exceeds the supported limit".into(),
        ));
    }
    let mut dimensions = Vec::with_capacity(rank);
    for _ in 0..rank {
        let dimension = usize::try_from(read_u64(&bytes, &mut offset)?)
            .map_err(|_| AppError::NativeInference("native tensor dimension overflow".into()))?;
        dimensions.push(dimension);
    }
    let element_count = tensor_element_count(&dimensions)?;
    let payload_bytes = element_count
        .checked_mul(size_of::<f32>())
        .ok_or_else(|| AppError::NativeInference("native tensor size overflow".into()))?;
    if bytes.len() != offset.saturating_add(payload_bytes) {
        return Err(AppError::NativeInference(
            "native tensor payload has an inconsistent size".into(),
        ));
    }
    let mut values = Vec::with_capacity(element_count);
    for chunk in bytes[offset..].chunks_exact(size_of::<f32>()) {
        let value = f32::from_le_bytes(chunk.try_into().expect("four-byte float chunk"));
        if !value.is_finite() {
            return Err(AppError::NativeInference(
                "native tensor output contains a non-finite value".into(),
            ));
        }
        values.push(value);
    }
    Ok(NativeTensor { dimensions, values })
}

fn validate_tensor_shape(dimensions: &[usize], value_count: usize) -> Result<(), AppError> {
    if dimensions.len() > MAX_TENSOR_RANK || tensor_element_count(dimensions)? != value_count {
        return Err(AppError::NativeInference(
            "native tensor has an inconsistent shape".into(),
        ));
    }
    Ok(())
}

fn tensor_element_count(dimensions: &[usize]) -> Result<usize, AppError> {
    let count = dimensions
        .iter()
        .try_fold(1_usize, |count, dimension| count.checked_mul(*dimension));
    match count {
        Some(count) if count <= MAX_TENSOR_BYTES / size_of::<f32>() => Ok(count),
        _ => Err(AppError::NativeInference(
            "native tensor exceeds the 512 MiB limit".into(),
        )),
    }
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, AppError> {
    let value = bytes
        .get(*offset..offset.saturating_add(size_of::<u32>()))
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| AppError::NativeInference("native tensor header is truncated".into()))?;
    *offset += size_of::<u32>();
    Ok(value)
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, AppError> {
    let value = bytes
        .get(*offset..offset.saturating_add(size_of::<u64>()))
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| AppError::NativeInference("native tensor header is truncated".into()))?;
    *offset += size_of::<u64>();
    Ok(value)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceMeasurement {
    pub selected_backend: BackendKind,
    pub output_count: usize,
    pub wall_time_ms: f64,
    pub load_time_ms: f64,
    pub inference_time_ms: f64,
    pub real_time_factor: Option<f64>,
    pub peak_rss_bytes: u64,
    pub runtime_bytes: u64,
    pub program_bytes: u64,
}

pub trait InferenceBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn probe(&self, program: &Path) -> Result<InferenceMeasurement, AppError>;
    fn infer(&self, request: &InferenceRequest<'_>) -> Result<InferenceMeasurement, AppError>;
}

#[derive(Clone, Debug)]
pub struct BackendProgram {
    pub backend: BackendKind,
    pub program: PathBuf,
}

#[derive(Debug)]
pub struct SelectedInferenceBackend {
    pub backend: ExecuTorchBackend,
    pub program: PathBuf,
    pub startup_measurement: InferenceMeasurement,
}

/// Probe backend-specific programs in platform priority order.
///
/// Each probe gets its own process. A delegate abort is therefore contained
/// and does not prevent trying the next program.
pub fn select_first_available(
    worker: &Path,
    priorities: &[BackendKind],
    programs: &[BackendProgram],
    timeout: Duration,
) -> Result<SelectedInferenceBackend, AppError> {
    let mut attempted = Vec::new();
    for priority in priorities {
        let Some(candidate) = programs
            .iter()
            .find(|candidate| candidate.backend == *priority)
        else {
            continue;
        };
        attempted.push(priority.label());
        let backend =
            ExecuTorchBackend::new(*priority, worker.to_path_buf())?.with_timeout(timeout)?;
        if let Ok(startup_measurement) = backend.probe(&candidate.program) {
            return Ok(SelectedInferenceBackend {
                backend,
                program: candidate.program.clone(),
                startup_measurement,
            });
        }
    }
    let attempted = if attempted.is_empty() {
        "none".into()
    } else {
        attempted.join(", ")
    };
    Err(AppError::NativeInference(format!(
        "no native inference backend passed its startup probe (attempted: {attempted})"
    )))
}

#[derive(Clone, Debug)]
pub struct ExecuTorchBackend {
    kind: BackendKind,
    worker: PathBuf,
    timeout: Duration,
    session: Arc<Mutex<Option<WorkerSession>>>,
}

impl ExecuTorchBackend {
    pub fn new(kind: BackendKind, worker: PathBuf) -> Result<Self, AppError> {
        validate_regular_file(&worker, "ExecuTorch worker")?;
        Ok(Self {
            kind,
            worker,
            timeout: DEFAULT_INFERENCE_TIMEOUT,
            session: Arc::new(Mutex::new(None)),
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, AppError> {
        if timeout.is_zero() {
            return Err(AppError::NativeInference(
                "native inference timeout must be positive".into(),
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    fn measurement(
        &self,
        program: &Path,
        response: WorkerResponse,
        elapsed: Duration,
        audio_duration: Option<Duration>,
    ) -> InferenceMeasurement {
        let runtime_bytes = fs::metadata(&self.worker)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let program_bytes = fs::metadata(program)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let real_time_factor = audio_duration
            .filter(|duration| !duration.is_zero())
            .map(|duration| response.inference_time_ms / (duration.as_secs_f64() * 1_000.0));
        InferenceMeasurement {
            selected_backend: self.kind,
            output_count: response.outputs,
            wall_time_ms: elapsed.as_secs_f64() * 1_000.0,
            load_time_ms: response.load_time_ms,
            inference_time_ms: response.inference_time_ms,
            real_time_factor,
            peak_rss_bytes: response.peak_rss_bytes,
            runtime_bytes,
            program_bytes,
        }
    }
}

impl InferenceBackend for ExecuTorchBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn probe(&self, program: &Path) -> Result<InferenceMeasurement, AppError> {
        validate_regular_file(program, "ExecuTorch program")?;
        let started = Instant::now();
        let response = run_worker(
            &self.worker,
            &["self-test".into(), program.as_os_str().into()],
            self.timeout,
        )?;
        let parsed: WorkerResponse = serde_json::from_slice(&response.stdout).map_err(|error| {
            AppError::NativeInference(format!("invalid worker response: {error}"))
        })?;
        Ok(self.measurement(program, parsed, started.elapsed(), None))
    }

    fn infer(&self, request: &InferenceRequest<'_>) -> Result<InferenceMeasurement, AppError> {
        validate_regular_file(request.program, "ExecuTorch program")?;
        if request.inputs.is_empty() || request.inputs.len() > 16 {
            return Err(AppError::NativeInference(
                "native inference requires between one and sixteen tensors".into(),
            ));
        }
        for input in request.inputs {
            validate_regular_file(input, "input tensor")?;
        }
        validate_new_output_directory(request.output_directory)?;

        let mut arguments = vec![
            "infer".into(),
            request.program.as_os_str().into(),
            request.output_directory.as_os_str().into(),
        ];
        arguments.extend(request.inputs.iter().map(|path| path.as_os_str().into()));
        let started = Instant::now();
        let response =
            run_worker_session(&self.worker, &arguments[1..], &self.session, self.timeout)?;
        let parsed: WorkerResponse = serde_json::from_slice(&response.stdout).map_err(|error| {
            AppError::NativeInference(format!("invalid worker response: {error}"))
        })?;
        Ok(self.measurement(
            request.program,
            parsed,
            started.elapsed(),
            request.audio_duration,
        ))
    }
}

#[derive(Deserialize)]
struct WorkerResponse {
    outputs: usize,
    #[serde(rename = "loadTimeMs")]
    load_time_ms: f64,
    #[serde(rename = "inferenceTimeMs")]
    inference_time_ms: f64,
    #[serde(rename = "peakRssBytes")]
    peak_rss_bytes: u64,
}

struct ProcessOutput {
    stdout: Vec<u8>,
}

struct SessionResponse {
    success: bool,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct WorkerSession {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<Result<SessionResponse, String>>,
}

impl Drop for WorkerSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn run_worker_session(
    worker: &Path,
    paths: &[std::ffi::OsString],
    slot: &Arc<Mutex<Option<WorkerSession>>>,
    timeout: Duration,
) -> Result<ProcessOutput, AppError> {
    if paths.len() < 3 || paths.len() > 18 {
        return Err(AppError::NativeInference(
            "native session received an invalid path count".into(),
        ));
    }
    let mut slot = slot
        .lock()
        .map_err(|_| AppError::NativeInference("native worker session is unavailable".into()))?;
    if slot.is_none() {
        *slot = Some(start_worker_session(worker)?);
    }
    let result = (|| {
        let session = slot.as_mut().expect("worker session was initialized");
        session
            .stdin
            .write_all(SESSION_REQUEST_MAGIC)
            .and_then(|_| session.stdin.write_all(&(paths.len() as u32).to_le_bytes()))
            .map_err(|error| AppError::io(worker, error))?;
        for path in paths {
            let bytes = path_bytes(path)?;
            if bytes.is_empty() || bytes.len() > MAX_SESSION_PATH_BYTES || bytes.contains(&0) {
                return Err(AppError::NativeInference(
                    "native session path is outside protocol bounds".into(),
                ));
            }
            session
                .stdin
                .write_all(&(bytes.len() as u32).to_le_bytes())
                .and_then(|_| session.stdin.write_all(bytes))
                .map_err(|error| AppError::io(worker, error))?;
        }
        session
            .stdin
            .flush()
            .map_err(|error| AppError::io(worker, error))?;
        let response = session
            .responses
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => AppError::NativeInference(format!(
                    "native inference exceeded the {:.1} second limit",
                    timeout.as_secs_f64()
                )),
                mpsc::RecvTimeoutError::Disconnected => {
                    AppError::NativeInference("native worker session terminated".into())
                }
            })?;
        let response = response.map_err(AppError::NativeInference)?;
        if !response.success {
            let detail = String::from_utf8_lossy(&response.payload)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            return Err(AppError::NativeInference(
                detail.chars().take(MAX_ERROR_DETAIL_CHARS).collect(),
            ));
        }
        Ok(ProcessOutput {
            stdout: response.payload,
        })
    })();
    if result.is_err() {
        slot.take();
    }
    result
}

#[cfg(unix)]
fn path_bytes(path: &std::ffi::OsStr) -> Result<&[u8], AppError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_bytes())
}

#[cfg(not(unix))]
fn path_bytes(path: &std::ffi::OsStr) -> Result<&[u8], AppError> {
    path.to_str()
        .map(str::as_bytes)
        .ok_or_else(|| AppError::NativeInference("native session path is not UTF-8".into()))
}

fn start_worker_session(worker: &Path) -> Result<WorkerSession, AppError> {
    let mut child = Command::new(worker)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::io(worker, error))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::NativeInference("native worker stdin is unavailable".into()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::NativeInference("native worker stdout is unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::NativeInference("native worker stderr is unavailable".into()))?;
    let (sender, responses) = mpsc::channel();
    thread::spawn(move || loop {
        let response = (|| -> Result<SessionResponse, String> {
            let mut magic = [0_u8; 8];
            stdout
                .read_exact(&mut magic)
                .map_err(|error| error.to_string())?;
            if &magic != SESSION_RESPONSE_MAGIC {
                return Err("native worker returned invalid session framing".into());
            }
            let mut word = [0_u8; 4];
            stdout
                .read_exact(&mut word)
                .map_err(|error| error.to_string())?;
            let success = u32::from_le_bytes(word) == 1;
            stdout
                .read_exact(&mut word)
                .map_err(|error| error.to_string())?;
            let length = u32::from_le_bytes(word) as usize;
            if length > MAX_PROCESS_OUTPUT_BYTES {
                return Err("native worker response exceeded its bound".into());
            }
            let mut payload = vec![0_u8; length];
            stdout
                .read_exact(&mut payload)
                .map_err(|error| error.to_string())?;
            Ok(SessionResponse { success, payload })
        })();
        let failed = response.is_err();
        if sender.send(response).is_err() || failed {
            break;
        }
    });
    thread::spawn(move || {
        let _ = read_bounded(stderr);
    });
    Ok(WorkerSession {
        child,
        stdin,
        responses,
    })
}

fn run_worker(
    worker: &Path,
    arguments: &[std::ffi::OsString],
    timeout: Duration,
) -> Result<ProcessOutput, AppError> {
    let mut child = Command::new(worker)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::io(worker, error))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::NativeInference("native worker stdout is unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::NativeInference("native worker stderr is unavailable".into()))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AppError::io(worker, error))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(AppError::NativeInference(format!(
                "native inference exceeded the {:.1} second limit",
                timeout.as_secs_f64()
            )));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| AppError::NativeInference("native worker stdout reader failed".into()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| AppError::NativeInference("native worker stderr reader failed".into()))??;
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        let detail = detail.split_whitespace().collect::<Vec<_>>().join(" ");
        let detail = detail
            .chars()
            .take(MAX_ERROR_DETAIL_CHARS)
            .collect::<String>();
        return Err(AppError::NativeInference(if detail.is_empty() {
            format!("native worker exited with status {status}")
        } else {
            detail
        }));
    }
    Ok(ProcessOutput { stdout })
}

fn read_bounded(mut source: impl Read) -> Result<Vec<u8>, AppError> {
    let mut kept = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|error| AppError::NativeInference(error.to_string()))?;
        if count == 0 {
            return Ok(kept);
        }
        let remaining = MAX_PROCESS_OUTPUT_BYTES.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..count.min(remaining)]);
    }
}

fn validate_regular_file(path: &Path, description: &str) -> Result<(), AppError> {
    if !path.is_absolute() {
        return Err(AppError::NativeInference(format!(
            "{description} path must be absolute"
        )));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| AppError::io(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::NativeInference(format!(
            "{description} must be a regular, non-symlink file"
        )));
    }
    Ok(())
}

fn validate_new_output_directory(path: &Path) -> Result<(), AppError> {
    if !path.is_absolute() || path.exists() {
        return Err(AppError::NativeInference(
            "output directory must be an absolute path that does not exist".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::NativeInference("output directory has no parent".into()))?;
    let metadata = fs::symlink_metadata(parent).map_err(|error| AppError::io(parent, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::NativeInference(
            "output parent must be a regular, non-symlink directory".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_priority_matches_supported_platforms() {
        assert_eq!(
            preferred_backends("macos", "aarch64", false),
            [BackendKind::Mlx]
        );
        assert_eq!(
            preferred_backends_for(ModelFamily::StemSeparation, "macos", "aarch64", false,),
            [BackendKind::Mlx]
        );
        assert_eq!(
            preferred_backends("linux", "x86_64", true),
            [BackendKind::Cuda]
        );
        assert!(preferred_backends("linux", "aarch64", false).is_empty());
        assert!(preferred_backends("unsupported", "x86_64", true).is_empty());
    }

    #[test]
    fn output_directory_must_be_new_and_have_a_regular_parent() {
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("output");
        assert!(validate_new_output_directory(&output).is_ok());
        fs::create_dir(&output).unwrap();
        assert!(validate_new_output_directory(&output).is_err());
    }

    #[test]
    fn native_tensor_files_round_trip_and_reject_non_finite_values() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("input.tensor");
        write_tensor_file(&path, &[1, 2, 2], &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let tensor = read_tensor_file(&path).unwrap();
        assert_eq!(tensor.dimensions, [1, 2, 2]);
        assert_eq!(tensor.values, [1.0, 2.0, 3.0, 4.0]);
        assert!(
            write_tensor_file(&temporary.path().join("invalid.tensor"), &[1], &[f32::NAN],)
                .is_err()
        );
    }

    #[test]
    fn backend_candidates_follow_priority_not_manifest_order() {
        let programs = [
            BackendProgram {
                backend: BackendKind::Cuda,
                program: PathBuf::from("cuda.pte"),
            },
            BackendProgram {
                backend: BackendKind::Mlx,
                program: PathBuf::from("mlx.pte"),
            },
        ];
        let priorities = preferred_backends("macos", "aarch64", false);
        let selected = priorities
            .iter()
            .find_map(|priority| programs.iter().find(|program| program.backend == *priority))
            .unwrap();
        assert_eq!(selected.backend, BackendKind::Mlx);
    }
}
