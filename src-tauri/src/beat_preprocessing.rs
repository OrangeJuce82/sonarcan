//! Native Beat This! log-mel preprocessing.
//!
//! This matches the pinned `beat_this.preprocessing.LogMelSpect` parameters.
//! Audio downmixing and band-limited resampling are kept here as offline work;
//! they never run in the CPAL callback.

use std::f32::consts::PI;

use rustfft::{num_complex::Complex32, FftPlanner};

use crate::error::AppError;

pub const SAMPLE_RATE: u32 = 22_050;
pub const FFT_SIZE: usize = 1_024;
pub const HOP_SIZE: usize = 441;
pub const MEL_BANDS: usize = 128;
const MIN_FREQUENCY: f32 = 30.0;
const MAX_FREQUENCY: f32 = 11_000.0;
const MAX_FRAMES: usize = 200_000;
const RESAMPLER_RADIUS: isize = 32;
const MAX_RESAMPLED_SAMPLES: usize = MAX_FRAMES * HOP_SIZE;

#[derive(Clone, Debug)]
pub struct LogMelSpectrogram {
    pub frames: usize,
    /// Frame-major values with exactly `frames * MEL_BANDS` entries.
    pub values: Vec<f32>,
}

/// Downmix interleaved PCM and resample it to Beat This!'s 22.05 kHz input.
///
/// The windowed-sinc low-pass filter preserves the analysis bandwidth when
/// downsampling. Keeping this implementation local avoids shipping another
/// DSP runtime solely for one offline preprocessing operation.
pub fn mono_22050(
    interleaved: &[f32],
    channels: usize,
    source_rate: u32,
) -> Result<Vec<f32>, AppError> {
    if channels == 0 || source_rate == 0 || interleaved.is_empty() {
        return Err(AppError::NativeInference(
            "Beat This resampling requires non-empty audio and a valid format".into(),
        ));
    }
    if interleaved.len() % channels != 0 || interleaved.iter().any(|sample| !sample.is_finite()) {
        return Err(AppError::NativeInference(
            "Beat This resampling received malformed or non-finite PCM".into(),
        ));
    }

    let mono = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().map(|&sample| f64::from(sample)).sum::<f64>() / channels as f64)
        .map(|sample| sample as f32)
        .collect::<Vec<_>>();
    if source_rate == SAMPLE_RATE {
        return Ok(mono);
    }

    // libsoxr, used by the reference implementation, returns the nearest
    // integer output length. Mirroring that contract also keeps STFT frame
    // positions stable at the end of a song.
    let output_length =
        ((mono.len() as f64 * f64::from(SAMPLE_RATE) / f64::from(source_rate)).round()) as usize;
    if output_length == 0 || output_length > MAX_RESAMPLED_SAMPLES {
        return Err(AppError::NativeInference(
            "Beat This resampling exceeds the bounded output length".into(),
        ));
    }

    resample_mono_to(&mono, source_rate, SAMPLE_RATE, output_length)
}

pub(crate) fn resample_mono_to(
    mono: &[f32],
    source_rate: u32,
    target_rate: u32,
    output_length: usize,
) -> Result<Vec<f32>, AppError> {
    if mono.is_empty()
        || source_rate == 0
        || target_rate == 0
        || output_length == 0
        || output_length > MAX_RESAMPLED_SAMPLES
        || mono.iter().any(|sample| !sample.is_finite())
    {
        return Err(AppError::NativeInference(
            "native resampling received an invalid shape or sample".into(),
        ));
    }
    let source_per_output = f64::from(source_rate) / f64::from(target_rate);
    let cutoff = (f64::from(target_rate) / f64::from(source_rate)).min(1.0);
    let mut output = Vec::with_capacity(output_length);
    for output_index in 0..output_length {
        let position = output_index as f64 * source_per_output;
        let center = position.floor() as isize;
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        for tap in center - RESAMPLER_RADIUS + 1..=center + RESAMPLER_RADIUS {
            let distance = position - tap as f64;
            let normalized = distance / RESAMPLER_RADIUS as f64;
            if normalized.abs() >= 1.0 {
                continue;
            }
            // Blackman-windowed ideal low-pass response.
            let window = 0.42
                + 0.5 * (std::f64::consts::PI * normalized).cos()
                + 0.08 * (2.0 * std::f64::consts::PI * normalized).cos();
            let argument = std::f64::consts::PI * cutoff * distance;
            let sinc = if argument.abs() < f64::EPSILON {
                cutoff
            } else {
                cutoff * argument.sin() / argument
            };
            let weight = window * sinc;
            if let Ok(index) = usize::try_from(tap) {
                if let Some(sample) = mono.get(index) {
                    weighted_sum += f64::from(*sample) * weight;
                }
            }
            weight_sum += weight;
        }
        output.push(if weight_sum.abs() > f64::EPSILON {
            (weighted_sum / weight_sum) as f32
        } else {
            0.0
        });
    }
    Ok(output)
}

pub fn log_mel_22050(signal: &[f32]) -> Result<LogMelSpectrogram, AppError> {
    if signal.is_empty() {
        return Err(AppError::NativeInference(
            "Beat This preprocessing requires non-empty mono audio".into(),
        ));
    }
    if signal.iter().any(|sample| !sample.is_finite()) {
        return Err(AppError::NativeInference(
            "Beat This preprocessing received non-finite audio".into(),
        ));
    }
    let frames = signal.len() / HOP_SIZE + 1;
    if frames > MAX_FRAMES {
        return Err(AppError::NativeInference(
            "Beat This preprocessing exceeds the bounded frame limit".into(),
        ));
    }

    let window = periodic_hann_window();
    let filters = slaney_filterbank();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut spectrum = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
    let mut values = Vec::with_capacity(frames * MEL_BANDS);

    for frame in 0..frames {
        fill_reflected_frame(signal, frame * HOP_SIZE, &window, &mut spectrum);
        fft.process(&mut spectrum);
        append_log_mel_frame(&spectrum, &filters, &mut values);
    }
    Ok(LogMelSpectrogram { frames, values })
}

fn periodic_hann_window() -> Vec<f32> {
    (0..FFT_SIZE)
        .map(|index| 0.5 - 0.5 * (2.0 * PI * index as f32 / FFT_SIZE as f32).cos())
        .collect()
}

fn fill_reflected_frame(
    signal: &[f32],
    padded_start: usize,
    window: &[f32],
    destination: &mut [Complex32],
) {
    let pad = FFT_SIZE / 2;
    for (index, value) in destination.iter_mut().enumerate() {
        let padded_index = padded_start + index;
        let source_index = reflect_index(padded_index as isize - pad as isize, signal.len());
        *value = Complex32::new(signal[source_index] * window[index], 0.0);
    }
}

fn reflect_index(mut index: isize, length: usize) -> usize {
    if length == 1 {
        return 0;
    }
    let last = length as isize - 1;
    while index < 0 || index > last {
        index = if index < 0 { -index } else { 2 * last - index };
    }
    index as usize
}

fn append_log_mel_frame(spectrum: &[Complex32], filters: &[f32], output: &mut Vec<f32>) {
    let normalization = (FFT_SIZE as f32).sqrt();
    for band in 0..MEL_BANDS {
        let filter = &filters[band * (FFT_SIZE / 2 + 1)..(band + 1) * (FFT_SIZE / 2 + 1)];
        let magnitude = spectrum
            .iter()
            .take(FFT_SIZE / 2 + 1)
            .zip(filter)
            .map(|(bin, weight)| bin.norm() / normalization * weight)
            .sum::<f32>();
        output.push((1_000.0 * magnitude).ln_1p());
    }
}

fn slaney_filterbank() -> Vec<f32> {
    let frequency_bins = FFT_SIZE / 2 + 1;
    let min_mel = hz_to_slaney_mel(MIN_FREQUENCY);
    let max_mel = hz_to_slaney_mel(MAX_FREQUENCY);
    let points = (0..MEL_BANDS + 2)
        .map(|index| {
            let mel = min_mel + (max_mel - min_mel) * index as f32 / (MEL_BANDS + 1) as f32;
            slaney_mel_to_hz(mel)
        })
        .collect::<Vec<_>>();
    let mut filters = vec![0.0; MEL_BANDS * frequency_bins];
    for band in 0..MEL_BANDS {
        let lower = points[band];
        let center = points[band + 1];
        let upper = points[band + 2];
        for bin in 0..frequency_bins {
            let frequency = SAMPLE_RATE as f32 * bin as f32 / FFT_SIZE as f32;
            let rising = (frequency - lower) / (center - lower);
            let falling = (upper - frequency) / (upper - center);
            filters[band * frequency_bins + bin] = rising.min(falling).max(0.0);
        }
    }
    filters
}

fn hz_to_slaney_mel(frequency: f32) -> f32 {
    const LINEAR_SCALE: f32 = 200.0 / 3.0;
    const LOG_THRESHOLD_HZ: f32 = 1_000.0;
    const LOG_THRESHOLD_MEL: f32 = 15.0;
    const LOG_STEP: f32 = 0.068_751_78; // ln(6.4) / 27
    if frequency < LOG_THRESHOLD_HZ {
        frequency / LINEAR_SCALE
    } else {
        LOG_THRESHOLD_MEL + (frequency / LOG_THRESHOLD_HZ).ln() / LOG_STEP
    }
}

fn slaney_mel_to_hz(mel: f32) -> f32 {
    const LINEAR_SCALE: f32 = 200.0 / 3.0;
    const LOG_THRESHOLD_MEL: f32 = 15.0;
    const LOG_STEP: f32 = 0.068_751_78;
    if mel < LOG_THRESHOLD_MEL {
        mel * LINEAR_SCALE
    } else {
        1_000.0 * (LOG_STEP * (mel - LOG_THRESHOLD_MEL)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_without_resampling_at_the_model_rate() {
        let mono = mono_22050(&[1.0, -1.0, 0.5, 0.25], 2, SAMPLE_RATE).unwrap();
        assert_eq!(mono, [0.0, 0.375]);
    }

    #[test]
    fn resampler_preserves_length_dc_and_an_audible_tone() {
        let source_rate = 48_000_u32;
        let input = (0..source_rate as usize)
            .map(|index| {
                0.25 + 0.5
                    * (2.0 * std::f32::consts::PI * 440.0 * index as f32 / source_rate as f32).sin()
            })
            .collect::<Vec<_>>();
        let output = mono_22050(&input, 1, source_rate).unwrap();
        assert_eq!(output.len(), SAMPLE_RATE as usize);
        let interior = &output[64..output.len() - 64];
        let mean = interior.iter().sum::<f32>() / interior.len() as f32;
        assert!((mean - 0.25).abs() < 2e-4, "mean={mean}");
        let amplitude = interior
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (index + 64) as f32 / SAMPLE_RATE as f32;
                (sample - mean) * phase.sin()
            })
            .sum::<f32>()
            * 2.0
            / interior.len() as f32;
        assert!((amplitude - 0.5).abs() < 2e-3, "amplitude={amplitude}");
    }

    #[test]
    fn resampled_log_mel_stays_close_to_the_pinned_soxr_reference() {
        let source_rate = 48_000_u32;
        let input = (0..source_rate as usize)
            .map(|index| {
                let time = index as f32 / source_rate as f32;
                0.25 + 0.5 * (2.0 * std::f32::consts::PI * 440.0 * time).sin()
                    + 0.1 * (2.0 * std::f32::consts::PI * 8_000.0 * time).sin()
            })
            .collect::<Vec<_>>();
        let spectrogram = log_mel_22050(&mono_22050(&input, 1, source_rate).unwrap()).unwrap();
        let maximum = spectrogram.values.iter().copied().fold(0.0_f32, f32::max);
        let reference = [
            ((0, 0), 5.686_128),
            ((0, 20), 6.101_662),
            ((1, 20), 3.181_795),
            ((10, 20), 2.131_09),
            ((25, 40), 0.061_186_5),
            ((50, 64), 3.550_046),
            ((50, 100), 3.430_039),
        ];
        for ((frame, band), expected) in reference {
            let actual = spectrogram.values[frame * MEL_BANDS + band];
            assert!(
                (actual - expected).abs() < 3e-3,
                "{frame}:{band}: {actual} != {expected}"
            );
        }
        assert!((maximum - 8.282_222).abs() < 0.03, "maximum={maximum}");
    }

    #[test]
    fn resampler_rejects_invalid_audio() {
        assert!(mono_22050(&[], 1, 48_000).is_err());
        assert!(mono_22050(&[0.0], 0, 48_000).is_err());
        assert!(mono_22050(&[0.0], 2, 48_000).is_err());
        assert!(mono_22050(&[f32::NAN], 1, 48_000).is_err());
    }

    #[test]
    fn one_second_matches_the_pinned_torchaudio_reference() {
        let signal = (0..SAMPLE_RATE)
            .map(|index| {
                (0.2_f64
                    * (2.0 * std::f64::consts::PI * 440.0 * index as f64 / SAMPLE_RATE as f64)
                        .sin()) as f32
            })
            .collect::<Vec<_>>();
        let spectrogram = log_mel_22050(&signal).unwrap();
        assert_eq!(spectrogram.frames, 51);
        let sum = spectrogram.values.iter().sum::<f32>();
        let squared_sum = spectrogram
            .values
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        let maximum = spectrogram.values.iter().copied().fold(0.0_f32, f32::max);
        assert!((sum - 3_408.920_7).abs() < 0.1, "sum={sum}");
        assert!(
            (squared_sum - 13_759.581).abs() < 0.5,
            "squared_sum={squared_sum}"
        );
        assert!((maximum - 7.366_310_6).abs() < 2e-3, "maximum={maximum}");
        let reference = [
            ((0, 0), 4.772_883),
            ((0, 20), 5.190_091_6),
            ((0, 64), 2.588_840_7),
            ((1, 20), 2.326_514_7),
            ((10, 20), 1.378_672),
            ((25, 40), 0.024_922_082),
            ((50, 64), 2.581_545_4),
            ((50, 100), 1.857_035_5),
        ];
        for ((frame, band), expected) in reference {
            let actual = spectrogram.values[frame * MEL_BANDS + band];
            assert!(
                (actual - expected).abs() < 2e-3,
                "{frame}:{band}: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn rejects_empty_and_non_finite_audio() {
        assert!(log_mel_22050(&[]).is_err());
        assert!(log_mel_22050(&[f32::NAN]).is_err());
    }
}
