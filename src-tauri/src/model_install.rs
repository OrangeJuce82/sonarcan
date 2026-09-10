//! First-run installation of the pinned analysis models.
//!
//! Model bytes are installed on disk only. Inference workers remain responsible
//! for loading a selected model into memory when its feature is used.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{error::AppError, python_runtime};

pub const MODEL_INSTALL_PROGRESS: &str = "model-install-progress";

const SCNET_ID: &str = "scnet-large-starrytong-v1.0.9";
const SCNET_FILE: &str = "SCNet-large_starrytong_fixed.ckpt";
const SCNET_URL: &str = "https://github.com/ZFTurbo/Music-Source-Separation-Training/releases/download/v1.0.9/SCNet-large_starrytong_fixed.ckpt";
const SCNET_SIZE: u64 = 168_852_258;
const SCNET_SHA256: &str = "65900dfa07d6b6e5d784c0f143920200a4bd281d6e78a806c549d0b912d5885e";

const DEMUCS_ID: &str = "htdemucs-v4";
const DEMUCS_FILE: &str = "955717e8-8726e21a.th";
const DEMUCS_URL: &str =
    "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/955717e8-8726e21a.th";
const DEMUCS_SIZE: u64 = 84_141_911;
const DEMUCS_SHA256: &str = "8726e21a993978c7ba086d3872e7608d7d5bfca646ca4aca459ffda844faa8b4";

const BEAT_THIS_FILE: &str = "final0.ckpt";
const BEAT_THIS_URL: &str =
    "https://cloud.cp.jku.at/public.php/dav/files/7ik4RrBKTS273gp/final0.ckpt";
const BEAT_THIS_SIZE: u64 = 81_058_141;
const BEAT_THIS_SHA256: &str = "8c328b45f59d8dd3dff219253ff6a8d6482be57d0133a29140e2febbf8eb8331";

const LV_CHORDIA_FILES: [(&str, u64, &str); 5] = [
    (
        "joint_chord_net_ismir_naive_v1.0_reweight(0.0,10.0)_s0.best.sdict",
        5_746_183,
        "921b42d5d1cf9ce1c0c0e45a74d409b8066e0acec46058ef74e24ee0fb540761",
    ),
    (
        "joint_chord_net_ismir_naive_v1.0_reweight(0.0,10.0)_s1.best.sdict",
        5_746_175,
        "bcb75859e0efa256696cf5da396b320093317b9b1d9560c304f46c25fe1f8b17",
    ),
    (
        "joint_chord_net_ismir_naive_v1.0_reweight(0.0,10.0)_s2.best.sdict",
        5_746_179,
        "acddf85c3fff29954c4877021177d72e2cba9f729ce80c1010f054c477bf3f61",
    ),
    (
        "joint_chord_net_ismir_naive_v1.0_reweight(0.0,10.0)_s3.best.sdict",
        5_746_175,
        "65d81a3ab73435aaaade586981b4cabdf57b8953d76052703e6968c32ef8421c",
    ),
    (
        "joint_chord_net_ismir_naive_v1.0_reweight(0.0,10.0)_s4.best.sdict",
        5_746_227,
        "5ff6b0ec85640e17a09a9b3de68c93fdd45adc24488e8fa9be5715c28d561122",
    ),
];

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

struct RemoteModel {
    id: &'static str,
    name: &'static str,
    url: &'static str,
    file: &'static str,
    size: u64,
    sha256: &'static str,
}

const REMOTE_MODELS: [RemoteModel; 3] = [
    RemoteModel {
        id: SCNET_ID,
        name: "SCNet-large",
        url: SCNET_URL,
        file: SCNET_FILE,
        size: SCNET_SIZE,
        sha256: SCNET_SHA256,
    },
    RemoteModel {
        id: DEMUCS_ID,
        name: "HTDemucs Fast",
        url: DEMUCS_URL,
        file: DEMUCS_FILE,
        size: DEMUCS_SIZE,
        sha256: DEMUCS_SHA256,
    },
    RemoteModel {
        id: "beat-this-final0",
        name: "Beat This!",
        url: BEAT_THIS_URL,
        file: BEAT_THIS_FILE,
        size: BEAT_THIS_SIZE,
        sha256: BEAT_THIS_SHA256,
    },
];

pub fn beat_this_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map(|root| root.join("models").join("beat-this").join(BEAT_THIS_FILE))
        .map_err(|error| AppError::BackgroundTask(error.to_string()))
}

pub fn prepare(app: &AppHandle) -> Result<ModelInstallResult, AppError> {
    if !crate::accelerated_analysis_available() {
        emit(app, "runtime", "Runtime", "complete", 1.0, 0, 0, 1, 1);
        return Ok(ModelInstallResult {
            installed: false,
            model_count: 0,
        });
    }

    let model_root = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?
        .join("models");
    ensure_directory(&model_root)?;
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .build()
        .map_err(model_error)?;
    let total_bytes = REMOTE_MODELS.iter().map(|model| model.size).sum::<u64>();
    let mut completed_bytes = 0_u64;
    let model_count = REMOTE_MODELS.len() + 1;
    let mut installed = false;

    for (index, model) in REMOTE_MODELS.iter().enumerate() {
        let target = if model.id == "beat-this-final0" {
            beat_this_path(app)?
        } else {
            model_root
                .join("stem-separation")
                .join(model.id)
                .join(model.file)
        };
        emit(
            app,
            model.id,
            model.name,
            "checking",
            fraction(completed_bytes, total_bytes),
            completed_bytes,
            total_bytes,
            index + 1,
            model_count,
        );
        if !verified_file(&target, model.size, model.sha256)? {
            download_model(
                &client,
                app,
                model,
                &target,
                completed_bytes,
                total_bytes,
                index + 1,
                model_count,
            )?;
            installed = true;
        }
        completed_bytes += model.size;
        emit(
            app,
            model.id,
            model.name,
            "verified",
            fraction(completed_bytes, total_bytes),
            completed_bytes,
            total_bytes,
            index + 1,
            model_count,
        );
    }

    verify_lv_chordia(app, completed_bytes, total_bytes, model_count)?;
    emit(
        app,
        "complete",
        "SonArcan",
        "complete",
        1.0,
        total_bytes,
        total_bytes,
        model_count,
        model_count,
    );
    Ok(ModelInstallResult {
        installed,
        model_count,
    })
}

fn verify_lv_chordia(
    app: &AppHandle,
    completed: u64,
    total: u64,
    model_count: usize,
) -> Result<(), AppError> {
    emit(
        app,
        "lv-chordia",
        "LV-Chordia",
        "checking",
        fraction(completed, total),
        completed,
        total,
        model_count,
        model_count,
    );
    let root = lv_chordia_model_root().ok_or_else(|| {
        AppError::BackgroundTask("the LV-Chordia runtime location is unavailable".into())
    })?;
    for (file, size, sha256) in LV_CHORDIA_FILES {
        if !verified_file(&root.join(file), size, sha256)? {
            return Err(AppError::BackgroundTask(format!(
                "the verified LV-Chordia model is missing or damaged: {file}"
            )));
        }
    }
    emit(
        app,
        "lv-chordia",
        "LV-Chordia",
        "verified",
        1.0,
        completed,
        total,
        model_count,
        model_count,
    );
    Ok(())
}

fn lv_chordia_model_root() -> Option<PathBuf> {
    let relative = Path::new("python-runtime/runtime/share/lv-chordia/cache_data");
    let configured = python_runtime::resource_path(relative);
    if configured.as_ref().is_some_and(|path| path.is_dir()) {
        return configured;
    }
    #[cfg(debug_assertions)]
    {
        let development = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(relative);
        if development.is_dir() {
            return Some(development);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn download_model(
    client: &Client,
    app: &AppHandle,
    model: &RemoteModel,
    target: &Path,
    completed_before: u64,
    total_bytes: u64,
    model_index: usize,
    model_count: usize,
) -> Result<(), AppError> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::BackgroundTask("invalid model cache path".into()))?;
    ensure_directory(parent)?;
    let temporary = parent.join(format!(".{}.{}.part", model.file, Uuid::new_v4()));
    let result = (|| {
        let mut response = client
            .get(model.url)
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
            if received > model.size {
                return Err(AppError::BackgroundTask(format!(
                    "{} download exceeded its pinned size",
                    model.name
                )));
            }
            output
                .write_all(&buffer[..count])
                .map_err(|error| AppError::io(&temporary, error))?;
            emit(
                app,
                model.id,
                model.name,
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
        if !verified_file(&temporary, model.size, model.sha256)? {
            return Err(AppError::BackgroundTask(format!(
                "{} failed SHA-256 verification",
                model.name
            )));
        }
        if target.exists() {
            fs::remove_file(target).map_err(|error| AppError::io(target, error))?;
        }
        fs::rename(&temporary, target).map_err(|error| AppError::io(target, error))?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
}
