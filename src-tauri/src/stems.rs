//! Optional, local HTDemucs stem separation and project-scoped cache management.
//!
//! Inference never runs on the CPAL callback. The callback only receives an
//! immutable, sample-aligned `StemSet` after the complete cache is committed.

use std::{
    any::Any,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};

use burn::backend::wgpu::{
    graphics::AutoGraphicsApi, init_setup, RuntimeOptions, Wgpu, WgpuDevice,
};
use burn::backend::NdArray;
use demucs_core::{
    listener::{ForwardEvent, ForwardListener},
    model::metadata::{download_url, StemId, HTDEMUCS},
    Demucs, ModelOptions,
};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    audio_engine::{AudioEngine, DecodedAudio},
    error::AppError,
    project,
};

const CACHE_VERSION: u32 = 1;
const MODEL_REVISION: &str = "htdemucs-955717e8";
const STEM_MAGIC: &[u8; 8] = b"SACSTM01";
const STEM_NAMES: [&str; 4] = ["vocals", "drums", "bass", "other"];

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
    Downloading,
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
        engine.disable_stems();
        if let Ok(mut status) = self.status.lock() {
            *status = StemStatus::default();
        }
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
        set_status(
            &self.status,
            StemStatus {
                state: StemState::Separating,
                progress: 0.0,
                stage: "checkingCache".into(),
                track_id: Some(track_id),
                cached: false,
                error: None,
                compute_backend: None,
            },
        );
        let status = Arc::clone(&self.status);
        let running = Arc::clone(&self.running);
        std::thread::Builder::new()
            .name("sonarcan-htdemucs".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                info!(%track_id, project = %package_path.display(), "stem worker started");
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    separate_or_load(&app, &package_path, track_id, &status)
                }))
                .unwrap_or_else(|payload| {
                    Err(AppError::StemSeparation(format!(
                        "HTDemucs worker panicked: {}",
                        panic_message(payload.as_ref())
                    )))
                });
                if let Err(error) = result {
                    warn!(%track_id, %error, "stem separation failed");
                    set_status(
                        &status,
                        StemStatus {
                            state: StemState::Failed,
                            progress: 0.0,
                            stage: "failed".into(),
                            track_id: Some(track_id),
                            cached: false,
                            error: Some(error.to_string()),
                            compute_backend: None,
                        },
                    );
                }
                info!(%track_id, "stem worker stopped");
                running.store(false, Ordering::Release);
            })
            .map_err(|error| {
                self.running.store(false, Ordering::Release);
                AppError::StemSeparation(error.to_string())
            })?;
        Ok(())
    }
}

fn separate_or_load(
    app: &AppHandle,
    package_path: &Path,
    track_id: Uuid,
    status: &Arc<Mutex<StemStatus>>,
) -> Result<(), AppError> {
    info!(%track_id, "resolving stem source audio");
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
        info!(%track_id, "loading stems from project cache");
        app.state::<AudioEngine>()
            .activate_stems(&media_path, stems)?;
        set_status(
            status,
            StemStatus {
                state: StemState::Ready,
                progress: 1.0,
                stage: "ready".into(),
                track_id: Some(track_id),
                cached: true,
                error: None,
                compute_backend: None,
            },
        );
        return Ok(());
    }

    let decoded = app
        .state::<AudioEngine>()
        .decoded_for_analysis(&media_path)?;
    info!(%track_id, frames = decoded.frames, sample_rate = decoded.sample_rate, "source decoded for stem separation");
    let (left, right) = stereo_channels(&decoded);
    let weights = load_or_download_model(status, track_id)?;
    info!(%track_id, model_bytes = weights.len(), "HTDemucs model loaded");
    set_status(
        status,
        StemStatus {
            state: StemState::Separating,
            progress: 0.08,
            stage: "loadingModel".into(),
            track_id: Some(track_id),
            cached: false,
            error: None,
            compute_backend: Some("GPU".into()),
        },
    );
    let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        separate_with_gpu(
            &weights,
            &left,
            &right,
            decoded.sample_rate,
            status,
            track_id,
        )
    }));
    let (separated, compute_backend) = match gpu_result {
        Ok(Ok(stems)) => (stems, "GPU"),
        Ok(Err(error)) => return Err(error),
        Err(payload) => {
            let reason = panic_message(payload.as_ref());
            warn!(%track_id, %reason, "HTDemucs GPU inference is incompatible; retrying on CPU");
            set_status(
                status,
                StemStatus {
                    state: StemState::Separating,
                    progress: 0.08,
                    stage: "cpuFallback".into(),
                    track_id: Some(track_id),
                    cached: false,
                    error: None,
                    compute_backend: Some("CPU".into()),
                },
            );
            let stems = separate_with_cpu(
                &weights,
                &left,
                &right,
                decoded.sample_rate,
                status,
                track_id,
            )?;
            (stems, "CPU")
        }
    };
    info!(%track_id, stem_count = separated.len(), "HTDemucs inference completed");

    let mut ordered: [Option<Arc<DecodedAudio>>; 4] = std::array::from_fn(|_| None);
    for stem in separated {
        let index = match stem.id {
            StemId::Vocals => 0,
            StemId::Drums => 1,
            StemId::Bass => 2,
            StemId::Other => 3,
            _ => continue,
        };
        let available = stem.left.len().min(stem.right.len()).min(decoded.frames);
        let mut samples = vec![0.0; decoded.frames * 2];
        for frame in 0..available {
            samples[frame * 2] = stem.left[frame];
            samples[frame * 2 + 1] = stem.right[frame];
        }
        ordered[index] = Some(Arc::new(DecodedAudio {
            samples,
            channels: 2,
            sample_rate: decoded.sample_rate,
            frames: decoded.frames,
        }));
    }
    let stems: [Arc<DecodedAudio>; 4] = ordered
        .into_iter()
        .map(|stem| {
            stem.ok_or_else(|| {
                AppError::StemSeparation("HTDemucs returned an incomplete stem set".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| AppError::StemSeparation("invalid stem count".into()))?;
    store_cache(
        package_path,
        track_id,
        metadata.len(),
        modified_ns(&metadata),
        &stems,
    )?;
    app.state::<AudioEngine>()
        .activate_stems(&media_path, stems)?;
    info!(%track_id, "HTDemucs stem cache is ready");
    set_status(
        status,
        StemStatus {
            state: StemState::Ready,
            progress: 1.0,
            stage: "ready".into(),
            track_id: Some(track_id),
            cached: false,
            error: None,
            compute_backend: Some(compute_backend.into()),
        },
    );
    Ok(())
}

fn separate_with_gpu(
    weights: &[u8],
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    status: &Arc<Mutex<StemStatus>>,
    track_id: Uuid,
) -> Result<Vec<demucs_core::Stem>, AppError> {
    let model = Demucs::<Wgpu>::from_bytes(ModelOptions::FourStem, weights, gpu_device())
        .map_err(|error| AppError::StemSeparation(error.to_string()))?;
    info!(%track_id, "HTDemucs GPU model initialized");
    run_inference(model, left, right, sample_rate, status, track_id, "GPU")
}

fn separate_with_cpu(
    weights: &[u8],
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    status: &Arc<Mutex<StemStatus>>,
    track_id: Uuid,
) -> Result<Vec<demucs_core::Stem>, AppError> {
    let model =
        Demucs::<NdArray<f32>>::from_bytes(ModelOptions::FourStem, weights, Default::default())
            .map_err(|error| AppError::StemSeparation(error.to_string()))?;
    info!(%track_id, "HTDemucs CPU fallback model initialized");
    run_inference(model, left, right, sample_rate, status, track_id, "CPU")
}

fn run_inference<B: burn::prelude::Backend>(
    model: Demucs<B>,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    status: &Arc<Mutex<StemStatus>>,
    track_id: Uuid,
    compute_backend: &'static str,
) -> Result<Vec<demucs_core::Stem>, AppError> {
    let mut listener = ProgressListener {
        status: Arc::clone(status),
        track_id,
        completed_chunks: 0,
        total_chunks: 1,
        compute_backend,
    };
    pollster::block_on(model.separate_with_listener(left, right, sample_rate, &mut listener))
        .map_err(|error| AppError::StemSeparation(error.to_string()))
}

struct ProgressListener {
    status: Arc<Mutex<StemStatus>>,
    track_id: Uuid,
    completed_chunks: usize,
    total_chunks: usize,
    compute_backend: &'static str,
}
impl ForwardListener for ProgressListener {
    fn on_event(&mut self, event: ForwardEvent) {
        match event {
            ForwardEvent::ChunkStarted { total, .. } => self.total_chunks = total.max(1),
            ForwardEvent::ChunkDone { index, total } => {
                self.completed_chunks = index + 1;
                self.total_chunks = total.max(1);
            }
            _ => return,
        }
        let progress = 0.12 + 0.82 * self.completed_chunks as f32 / self.total_chunks as f32;
        set_status(
            &self.status,
            StemStatus {
                state: StemState::Separating,
                progress,
                stage: "separating".into(),
                track_id: Some(self.track_id),
                cached: false,
                error: None,
                compute_backend: Some(self.compute_backend.into()),
            },
        );
    }
}

fn load_or_download_model(
    status: &Arc<Mutex<StemStatus>>,
    track_id: Uuid,
) -> Result<Vec<u8>, AppError> {
    let directory = ProjectDirs::from("com", "SonArcan", "SonArcan")
        .ok_or_else(|| AppError::StemSeparation("model cache directory is unavailable".into()))?
        .cache_dir()
        .join("models");
    fs::create_dir_all(&directory).map_err(|error| AppError::io(&directory, error))?;
    let path = directory.join(HTDEMUCS.filename);
    if path.is_file() {
        info!(path = %path.display(), "using cached HTDemucs model");
        return fs::read(&path).map_err(|error| AppError::io(&path, error));
    }
    set_status(
        status,
        StemStatus {
            state: StemState::Downloading,
            progress: 0.0,
            stage: "downloadingModel".into(),
            track_id: Some(track_id),
            cached: false,
            error: None,
            compute_backend: None,
        },
    );
    let model_url = download_url(&HTDEMUCS);
    info!(url = %model_url, path = %path.display(), "downloading HTDemucs model");
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(15 * 60))
        .build()
        .map_err(|error| AppError::StemSeparation(error.to_string()))?;
    let mut response = client
        .get(model_url)
        .send()
        .map_err(|error| AppError::StemSeparation(error.to_string()))?
        .error_for_status()
        .map_err(|error| AppError::StemSeparation(error.to_string()))?;
    let total = response
        .content_length()
        .unwrap_or(HTDEMUCS.size_mb as u64 * 1024 * 1024)
        .max(1);
    let mut data = Vec::with_capacity(total as usize);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(|error| AppError::StemSeparation(error.to_string()))?;
        if count == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..count]);
        set_status(
            status,
            StemStatus {
                state: StemState::Downloading,
                progress: 0.08 * data.len() as f32 / total as f32,
                stage: "downloadingModel".into(),
                track_id: Some(track_id),
                cached: false,
                error: None,
                compute_backend: None,
            },
        );
    }
    let temporary = path.with_extension("safetensors.tmp");
    fs::write(&temporary, &data).map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
    info!(path = %path.display(), bytes = data.len(), "HTDemucs model cached");
    Ok(data)
}

fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

fn gpu_device() -> WgpuDevice {
    static DEVICE: OnceLock<WgpuDevice> = OnceLock::new();
    DEVICE
        .get_or_init(|| {
            let device = WgpuDevice::default();
            init_setup::<AutoGraphicsApi>(
                &device,
                RuntimeOptions {
                    tasks_max: 128,
                    ..Default::default()
                },
            );
            info!("HTDemucs GPU runtime initialized");
            device
        })
        .clone()
}

fn stereo_channels(audio: &DecodedAudio) -> (Vec<f32>, Vec<f32>) {
    let mut left = Vec::with_capacity(audio.frames);
    let mut right = Vec::with_capacity(audio.frames);
    for frame in 0..audio.frames {
        left.push(audio.samples[frame * audio.channels]);
        right.push(audio.samples[frame * audio.channels + 1.min(audio.channels - 1)]);
    }
    (left, right)
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
    stems: &[Arc<DecodedAudio>; 4],
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
) -> Option<[Arc<DecodedAudio>; 4]> {
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
    let mut loaded = Vec::new();
    for name in STEM_NAMES {
        let mut file = File::open(directory.join(format!("{name}.pcm"))).ok()?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic).ok()?;
        if &magic != STEM_MAGIC {
            return None;
        }
        let mut u32_buffer = [0_u8; 4];
        let mut u64_buffer = [0_u8; 8];
        file.read_exact(&mut u32_buffer).ok()?;
        file.read_exact(&mut u64_buffer).ok()?;
        let sample_rate = u32::from_le_bytes(u32_buffer);
        let frames = u64::from_le_bytes(u64_buffer) as usize;
        if sample_rate != manifest.sample_rate || frames != manifest.frames {
            return None;
        }
        let mut bytes = vec![0_u8; frames * 2 * 4];
        file.read_exact(&mut bytes).ok()?;
        let samples = bytes
            .chunks_exact(4)
            .map(|value| f32::from_le_bytes([value[0], value[1], value[2], value[3]]))
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
    fn stem_cache_round_trips_and_rejects_a_changed_source() {
        let project = tempfile::tempdir().unwrap();
        let track_id = Uuid::new_v4();
        let stems: [Arc<DecodedAudio>; 4] = std::array::from_fn(|index| {
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
        assert_eq!(loaded[3].samples, stems[3].samples);
        assert!(load_cache(project.path(), track_id, 124, 456).is_none());
    }
}
