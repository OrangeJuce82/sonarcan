//! First-run installation of the pinned analysis models.
//!
//! Model bytes are installed on disk only. Inference workers remain responsible
//! for loading a selected model into memory when its feature is used.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{
    error::AppError,
    inference_backend::{BackendKind, BackendProgram},
    stem_contract::StemSeparationProfile,
};

pub const MODEL_INSTALL_PROGRESS: &str = "model-install-progress";

const PACK_MAGIC: &[u8; 8] = b"SACPKG01";
const MAX_PACK_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PACK_FILES: usize = 4_096;
const MAX_PACK_PATH_BYTES: usize = 4_096;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallProgress {
    model_id: &'static str,
    model_name: &'static str,
    stage: &'static str,
    progress: f64,
    completed_bytes: u64,
    total_bytes: u64,
    model_index: usize,
    model_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallResult {
    installed: bool,
    model_count: usize,
}

struct RemoteModelPack {
    id: &'static str,
    name: &'static str,
    url: &'static str,
    destination: &'static str,
    size: u64,
    sha256: &'static str,
}

const MACOS_PACKS: &[RemoteModelPack] = &[
    RemoteModelPack {
        id: "chord-rhythm-mlx-v1",
        name: "Beat and chord analysis (MLX)",
        url: "https://github.com/OrangeJuce82/sonarcan/releases/download/native-models-v1/sonarcan-macos-chord-rhythm.sacmodels",
        destination: "chord-rhythm",
        size: 204_178_291,
        sha256: "d7727488e99c3b6422cb1833cb2c97f1c76bc3f2f602f41f5e7b1fce02230270",
    },
    RemoteModelPack {
        id: "stem-separation-mlx-v1",
        name: "Four-stem separation (MLX)",
        url: "https://github.com/OrangeJuce82/sonarcan/releases/download/native-models-v1/sonarcan-macos-stem-separation.sacmodels",
        destination: "stem-separation",
        size: 341_114_095,
        sha256: "60f62956d6dfc20e692b4c9ffddd547470c61998dffb126508c0c434c355d07a",
    },
];
// Filled after the CUDA exporter and NVIDIA corpus gate publish the immutable
// x86_64 and arm64 packs. An empty catalog deliberately blocks Linux startup.
const LINUX_NVIDIA_PACKS: &[RemoteModelPack] = &[];

pub fn native_model_root(app: &AppHandle) -> Result<PathBuf, AppError> {
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("SONARCAN_NATIVE_MODEL_ROOT") {
        return Ok(PathBuf::from(root));
    }
    app.path()
        .app_data_dir()
        .map(|root| root.join("models").join("executorch"))
        .map_err(|error| AppError::BackgroundTask(error.to_string()))
}

pub fn beat_programs(app: &AppHandle, _fixed_only: bool) -> Result<Vec<BackendProgram>, AppError> {
    let root = native_model_root(app)?.join("chord-rhythm");
    let mut programs = Vec::new();
    for backend in native_chord_backends() {
        let program = root.join(backend_slug(backend)).join("beat-this-fixed.pte");
        if program.is_file() {
            programs.push(BackendProgram { backend, program });
        }
    }
    Ok(programs)
}

pub fn lv_program_roots(app: &AppHandle) -> Result<Vec<BackendProgram>, AppError> {
    let root = native_model_root(app)?.join("chord-rhythm");
    Ok(native_chord_backends()
        .into_iter()
        .filter_map(|backend| {
            let directory = root.join(backend_slug(backend)).join("lv-chordia");
            let probe = directory.join("lv-chordia-s0-convolution-0.pte");
            probe.is_file().then_some(BackendProgram {
                backend,
                program: probe,
            })
        })
        .collect())
}

pub fn lv_program_root(app: &AppHandle, backend: BackendKind) -> Result<PathBuf, AppError> {
    Ok(native_model_root(app)?
        .join("chord-rhythm")
        .join(backend_slug(backend))
        .join("lv-chordia"))
}

pub fn lv_cqt_kernel(app: &AppHandle, tuning: f32) -> Result<PathBuf, AppError> {
    let index = ((tuning.clamp(-0.5, 0.49) + 0.5) * 100.0).round() as usize;
    Ok(native_model_root(app)?
        .join("chord-rhythm")
        .join("cqt")
        .join(format!("tuning-{index:02}.saccqt")))
}

pub fn stem_program_roots(
    app: &AppHandle,
    profile: StemSeparationProfile,
) -> Result<Vec<BackendProgram>, AppError> {
    let root = native_model_root(app)?.join("stem-separation");
    Ok(native_stem_backends()
        .into_iter()
        .filter_map(|backend| {
            let directory = root.join(backend_slug(backend)).join(profile.argument());
            let probe = directory.join(match profile {
                StemSeparationProfile::Fast => "htdemucs-neural-core.pte",
                StemSeparationProfile::Hq => "scnet-encoder-0.pte",
            });
            probe.is_file().then_some(BackendProgram {
                backend,
                program: probe,
            })
        })
        .collect())
}

pub fn stem_program_root(
    app: &AppHandle,
    backend: BackendKind,
    profile: StemSeparationProfile,
) -> Result<PathBuf, AppError> {
    Ok(native_model_root(app)?
        .join("stem-separation")
        .join(backend_slug(backend))
        .join(profile.argument()))
}

pub const fn backend_slug(backend: BackendKind) -> &'static str {
    match backend {
        BackendKind::Mlx => "mlx",
        BackendKind::Cuda => "cuda",
    }
}

fn native_chord_backends() -> Vec<BackendKind> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        vec![BackendKind::Mlx]
    } else {
        vec![BackendKind::Cuda]
    }
}

fn native_stem_backends() -> Vec<BackendKind> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        vec![BackendKind::Mlx]
    } else {
        vec![BackendKind::Cuda]
    }
}

pub fn prepare(app: &AppHandle) -> Result<ModelInstallResult, AppError> {
    let packs = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        MACOS_PACKS
    } else if cfg!(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        LINUX_NVIDIA_PACKS
    } else {
        &[]
    };
    if packs.is_empty() {
        return Err(AppError::BackgroundTask(
            "no qualified native model catalog exists for this GPU platform".into(),
        ));
    }
    let root = native_model_root(app)?;
    ensure_directory(&root)?;
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .build()
        .map_err(model_error)?;
    let total_bytes = packs.iter().map(|pack| pack.size).sum::<u64>();
    let mut completed = 0;
    let mut installed = false;
    for (index, pack) in packs.iter().enumerate() {
        emit(
            app,
            pack.id,
            pack.name,
            "checking",
            fraction(completed, total_bytes),
            completed,
            total_bytes,
            index + 1,
            packs.len(),
        );
        let destination = safe_destination(&root, pack.destination)?;
        if !verify_installed_pack(&destination, pack.sha256)? {
            download_pack(
                &client,
                app,
                pack,
                &root,
                completed,
                total_bytes,
                index + 1,
                packs.len(),
            )?;
            installed = true;
        }
        completed += pack.size;
        emit(
            app,
            pack.id,
            pack.name,
            "verified",
            fraction(completed, total_bytes),
            completed,
            total_bytes,
            index + 1,
            packs.len(),
        );
    }
    emit(
        app,
        "complete",
        "SonArcan",
        "complete",
        1.0,
        total_bytes,
        total_bytes,
        packs.len(),
        packs.len(),
    );
    Ok(ModelInstallResult {
        installed,
        model_count: packs.len(),
    })
}

#[allow(clippy::too_many_arguments)]
fn download_pack(
    client: &Client,
    app: &AppHandle,
    pack: &RemoteModelPack,
    root: &Path,
    completed_before: u64,
    total_bytes: u64,
    model_index: usize,
    model_count: usize,
) -> Result<(), AppError> {
    let temporary = root.join(format!(".{}.{}.part", pack.id, Uuid::new_v4()));
    let result = (|| {
        let mut response = client
            .get(pack.url)
            .send()
            .map_err(model_error)?
            .error_for_status()
            .map_err(model_error)?;
        let mut output = File::options()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| AppError::io(&temporary, error))?;
        let mut received = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = response.read(&mut buffer).map_err(model_error)?;
            if count == 0 {
                break;
            }
            received = received.saturating_add(count as u64);
            if received > pack.size {
                return Err(AppError::BackgroundTask(format!(
                    "{} download exceeded its pinned size",
                    pack.name
                )));
            }
            output
                .write_all(&buffer[..count])
                .map_err(|error| AppError::io(&temporary, error))?;
            emit(
                app,
                pack.id,
                pack.name,
                "downloading",
                fraction(completed_before + received, total_bytes),
                completed_before + received,
                total_bytes,
                model_index,
                model_count,
            );
        }
        output
            .sync_all()
            .map_err(|error| AppError::io(&temporary, error))?;
        if !verified_file(&temporary, pack.size, pack.sha256)? {
            return Err(AppError::BackgroundTask(format!(
                "{} failed SHA-256 verification",
                pack.name
            )));
        }
        install_pack(&temporary, root, pack.destination, pack.sha256)
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn safe_destination(root: &Path, destination: &str) -> Result<PathBuf, AppError> {
    let relative = Path::new(destination);
    if relative.components().count() != 1
        || !matches!(relative.components().next(), Some(Component::Normal(_)))
    {
        return Err(AppError::BackgroundTask(
            "invalid native model destination".into(),
        ));
    }
    Ok(root.join(relative))
}

fn install_pack(
    pack: &Path,
    root: &Path,
    destination: &str,
    pack_sha256: &str,
) -> Result<(), AppError> {
    let target = safe_destination(root, destination)?;
    let staging = root.join(format!(".{destination}.{}.staging", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|error| AppError::io(&staging, error))?;
    let result = extract_pack(pack, &staging, pack_sha256).and_then(|()| {
        let backup = root.join(format!(".{destination}.{}.backup", Uuid::new_v4()));
        if target.exists() {
            fs::rename(&target, &backup).map_err(|error| AppError::io(&target, error))?;
        }
        if let Err(error) = fs::rename(&staging, &target) {
            if backup.exists() {
                let _ = fs::rename(&backup, &target);
            }
            return Err(AppError::io(&target, error));
        }
        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|error| AppError::io(&backup, error))?;
        }
        Ok(())
    });
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn extract_pack(pack: &Path, destination: &Path, pack_sha256: &str) -> Result<(), AppError> {
    let mut source = File::open(pack).map_err(|error| AppError::io(pack, error))?;
    let mut magic = [0_u8; 8];
    source
        .read_exact(&mut magic)
        .map_err(|error| AppError::io(pack, error))?;
    if &magic != PACK_MAGIC {
        return Err(AppError::BackgroundTask(
            "native model pack has an invalid header".into(),
        ));
    }
    let count = read_pack_u32(&mut source, pack)? as usize;
    if count == 0 || count > MAX_PACK_FILES {
        return Err(AppError::BackgroundTask(
            "native model pack file count is invalid".into(),
        ));
    }
    let mut manifest = format!("{pack_sha256}\n");
    for _ in 0..count {
        let path_length = read_pack_u16(&mut source, pack)? as usize;
        let size = read_pack_u64(&mut source, pack)?;
        if path_length == 0 || path_length > MAX_PACK_PATH_BYTES || size > MAX_PACK_BYTES {
            return Err(AppError::BackgroundTask(
                "native model pack entry exceeds its limit".into(),
            ));
        }
        let mut expected = [0_u8; 32];
        source
            .read_exact(&mut expected)
            .map_err(|error| AppError::io(pack, error))?;
        let mut path_bytes = vec![0_u8; path_length];
        source
            .read_exact(&mut path_bytes)
            .map_err(|error| AppError::io(pack, error))?;
        let relative = std::str::from_utf8(&path_bytes)
            .map_err(|_| AppError::BackgroundTask("native model path is not UTF-8".into()))?;
        let relative_path = Path::new(relative);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(AppError::BackgroundTask(
                "native model pack contains an unsafe path".into(),
            ));
        }
        let target = destination.join(relative_path);
        let parent = target
            .parent()
            .ok_or_else(|| AppError::BackgroundTask("invalid native model path".into()))?;
        ensure_directory(parent)?;
        let mut output = File::options()
            .create_new(true)
            .write(true)
            .open(&target)
            .map_err(|error| AppError::io(&target, error))?;
        let mut limited = (&mut source).take(size);
        let mut digest = Sha256::new();
        let copied = std::io::copy(
            &mut limited,
            &mut DigestWriter {
                output: &mut output,
                digest: &mut digest,
            },
        )
        .map_err(|error| AppError::io(&target, error))?;
        output
            .sync_all()
            .map_err(|error| AppError::io(&target, error))?;
        let actual = digest.finalize();
        if copied != size || actual.as_slice() != expected {
            return Err(AppError::BackgroundTask(format!(
                "native model entry failed verification: {relative}"
            )));
        }
        manifest.push_str(&format!("{}\t{size}\t{relative}\n", hex_digest(&expected)));
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(|error| AppError::io(pack, error))?
        != 0
    {
        return Err(AppError::BackgroundTask(
            "native model pack has trailing data".into(),
        ));
    }
    let marker = destination.join(".sacpack-manifest");
    fs::write(&marker, manifest).map_err(|error| AppError::io(&marker, error))
}

struct DigestWriter<'a> {
    output: &'a mut File,
    digest: &'a mut Sha256,
}
impl Write for DigestWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let count = self.output.write(buffer)?;
        self.digest.update(&buffer[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

fn read_pack_u16(source: &mut File, path: &Path) -> Result<u16, AppError> {
    let mut value = [0; 2];
    source
        .read_exact(&mut value)
        .map_err(|error| AppError::io(path, error))?;
    Ok(u16::from_le_bytes(value))
}
fn read_pack_u32(source: &mut File, path: &Path) -> Result<u32, AppError> {
    let mut value = [0; 4];
    source
        .read_exact(&mut value)
        .map_err(|error| AppError::io(path, error))?;
    Ok(u32::from_le_bytes(value))
}
fn read_pack_u64(source: &mut File, path: &Path) -> Result<u64, AppError> {
    let mut value = [0; 8];
    source
        .read_exact(&mut value)
        .map_err(|error| AppError::io(path, error))?;
    Ok(u64::from_le_bytes(value))
}
fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn verify_installed_pack(destination: &Path, expected_pack: &str) -> Result<bool, AppError> {
    let marker = destination.join(".sacpack-manifest");
    let text = match fs::read_to_string(&marker) {
        Ok(value) if value.len() <= 512 * 1024 => value,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(AppError::io(&marker, error)),
    };
    let mut lines = text.lines();
    if lines.next() != Some(expected_pack) {
        return Ok(false);
    }
    for line in lines {
        let mut fields = line.splitn(3, '\t');
        let (Some(hash), Some(size), Some(relative)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Ok(false);
        };
        let Ok(size) = size.parse::<u64>() else {
            return Ok(false);
        };
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || !verified_file(&destination.join(path), size, hash)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn ensure_directory(path: &Path) -> Result<(), AppError> {
    fs::create_dir_all(path).map_err(|error| AppError::io(path, error))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| AppError::io(path, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::BackgroundTask(format!(
            "invalid model cache directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn verified_file(path: &Path, size: u64, expected: &str) -> Result<bool, AppError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(AppError::io(path, error)),
    };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() != size
    {
        return Ok(false);
    }
    let mut source = File::open(path).map_err(|error| AppError::io(path, error))?;
    let mut digest = Sha256::new();
    std::io::copy(&mut source, &mut digest).map_err(|error| AppError::io(path, error))?;
    Ok(format!("{:x}", digest.finalize()) == expected)
}

#[allow(clippy::too_many_arguments)]
fn emit(
    app: &AppHandle,
    model_id: &'static str,
    model_name: &'static str,
    stage: &'static str,
    progress: f64,
    completed_bytes: u64,
    total_bytes: u64,
    model_index: usize,
    model_count: usize,
) {
    let _ = app.emit(
        MODEL_INSTALL_PROGRESS,
        ModelInstallProgress {
            model_id,
            model_name,
            stage,
            progress: progress.clamp(0.0, 1.0),
            completed_bytes,
            total_bytes,
            model_index,
            model_count,
        },
    );
}

fn fraction(completed: u64, total: u64) -> f64 {
    if total == 0 {
        1.0
    } else {
        completed as f64 / total as f64
    }
}

fn model_error(error: impl std::fmt::Display) -> AppError {
    AppError::BackgroundTask(format!("model installation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_only_regular_files_with_the_pinned_size_and_digest() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("model.bin");
        fs::write(&path, b"SonArcan").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"SonArcan"));
        assert!(verified_file(&path, 8, &digest).unwrap());
        assert!(!verified_file(&path, 7, &digest).unwrap());
        assert!(!verified_file(&path, 8, &"0".repeat(64)).unwrap());
    }

    #[test]
    fn overall_progress_is_bounded() {
        assert_eq!(fraction(0, 10), 0.0);
        assert_eq!(fraction(5, 10), 0.5);
        assert_eq!(fraction(1, 0), 1.0);
    }

    #[test]
    fn extracts_and_rechecks_a_pinned_native_pack() {
        let directory = tempfile::tempdir().unwrap();
        let pack = directory.path().join("models.sacmodels");
        let payload = b"native-pte";
        let relative = b"mlx/model.pte";
        let entry_digest = Sha256::digest(payload);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(PACK_MAGIC);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&(relative.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&entry_digest);
        bytes.extend_from_slice(relative);
        bytes.extend_from_slice(payload);
        fs::write(&pack, bytes).unwrap();
        let pack_digest = format!("{:x}", Sha256::digest(fs::read(&pack).unwrap()));
        let destination = directory.path().join("chord-rhythm");
        fs::create_dir(&destination).unwrap();
        extract_pack(&pack, &destination, &pack_digest).unwrap();
        assert_eq!(
            fs::read(destination.join("mlx/model.pte")).unwrap(),
            payload
        );
        assert!(verify_installed_pack(&destination, &pack_digest).unwrap());
        fs::write(destination.join("mlx/model.pte"), b"damaged").unwrap();
        assert!(!verify_installed_pack(&destination, &pack_digest).unwrap());
    }
}
