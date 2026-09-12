//! Native execution of LV-Chordia's pinned Librosa hybrid-CQT kernels.

use std::{fs, path::Path};

use rustfft::{num_complex::Complex32, FftPlanner};
use serde::Deserialize;

use crate::{beat_preprocessing::resample_mono_to, error::AppError};

const MAGIC: &[u8; 8] = b"SACCQT01";
const MAX_KERNEL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CQT_FRAMES: usize = 200_000;
const TUNING_FFT: usize = 2_048;
const TUNING_HOP: usize = 512;

#[derive(Clone, Debug)]
pub struct LvCqt {
    pub frames: usize,
    /// Frame-major values with `frames * model_bins` entries.
    pub values: Vec<f32>,
}

/// Reproduce Librosa's default `estimate_tuning(..., bins_per_octave=36)`
/// grid closely enough to select the exact pre-exported kernel bank entry.
pub fn estimate_tuning_36(signal: &[f32]) -> Result<f32, AppError> {
    if signal.is_empty() || signal.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid("LV-Chordia tuning requires finite non-empty audio"));
    }
    let frames = signal.len() / TUNING_HOP + 1;
    if frames > MAX_CQT_FRAMES {
        return Err(invalid("LV-Chordia tuning frame count exceeds its bound"));
    }
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(TUNING_FFT);
    let mut spectrum = vec![Complex32::new(0.0, 0.0); TUNING_FFT];
    let mut peaks = Vec::<(f64, f32)>::new();
    let first_bin = (150 * TUNING_FFT).div_ceil(22_050);
    let last_bin = (4_000 * TUNING_FFT)
        .div_ceil(22_050)
        .min(TUNING_FFT / 2 + 1);
    for frame in 0..frames {
        let center = frame * TUNING_HOP;
        for (index, value) in spectrum.iter_mut().enumerate() {
            let position = center as isize + index as isize - (TUNING_FFT / 2) as isize;
            let sample = usize::try_from(position)
                .ok()
                .and_then(|position| signal.get(position))
                .copied()
                .unwrap_or(0.0);
            let window =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * index as f32 / TUNING_FFT as f32).cos();
            *value = Complex32::new(sample * window, 0.0);
        }
        fft.process(&mut spectrum);
        let magnitudes = spectrum[..=TUNING_FFT / 2]
            .iter()
            .map(|value| value.norm())
            .collect::<Vec<_>>();
        let maximum = magnitudes.iter().copied().fold(0.0_f32, f32::max);
        for bin in first_bin.max(1)..last_bin.min(magnitudes.len() - 1) {
            let magnitude = magnitudes[bin];
            if magnitude <= 0.1 * maximum
                || magnitude <= magnitudes[bin - 1]
                || magnitude < magnitudes[bin + 1]
            {
                continue;
            }
            let left = magnitudes[bin - 1];
            let right = magnitudes[bin + 1];
            let a = right + left - 2.0 * magnitude;
            let b = (right - left) / 2.0;
            let shift = if b.abs() < a.abs() { -b / a } else { 0.0 };
            let interpolated_magnitude = magnitude + 0.5 * ((right - left) / 2.0) * shift;
            let frequency = (bin as f64 + f64::from(shift)) * 22_050.0 / TUNING_FFT as f64;
            peaks.push((frequency, interpolated_magnitude));
        }
    }
    if peaks.is_empty() {
        return Ok(0.0);
    }
    let mut magnitudes = peaks.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    magnitudes.sort_by(f32::total_cmp);
    let median = if magnitudes.len() % 2 == 0 {
        (magnitudes[magnitudes.len() / 2 - 1] + magnitudes[magnitudes.len() / 2]) / 2.0
    } else {
        magnitudes[magnitudes.len() / 2]
    };
    let mut histogram = [0_usize; 100];
    for (frequency, magnitude) in peaks {
        if magnitude < median {
            continue;
        }
        let mut residual = (36.0 * (frequency / 440.0).log2()).rem_euclid(1.0);
        if residual >= 0.5 {
            residual -= 1.0;
        }
        let bin = (((residual + 0.5) / 0.01).floor() as usize).min(99);
        histogram[bin] += 1;
    }
    let selected = histogram
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(index, _)| index)
        .unwrap_or(50);
    Ok(-0.5 + selected as f32 * 0.01)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KernelMetadata {
    sample_rate: u32,
    hop_length: usize,
    first_model_bin: usize,
    model_bins: usize,
    tuning: f32,
    stages: Vec<KernelStage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KernelStage {
    kind: StageKind,
    rate_divisor: u32,
    hop_length: usize,
    fft_size: usize,
    bins: Vec<usize>,
    #[serde(default)]
    value_offset: usize,
    #[serde(default)]
    value_count: usize,
    #[serde(default)]
    real_offset: usize,
    #[serde(default)]
    real_count: usize,
    #[serde(default)]
    imag_offset: usize,
    #[serde(default)]
    imag_count: usize,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum StageKind {
    Magnitude,
    Complex,
}

pub fn hybrid_cqt_22050(signal: &[f32], kernels: &Path) -> Result<LvCqt, AppError> {
    if signal.is_empty() || signal.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid(
            "LV-Chordia CQT requires finite non-empty mono audio",
        ));
    }
    let metadata = fs::symlink_metadata(kernels).map_err(|error| AppError::io(kernels, error))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_KERNEL_BYTES
    {
        return Err(invalid(
            "LV-Chordia CQT kernels are not a bounded regular file",
        ));
    }
    let bytes = fs::read(kernels).map_err(|error| AppError::io(kernels, error))?;
    if bytes.get(..8) != Some(MAGIC) {
        return Err(invalid("LV-Chordia CQT kernel format is unsupported"));
    }
    let json_length = bytes
        .get(8..12)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .map(|value| value as usize)
        .ok_or_else(|| invalid("LV-Chordia CQT kernel header is truncated"))?;
    let json_end = 12_usize
        .checked_add(json_length)
        .ok_or_else(|| invalid("LV-Chordia CQT kernel header overflow"))?;
    let descriptor: KernelMetadata = serde_json::from_slice(
        bytes
            .get(12..json_end)
            .ok_or_else(|| invalid("LV-Chordia CQT kernel metadata is truncated"))?,
    )
    .map_err(|error| invalid(&format!("invalid LV-Chordia CQT metadata: {error}")))?;
    validate_metadata(&descriptor)?;
    let payload = bytes
        .get(json_end..)
        .ok_or_else(|| invalid("LV-Chordia CQT payload is missing"))?;

    let mut rate_divisors = descriptor
        .stages
        .iter()
        .map(|stage| stage.rate_divisor)
        .collect::<Vec<_>>();
    rate_divisors.sort_unstable();
    rate_divisors.dedup();
    let mut signals = vec![(1_u32, signal.to_vec())];
    for divisor in rate_divisors.into_iter().filter(|divisor| *divisor > 1) {
        let previous_divisor = divisor / 2;
        let previous = signals
            .iter()
            .find(|(candidate, _)| *candidate == previous_divisor)
            .ok_or_else(|| invalid("LV-Chordia CQT has a non-recursive rate divisor"))?;
        let output_length = previous.1.len().div_ceil(2);
        let mut downsampled = resample_mono_to(&previous.1, 2, 1, output_length)?;
        for sample in &mut downsampled {
            *sample *= std::f32::consts::SQRT_2;
        }
        signals.push((divisor, downsampled));
    }

    let frame_count = descriptor
        .stages
        .iter()
        .map(|stage| {
            signals
                .iter()
                .find(|(divisor, _)| *divisor == stage.rate_divisor)
                .map(|(_, value)| value.len() / stage.hop_length + 1)
                .unwrap_or(usize::MAX)
        })
        .min()
        .unwrap_or(0);
    if frame_count == 0 || frame_count > MAX_CQT_FRAMES {
        return Err(invalid("LV-Chordia CQT frame count is outside its bound"));
    }
    let mut output = vec![0.0_f32; frame_count * descriptor.model_bins];
    for stage in &descriptor.stages {
        let stage_signal = &signals
            .iter()
            .find(|(divisor, _)| *divisor == stage.rate_divisor)
            .ok_or_else(|| invalid("LV-Chordia CQT stage signal is unavailable"))?
            .1;
        execute_stage(
            stage,
            stage_signal,
            payload,
            descriptor.first_model_bin,
            descriptor.model_bins,
            frame_count,
            &mut output,
        )?;
    }
    Ok(LvCqt {
        frames: frame_count,
        values: output,
    })
}

fn validate_metadata(metadata: &KernelMetadata) -> Result<(), AppError> {
    if metadata.sample_rate != 22_050
        || metadata.hop_length != 512
        || metadata.first_model_bin != 18
        || metadata.model_bins != 252
        || !metadata.tuning.is_finite()
        || !(-0.5..0.5).contains(&metadata.tuning)
        || metadata.stages.is_empty()
        || metadata.stages.len() > 16
    {
        return Err(invalid(
            "LV-Chordia CQT metadata does not match the pinned model",
        ));
    }
    for stage in &metadata.stages {
        if stage.rate_divisor == 0
            || !stage.rate_divisor.is_power_of_two()
            || stage.hop_length == 0
            || stage.fft_size < 2
            || !stage.fft_size.is_power_of_two()
            || stage.bins.is_empty()
            || stage.bins.len() > metadata.model_bins
            || stage.bins.windows(2).any(|pair| pair[1] != pair[0] + 1)
            || stage.bins.iter().any(|bin| {
                *bin < metadata.first_model_bin
                    || *bin >= metadata.first_model_bin + metadata.model_bins
            })
        {
            return Err(invalid("LV-Chordia CQT stage metadata is invalid"));
        }
        let expected = stage
            .bins
            .len()
            .checked_mul(stage.fft_size / 2 + 1)
            .ok_or_else(|| invalid("LV-Chordia CQT stage size overflow"))?;
        let counts_match = match stage.kind {
            StageKind::Magnitude => stage.value_count == expected,
            StageKind::Complex => stage.real_count == expected && stage.imag_count == expected,
        };
        if !counts_match {
            return Err(invalid("LV-Chordia CQT stage payload shape is invalid"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_stage(
    stage: &KernelStage,
    signal: &[f32],
    payload: &[u8],
    first_model_bin: usize,
    model_bins: usize,
    frame_count: usize,
    output: &mut [f32],
) -> Result<(), AppError> {
    let frequencies = stage.fft_size / 2 + 1;
    let values = (stage.kind == StageKind::Magnitude)
        .then(|| read_values(payload, stage.value_offset, stage.value_count))
        .transpose()?;
    let real = (stage.kind == StageKind::Complex)
        .then(|| read_values(payload, stage.real_offset, stage.real_count))
        .transpose()?;
    let imaginary = (stage.kind == StageKind::Complex)
        .then(|| read_values(payload, stage.imag_offset, stage.imag_count))
        .transpose()?;
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(stage.fft_size);
    let mut spectrum = vec![Complex32::new(0.0, 0.0); stage.fft_size];
    for frame in 0..frame_count {
        let center = frame * stage.hop_length;
        for (index, destination) in spectrum.iter_mut().enumerate() {
            let position = center as isize + index as isize - (stage.fft_size / 2) as isize;
            let sample = usize::try_from(position)
                .ok()
                .and_then(|position| signal.get(position))
                .copied()
                .unwrap_or(0.0);
            let window = if stage.kind == StageKind::Magnitude {
                0.5 - 0.5
                    * (2.0 * std::f32::consts::PI * index as f32 / stage.fft_size as f32).cos()
            } else {
                1.0
            };
            *destination = Complex32::new(sample * window, 0.0);
        }
        fft.process(&mut spectrum);
        for (row, &global_bin) in stage.bins.iter().enumerate() {
            let mut projected = Complex32::new(0.0, 0.0);
            for (frequency, spectrum_value) in spectrum.iter().enumerate().take(frequencies) {
                let index = row * frequencies + frequency;
                projected += if let Some(values) = &values {
                    Complex32::new(values[index] * spectrum_value.norm(), 0.0)
                } else {
                    Complex32::new(
                        real.as_ref().unwrap()[index],
                        imaginary.as_ref().unwrap()[index],
                    ) * *spectrum_value
                };
            }
            output[frame * model_bins + global_bin - first_model_bin] = projected.norm();
        }
    }
    Ok(())
}

fn read_values(payload: &[u8], offset: usize, count: usize) -> Result<Vec<f32>, AppError> {
    let byte_offset = offset
        .checked_mul(4)
        .ok_or_else(|| invalid("LV-Chordia CQT payload offset overflow"))?;
    let byte_count = count
        .checked_mul(4)
        .ok_or_else(|| invalid("LV-Chordia CQT payload size overflow"))?;
    let bytes = payload
        .get(byte_offset..byte_offset.saturating_add(byte_count))
        .ok_or_else(|| invalid("LV-Chordia CQT payload is truncated"))?;
    Ok(bytes
        .chunks_exact(4)
        .map(|value| f32::from_le_bytes(value.try_into().unwrap()))
        .collect())
}

fn invalid(message: &str) -> AppError {
    AppError::NativeInference(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_untrusted_kernel_file() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("kernels.saccqt");
        fs::write(&path, b"not-a-kernel").unwrap();
        assert!(hybrid_cqt_22050(&[0.0; 22_050], &path).is_err());
    }

    #[test]
    fn generated_kernels_match_the_pinned_librosa_reference_when_available() {
        let Some(path) = std::env::var_os("SONARCAN_LV_CQT_KERNELS") else {
            return;
        };
        let signal = (0..22_050)
            .map(|index| {
                let time = index as f32 / 22_050.0;
                0.2 * (2.0 * std::f32::consts::PI * 440.0 * time).sin()
                    + 0.1 * (2.0 * std::f32::consts::PI * 110.0 * time).sin()
            })
            .collect::<Vec<_>>();
        let cqt = hybrid_cqt_22050(&signal, Path::new(&path)).unwrap();
        assert_eq!(cqt.frames, 44);
        let reference = [
            ((0, 0), 0.061_207_253),
            ((0, 84), 0.013_904_031),
            ((1, 120), 0.088_903_02),
            ((10, 135), 5.097_824_6),
            ((20, 117), 0.001_438_866),
            ((43, 84), 0.012_169_321),
            ((43, 251), 0.001_486_774),
        ];
        for ((frame, bin), expected) in reference {
            let actual = cqt.values[frame * 252 + bin];
            assert!(
                (actual - expected).abs() < 5e-3,
                "{frame}:{bin}: {actual} != {expected}; probes={:?}",
                [
                    cqt.values[0],
                    cqt.values[84],
                    cqt.values[252 + 120],
                    cqt.values[10 * 252 + 135],
                    cqt.values[20 * 252 + 117],
                    cqt.values[43 * 252 + 84],
                    cqt.values[43 * 252 + 251],
                ]
            );
        }
    }

    #[test]
    fn tuning_estimator_matches_librosa_for_reference_tones() {
        let signal = (0..22_050)
            .map(|index| {
                let time = index as f32 / 22_050.0;
                0.2 * (2.0 * std::f32::consts::PI * 440.0 * time).sin()
                    + 0.1 * (2.0 * std::f32::consts::PI * 110.0 * time).sin()
            })
            .collect::<Vec<_>>();
        assert!((estimate_tuning_36(&signal).unwrap() - 0.03).abs() < 0.011);
    }
}
