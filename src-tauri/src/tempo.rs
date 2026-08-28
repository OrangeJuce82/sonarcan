use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{audio_engine::DecodedAudio, error::AppError};

const CACHE_VERSION: u32 = 1;
const MIN_BPM: f64 = 60.0;
const MAX_BPM: f64 = 200.0;
const HOP_FRAMES: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TempoAnalysis {
    pub cache_version: u32,
    pub track_id: Uuid,
    pub bpm: Option<f64>,
    pub confidence: f64,
}

pub fn load_cached(package_path: &Path, track_id: Uuid) -> Option<TempoAnalysis> {
    let bytes = fs::read(cache_path(package_path, track_id)).ok()?;
    let cached = serde_json::from_slice::<TempoAnalysis>(&bytes).ok()?;
    (cached.cache_version == CACHE_VERSION && cached.track_id == track_id).then_some(cached)
}

pub fn analyze_and_store_from_decoded(
    package_path: &Path,
    track_id: Uuid,
    audio: &DecodedAudio,
) -> Result<TempoAnalysis, AppError> {
    if let Some(cached) = load_cached(package_path, track_id) {
        return Ok(cached);
    }
    let mut envelope = Vec::with_capacity(audio.frames / HOP_FRAMES + 1);
    for frames in audio.samples.chunks(audio.channels * HOP_FRAMES) {
        let frame_count = frames.len() / audio.channels;
        if frame_count == 0 {
            continue;
        }
        let energy = frames
            .chunks(audio.channels)
            .map(|frame| {
                let mono =
                    frame.iter().map(|value| *value as f64).sum::<f64>() / audio.channels as f64;
                mono * mono
            })
            .sum::<f64>();
        envelope.push((energy / frame_count as f64).sqrt());
    }
    let (bpm, confidence) = detect_bpm(&envelope, audio.sample_rate);
    let analysis = TempoAnalysis {
        cache_version: CACHE_VERSION,
        track_id,
        bpm,
        confidence,
    };
    store(package_path, &analysis)?;
    Ok(analysis)
}

fn store(package_path: &Path, analysis: &TempoAnalysis) -> Result<(), AppError> {
    let cache_path = cache_path(package_path, analysis.track_id);
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    let temporary = cache_path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec(analysis)?)
        .map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &cache_path).map_err(|error| AppError::io(&cache_path, error))?;
    Ok(())
}

fn cache_path(package_path: &Path, track_id: Uuid) -> std::path::PathBuf {
    package_path
        .join("Analysis")
        .join("tempo")
        .join(format!("{track_id}.json"))
}

fn detect_bpm(energy: &[f64], sample_rate: u32) -> (Option<f64>, f64) {
    if energy.len() < 64 {
        return (None, 0.0);
    }
    let mut onset = Vec::with_capacity(energy.len());
    onset.push(0.0);
    onset.extend(energy.windows(2).map(|pair| (pair[1] - pair[0]).max(0.0)));
    let mean = onset.iter().sum::<f64>() / onset.len() as f64;
    for value in &mut onset {
        *value = (*value - mean).max(0.0);
    }
    let envelope_rate = sample_rate as f64 / HOP_FRAMES as f64;
    let min_lag = (envelope_rate * 60.0 / MAX_BPM).floor().max(1.0) as usize;
    let max_lag = (envelope_rate * 60.0 / MIN_BPM).ceil() as usize;
    let mut best_lag = 0;
    let mut best_score = 0.0;
    let mut total_score = 0.0;
    for lag in min_lag..=max_lag.min(onset.len() / 2) {
        let score = normalized_correlation(&onset, lag)
            + 0.35 * normalized_correlation(&onset, lag * 2)
            + if lag >= 2 {
                0.2 * normalized_correlation(&onset, lag / 2)
            } else {
                0.0
            };
        total_score += score.max(0.0);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best_score < 0.08 {
        return (None, 0.0);
    }
    let mut bpm = 60.0 * envelope_rate / best_lag as f64;
    // Autocorrelation cannot intrinsically distinguish a beat from its bar-level
    // half-time pulse. Prefer the conventional practice range while retaining
    // the wider search interval for genuinely slow or fast material.
    if bpm < 90.0 && bpm * 2.0 <= MAX_BPM {
        bpm *= 2.0;
    }
    let candidates = (max_lag.saturating_sub(min_lag) + 1).max(1) as f64;
    let average = total_score / candidates;
    let confidence = ((best_score - average) / best_score.max(f64::EPSILON)).clamp(0.0, 1.0);
    (Some((bpm * 10.0).round() / 10.0), confidence)
}

fn normalized_correlation(signal: &[f64], lag: usize) -> f64 {
    if lag == 0 || lag >= signal.len() {
        return 0.0;
    }
    let mut numerator = 0.0;
    let mut left_energy = 0.0;
    let mut right_energy = 0.0;
    for index in lag..signal.len() {
        let left = signal[index];
        let right = signal[index - lag];
        numerator += left * right;
        left_energy += left * left;
        right_energy += right * right;
    }
    numerator / (left_energy * right_energy).sqrt().max(f64::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_regular_120_bpm_pulse_train() {
        let sample_rate = 48_000;
        let envelope_rate = sample_rate as f64 / HOP_FRAMES as f64;
        let pulse_interval = (envelope_rate * 0.5).round() as usize;
        let mut energy = vec![0.01; pulse_interval * 32];
        for index in (0..energy.len()).step_by(pulse_interval) {
            energy[index] = 1.0;
        }
        let (bpm, confidence) = detect_bpm(&energy, sample_rate);
        assert!(
            (bpm.unwrap() - 120.0).abs() < 3.0,
            "detected {bpm:?} with confidence {confidence}"
        );
        assert!(confidence > 0.1);
    }
}
