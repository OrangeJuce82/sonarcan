use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{audio_engine::DecodedAudio, error::AppError};

const CACHE_VERSION: u32 = 1;
const TARGET_BUCKETS: usize = 32_768;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaveformPeak {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaveformData {
    pub cache_version: u32,
    pub track_id: Uuid,
    pub duration_seconds: f64,
    pub peaks: Vec<WaveformPeak>,
}

pub fn load_cached(package_path: &Path, track_id: Uuid) -> Option<WaveformData> {
    let cache_path = cache_path(package_path, track_id);
    let bytes = fs::read(cache_path).ok()?;
    let cached = serde_json::from_slice::<WaveformData>(&bytes).ok()?;
    (cached.cache_version == CACHE_VERSION && cached.track_id == track_id).then_some(cached)
}

pub fn generate_and_store_from_decoded(
    package_path: &Path,
    track_id: Uuid,
    audio: &DecodedAudio,
) -> Result<WaveformData, AppError> {
    if let Some(cached) = load_cached(package_path, track_id) {
        return Ok(cached);
    }
    let frames_per_bucket = audio.frames.div_ceil(TARGET_BUCKETS).max(1);
    let peaks = audio
        .samples
        .chunks(audio.channels * frames_per_bucket)
        .map(|samples| WaveformPeak {
            min: samples.iter().copied().fold(1.0, f32::min),
            max: samples.iter().copied().fold(-1.0, f32::max),
        })
        .collect();
    let waveform = WaveformData {
        cache_version: CACHE_VERSION,
        track_id,
        duration_seconds: audio.frames as f64 / audio.sample_rate as f64,
        peaks,
    };
    store(package_path, &waveform)?;
    Ok(waveform)
}

fn store(package_path: &Path, waveform: &WaveformData) -> Result<(), AppError> {
    let cache_path = cache_path(package_path, waveform.track_id);
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    let temporary = cache_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(&waveform)?;
    fs::write(&temporary, bytes).map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &cache_path).map_err(|error| AppError::io(&cache_path, error))?;
    Ok(())
}

fn cache_path(package_path: &Path, track_id: Uuid) -> std::path::PathBuf {
    package_path
        .join("Analysis")
        .join("waveform")
        .join(format!("{track_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_and_reuses_a_project_waveform_cache() {
        let temp = tempfile::tempdir().unwrap();
        let package_path = temp.path().join("Waveform Test.sac");
        let track_id = Uuid::new_v4();
        let samples = (0..8_000)
            .map(|index| if index % 100 < 50 { 0.5 } else { -0.5 })
            .collect();
        let audio = DecodedAudio {
            samples,
            channels: 1,
            sample_rate: 8_000,
            frames: 8_000,
        };

        let generated = generate_and_store_from_decoded(&package_path, track_id, &audio).unwrap();
        let cached = generate_and_store_from_decoded(&package_path, track_id, &audio).unwrap();

        assert!(!generated.peaks.is_empty());
        assert_eq!(generated, cached);
        assert!(package_path
            .join("Analysis/waveform")
            .join(format!("{track_id}.json"))
            .is_file());
    }
}
