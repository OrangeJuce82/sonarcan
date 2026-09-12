//! Native four-stem orchestration around backend-specific ExecuTorch programs.
//!
//! Audio framing, FFT boundaries, overlap-add and tensor interpretation live
//! here. The ExecuTorch bridge remains model-agnostic and Python is never
//! involved at runtime.

use std::{fs, path::Path, time::Duration};

use rustfft::{num_complex::Complex32, FftPlanner};

use crate::{
    audio_engine::DecodedAudio,
    beat_preprocessing::resample_mono_to,
    error::AppError,
    inference_backend::{
        read_tensor_file, write_tensor_file, InferenceBackend, InferenceMeasurement,
        InferenceRequest, NativeTensor,
    },
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: usize = 2;
const SOURCES: usize = 4;
const SCNET_CHUNK: usize = 485_100;
const SCNET_OVERLAP: usize = 4;
const SCNET_FFT: usize = 4_096;
const SCNET_HOP: usize = 1_024;
const SCNET_FRAMES: usize = 476;
const SCNET_BINS: usize = SCNET_FFT / 2 + 1;
const SCNET_SOURCE_TO_APP: [usize; SOURCES] = [3, 0, 1, 2];
const DEMUCS_CHUNK: usize = 343_980;
const DEMUCS_STRIDE: usize = 257_985;
const DEMUCS_FFT: usize = 4_096;
const DEMUCS_HOP: usize = 1_024;
const DEMUCS_BINS: usize = 2_048;
const DEMUCS_FRAMES: usize = 336;

pub struct StemInferenceOutput {
    pub stems: [DecodedAudio; SOURCES],
    pub measurements: Vec<InferenceMeasurement>,
}

pub fn restore_source_format(
    stems: [DecodedAudio; SOURCES],
    source: &DecodedAudio,
) -> Result<[DecodedAudio; SOURCES], AppError> {
    if source.sample_rate == SAMPLE_RATE && stems[0].frames == source.frames {
        return Ok(stems);
    }
    stems
        .into_iter()
        .map(|stem| {
            let mut channels = [
                Vec::with_capacity(stem.frames),
                Vec::with_capacity(stem.frames),
            ];
            for frame in stem.samples.chunks_exact(CHANNELS) {
                channels[0].push(frame[0]);
                channels[1].push(frame[1]);
            }
            let left = resample_mono_to(
                &channels[0],
                stem.sample_rate,
                source.sample_rate,
                source.frames,
            )?;
            let right = resample_mono_to(
                &channels[1],
                stem.sample_rate,
                source.sample_rate,
                source.frames,
            )?;
            Ok(DecodedAudio {
                samples: left
                    .into_iter()
                    .zip(right)
                    .flat_map(|(left, right)| [left, right])
                    .collect(),
                channels: CHANNELS,
                sample_rate: source.sample_rate,
                frames: source.frames,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?
        .try_into()
        .map_err(|_| invalid("native stem inference returned an invalid source count"))
}

/// Run the pinned SCNet Large profile using its real-valued graph partitions.
pub fn separate_scnet(
    backend: &dyn InferenceBackend,
    programs: &Path,
    scratch: &Path,
    source: &DecodedAudio,
) -> Result<StemInferenceOutput, AppError> {
    let mix = stereo_44100(source)?;
    let frames = mix.len() / CHANNELS;
    let step = SCNET_CHUNK / SCNET_OVERLAP;
    let border = SCNET_CHUNK - step;
    let padded = if frames > border * 2 {
        reflect_pad_stereo(&mix, border, border)
    } else {
        mix
    };
    let padded_frames = padded.len() / CHANNELS;
    let fade = SCNET_CHUNK / 10;
    let window = linear_fade_window(SCNET_CHUNK, fade);
    let mut accumulated = vec![0.0_f32; SOURCES * CHANNELS * padded_frames];
    let mut weights = vec![0.0_f32; padded_frames];
    let mut measurements = Vec::new();
    let mut sequence = 0_usize;
    let audio_duration = Duration::from_secs_f64(frames as f64 / f64::from(SAMPLE_RATE));

    for position in (0..padded_frames).step_by(step) {
        let count = (padded_frames - position).min(SCNET_CHUNK);
        let chunk = padded_stereo_chunk(&padded, position, count, SCNET_CHUNK);
        let estimated = infer_scnet_chunk(
            backend,
            programs,
            scratch,
            &chunk,
            audio_duration,
            &mut sequence,
            &mut measurements,
        )?;
        for local in 0..count {
            let weight = if (position == 0 && local < fade)
                || (position + step >= padded_frames && local >= SCNET_CHUNK - fade)
            {
                1.0
            } else {
                window[local]
            };
            weights[position + local] += weight;
            for source_index in 0..SOURCES {
                for channel in 0..CHANNELS {
                    let destination =
                        ((source_index * CHANNELS + channel) * padded_frames) + position + local;
                    let input = ((source_index * CHANNELS + channel) * SCNET_CHUNK) + local;
                    accumulated[destination] += estimated[input] * weight;
                }
            }
        }
    }
    if weights
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid("SCNet overlap-add produced invalid weights"));
    }
    for source_index in 0..SOURCES {
        for channel in 0..CHANNELS {
            let offset = (source_index * CHANNELS + channel) * padded_frames;
            for frame in 0..padded_frames {
                accumulated[offset + frame] /= weights[frame];
            }
        }
    }
    let trim_start = if padded_frames == frames { 0 } else { border };
    let stems = std::array::from_fn(|app_index| {
        let source_index = SCNET_SOURCE_TO_APP[app_index];
        let mut samples = Vec::with_capacity(frames * CHANNELS);
        for frame in 0..frames {
            for channel in 0..CHANNELS {
                samples.push(
                    accumulated
                        [(source_index * CHANNELS + channel) * padded_frames + trim_start + frame],
                );
            }
        }
        DecodedAudio {
            samples,
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            frames,
        }
    });
    Ok(StemInferenceOutput {
        stems,
        measurements,
    })
}

/// Run the pinned HTDemucs neural core around native Hann STFT/ISTFT and the
/// reference 25% triangular overlap-add contract.
pub fn separate_demucs(
    backend: &dyn InferenceBackend,
    program: &Path,
    scratch: &Path,
    source: &DecodedAudio,
) -> Result<StemInferenceOutput, AppError> {
    let mut mix = stereo_44100(source)?;
    let frames = mix.len() / CHANNELS;
    let (mean, deviation) = mixture_normalization(&mix)?;
    for sample in &mut mix {
        *sample = (*sample - mean) / deviation;
    }
    let weight = triangular_window(DEMUCS_CHUNK);
    let mut accumulated = vec![0.0_f32; SOURCES * CHANNELS * frames];
    let mut weights = vec![0.0_f32; frames];
    let mut measurements = Vec::new();
    let mut sequence = 0_usize;
    let audio_duration = Duration::from_secs_f64(frames as f64 / f64::from(SAMPLE_RATE));
    for offset in (0..frames).step_by(DEMUCS_STRIDE) {
        let count = (frames - offset).min(DEMUCS_CHUNK);
        let chunk = centered_zero_padded_chunk(&mix, offset, count, DEMUCS_CHUNK);
        let estimated = infer_demucs_chunk(
            backend,
            program,
            scratch,
            &chunk,
            audio_duration,
            &mut sequence,
            &mut measurements,
        )?;
        let trim = (DEMUCS_CHUNK - count) / 2;
        for local in 0..count {
            weights[offset + local] += weight[local];
            for source_index in 0..SOURCES {
                for channel in 0..CHANNELS {
                    accumulated[(source_index * CHANNELS + channel) * frames + offset + local] +=
                        estimated
                            [(source_index * CHANNELS + channel) * DEMUCS_CHUNK + trim + local]
                            * weight[local];
                }
            }
        }
    }
    if weights
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid("HTDemucs overlap-add produced invalid weights"));
    }
    let stems = std::array::from_fn(|app_index| {
        let source_index = SCNET_SOURCE_TO_APP[app_index];
        let mut samples = Vec::with_capacity(frames * CHANNELS);
        for frame in 0..frames {
            for channel in 0..CHANNELS {
                let normalized = accumulated[(source_index * CHANNELS + channel) * frames + frame]
                    / weights[frame];
                samples.push(normalized * deviation + mean);
            }
        }
        DecodedAudio {
            samples,
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            frames,
        }
    });
    Ok(StemInferenceOutput {
        stems,
        measurements,
    })
}

#[allow(clippy::too_many_arguments)]
fn infer_demucs_chunk(
    backend: &dyn InferenceBackend,
    program: &Path,
    scratch: &Path,
    interleaved: &[f32],
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<Vec<f32>, AppError> {
    if interleaved.len() != DEMUCS_CHUNK * CHANNELS {
        return Err(invalid("HTDemucs received an invalid chunk"));
    }
    let (magnitude, mixture) = demucs_inputs(interleaved)?;
    let outputs = infer(
        backend,
        program,
        &[magnitude, mixture],
        scratch,
        audio_duration,
        sequence,
        measurements,
    )?;
    if outputs.len() != 2
        || outputs[0].dimensions.as_slice()
            != [1, SOURCES, CHANNELS * 2, DEMUCS_BINS, DEMUCS_FRAMES]
        || outputs[1].dimensions.as_slice() != [1, SOURCES, CHANNELS, DEMUCS_CHUNK]
    {
        return Err(invalid("HTDemucs neural core returned an invalid shape"));
    }
    demucs_reconstruct(&outputs[0], &outputs[1])
}

fn demucs_inputs(interleaved: &[f32]) -> Result<(NativeTensor, NativeTensor), AppError> {
    let mixture_values = interleaved_to_channel_major(interleaved);
    let padding = DEMUCS_HOP / 2 * 3;
    let rounded = DEMUCS_CHUNK.div_ceil(DEMUCS_HOP) * DEMUCS_HOP;
    let right = padding + rounded - DEMUCS_CHUNK;
    let window = hann_window(DEMUCS_FFT);
    let mut magnitude = vec![0.0; CHANNELS * 2 * DEMUCS_BINS * DEMUCS_FRAMES];
    for channel in 0..CHANNELS {
        let signal = &mixture_values[channel * DEMUCS_CHUNK..(channel + 1) * DEMUCS_CHUNK];
        let padded = reflect_pad_mono(signal, padding, right);
        let spectrum = centered_stft(&padded, DEMUCS_FFT, DEMUCS_HOP, Some(&window), true)?;
        let all_frames = padded.len() / DEMUCS_HOP + 1;
        for frequency in 0..DEMUCS_BINS {
            for frame in 0..DEMUCS_FRAMES {
                let value = spectrum[frequency * all_frames + frame + 2];
                magnitude[((channel * 2) * DEMUCS_BINS + frequency) * DEMUCS_FRAMES + frame] =
                    value.re;
                magnitude[((channel * 2 + 1) * DEMUCS_BINS + frequency) * DEMUCS_FRAMES + frame] =
                    value.im;
            }
        }
    }
    Ok((
        NativeTensor {
            dimensions: vec![1, CHANNELS * 2, DEMUCS_BINS, DEMUCS_FRAMES],
            values: magnitude,
        },
        NativeTensor {
            dimensions: vec![1, CHANNELS, DEMUCS_CHUNK],
            values: mixture_values,
        },
    ))
}

fn demucs_reconstruct(frequency: &NativeTensor, time: &NativeTensor) -> Result<Vec<f32>, AppError> {
    let window = hann_window(DEMUCS_FFT);
    let padding = DEMUCS_HOP / 2 * 3;
    let synthesis_length = DEMUCS_CHUNK.div_ceil(DEMUCS_HOP) * DEMUCS_HOP + 2 * padding;
    let synthesis_frames = DEMUCS_FRAMES + 4;
    let mut result = vec![0.0; SOURCES * CHANNELS * DEMUCS_CHUNK];
    for source in 0..SOURCES {
        for channel in 0..CHANNELS {
            let mut spectrum = vec![Complex32::new(0.0, 0.0); (DEMUCS_BINS + 1) * synthesis_frames];
            let tensor_channel = channel * 2;
            for bin in 0..DEMUCS_BINS {
                for frame in 0..DEMUCS_FRAMES {
                    spectrum[bin * synthesis_frames + frame + 2] = Complex32::new(
                        frequency.values[(((source * CHANNELS * 2 + tensor_channel)
                            * DEMUCS_BINS
                            + bin)
                            * DEMUCS_FRAMES)
                            + frame],
                        frequency.values[(((source * CHANNELS * 2 + tensor_channel + 1)
                            * DEMUCS_BINS
                            + bin)
                            * DEMUCS_FRAMES)
                            + frame],
                    );
                }
            }
            let audio = centered_istft(
                &spectrum,
                DEMUCS_BINS + 1,
                synthesis_frames,
                DEMUCS_FFT,
                DEMUCS_HOP,
                Some(&window),
                true,
                synthesis_length,
            )?;
            for frame in 0..DEMUCS_CHUNK {
                let destination = (source * CHANNELS + channel) * DEMUCS_CHUNK + frame;
                result[destination] = audio[padding + frame]
                    + time.values[(source * CHANNELS + channel) * DEMUCS_CHUNK + frame];
            }
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn infer_scnet_chunk(
    backend: &dyn InferenceBackend,
    programs: &Path,
    scratch: &Path,
    interleaved: &[f32],
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<Vec<f32>, AppError> {
    if interleaved.len() != SCNET_CHUNK * CHANNELS {
        return Err(invalid("SCNet received an invalid chunk"));
    }
    let mut value = scnet_stft(interleaved)?;
    let mut skips = Vec::with_capacity(3);
    for index in 0..3 {
        let outputs = infer(
            backend,
            &programs.join(format!("scnet-encoder-{index}.pte")),
            &[value],
            scratch,
            audio_duration,
            sequence,
            measurements,
        )?;
        if outputs.len() != 2 {
            return Err(invalid("SCNet encoder returned an invalid output count"));
        }
        value = outputs[0].clone();
        skips.push(outputs[1].clone());
    }
    for index in 0..6 {
        value = infer_one(
            backend,
            &programs.join(format!("scnet-dual-path-{index}.pte")),
            &[value],
            scratch,
            audio_duration,
            sequence,
            measurements,
        )?;
        value = scnet_feature_conversion(value, index % 2 == 0)?;
    }
    for index in 0..3 {
        value = infer_one(
            backend,
            &programs.join(format!("scnet-decoder-{index}.pte")),
            &[
                value,
                skips
                    .pop()
                    .ok_or_else(|| invalid("SCNet skip is missing"))?,
            ],
            scratch,
            audio_duration,
            sequence,
            measurements,
        )?;
    }
    scnet_istft(&value)
}

fn scnet_stft(interleaved: &[f32]) -> Result<NativeTensor, AppError> {
    let padding = {
        let mut value = SCNET_HOP - SCNET_CHUNK % SCNET_HOP;
        if ((SCNET_CHUNK + value) / SCNET_HOP) % 2 == 0 {
            value += SCNET_HOP;
        }
        value
    };
    let length = SCNET_CHUNK + padding;
    let mut channels = [Vec::with_capacity(length), Vec::with_capacity(length)];
    for frame in interleaved.chunks_exact(CHANNELS) {
        channels[0].push(frame[0]);
        channels[1].push(frame[1]);
    }
    channels[0].resize(length, 0.0);
    channels[1].resize(length, 0.0);
    let spectra = channels
        .iter()
        .map(|channel| centered_stft(channel, SCNET_FFT, SCNET_HOP, None, true))
        .collect::<Result<Vec<_>, _>>()?;
    if spectra
        .iter()
        .any(|value| value.len() != SCNET_BINS * SCNET_FRAMES)
    {
        return Err(invalid("SCNet STFT returned an invalid shape"));
    }
    let mut values = vec![0.0; CHANNELS * 2 * SCNET_BINS * SCNET_FRAMES];
    for channel in 0..CHANNELS {
        for frequency in 0..SCNET_BINS {
            for frame in 0..SCNET_FRAMES {
                let complex = spectra[channel][frequency * SCNET_FRAMES + frame];
                values[((channel * 2) * SCNET_BINS + frequency) * SCNET_FRAMES + frame] =
                    complex.re;
                values[((channel * 2 + 1) * SCNET_BINS + frequency) * SCNET_FRAMES + frame] =
                    complex.im;
            }
        }
    }
    Ok(NativeTensor {
        dimensions: vec![1, 4, SCNET_BINS, SCNET_FRAMES],
        values,
    })
}

fn scnet_feature_conversion(input: NativeTensor, forward: bool) -> Result<NativeTensor, AppError> {
    if input.dimensions.len() != 4 || input.dimensions[0] != 1 {
        return Err(invalid(
            "SCNet feature conversion received an invalid tensor",
        ));
    }
    let channels = input.dimensions[1];
    let frequencies = input.dimensions[2];
    let frames = input.dimensions[3];
    let mut planner = FftPlanner::<f32>::new();
    if forward {
        let output_frames = frames / 2 + 1;
        let fft = planner.plan_fft_forward(frames);
        let scale = (frames as f32).sqrt().recip();
        let mut output = vec![0.0; channels * 2 * frequencies * output_frames];
        let mut buffer = vec![Complex32::new(0.0, 0.0); frames];
        for channel in 0..channels {
            for frequency in 0..frequencies {
                for (frame, destination) in buffer.iter_mut().enumerate().take(frames) {
                    *destination = Complex32::new(
                        input.values[(channel * frequencies + frequency) * frames + frame],
                        0.0,
                    );
                }
                fft.process(&mut buffer);
                for frame in 0..output_frames {
                    output[((channel) * frequencies + frequency) * output_frames + frame] =
                        buffer[frame].re * scale;
                    output[((channel + channels) * frequencies + frequency) * output_frames
                        + frame] = buffer[frame].im * scale;
                }
            }
        }
        Ok(NativeTensor {
            dimensions: vec![1, channels * 2, frequencies, output_frames],
            values: output,
        })
    } else {
        if channels % 2 != 0 || frames < 2 {
            return Err(invalid(
                "SCNet inverse feature conversion has an invalid shape",
            ));
        }
        let output_channels = channels / 2;
        let output_frames = (frames - 1) * 2;
        let fft = planner.plan_fft_inverse(output_frames);
        let scale = (output_frames as f32).sqrt().recip();
        let mut output = vec![0.0; output_channels * frequencies * output_frames];
        let mut buffer = vec![Complex32::new(0.0, 0.0); output_frames];
        for channel in 0..output_channels {
            for frequency in 0..frequencies {
                buffer.fill(Complex32::new(0.0, 0.0));
                for (frame, destination) in buffer.iter_mut().enumerate().take(frames) {
                    *destination = Complex32::new(
                        input.values[(channel * frequencies + frequency) * frames + frame],
                        input.values[((channel + output_channels) * frequencies + frequency)
                            * frames
                            + frame],
                    );
                }
                for frame in 1..frames - 1 {
                    buffer[output_frames - frame] = buffer[frame].conj();
                }
                fft.process(&mut buffer);
                for frame in 0..output_frames {
                    output[(channel * frequencies + frequency) * output_frames + frame] =
                        buffer[frame].re * scale;
                }
            }
        }
        Ok(NativeTensor {
            dimensions: vec![1, output_channels, frequencies, output_frames],
            values: output,
        })
    }
}

fn scnet_istft(input: &NativeTensor) -> Result<Vec<f32>, AppError> {
    if input.dimensions.as_slice() != [1, SOURCES * CHANNELS * 2, SCNET_BINS, SCNET_FRAMES] {
        return Err(invalid("SCNet decoder returned an invalid shape"));
    }
    let reconstructed = SCNET_HOP * (SCNET_FRAMES - 1);
    let mut channel_major = vec![0.0; SOURCES * CHANNELS * SCNET_CHUNK];
    for source in 0..SOURCES {
        for channel in 0..CHANNELS {
            let tensor_channel = (source * CHANNELS + channel) * 2;
            let mut spectrum = vec![Complex32::new(0.0, 0.0); SCNET_BINS * SCNET_FRAMES];
            for frequency in 0..SCNET_BINS {
                for frame in 0..SCNET_FRAMES {
                    spectrum[frequency * SCNET_FRAMES + frame] = Complex32::new(
                        input.values
                            [(tensor_channel * SCNET_BINS + frequency) * SCNET_FRAMES + frame],
                        input.values[((tensor_channel + 1) * SCNET_BINS + frequency)
                            * SCNET_FRAMES
                            + frame],
                    );
                }
            }
            let audio = centered_istft(
                &spectrum,
                SCNET_BINS,
                SCNET_FRAMES,
                SCNET_FFT,
                SCNET_HOP,
                None,
                true,
                reconstructed,
            )?;
            channel_major[(source * CHANNELS + channel) * SCNET_CHUNK
                ..(source * CHANNELS + channel + 1) * SCNET_CHUNK]
                .copy_from_slice(&audio[..SCNET_CHUNK]);
        }
    }
    Ok(channel_major)
}

fn centered_stft(
    signal: &[f32],
    fft_size: usize,
    hop: usize,
    window: Option<&[f32]>,
    normalized: bool,
) -> Result<Vec<Complex32>, AppError> {
    if signal.is_empty() || fft_size == 0 || hop == 0 {
        return Err(invalid("native STFT received an invalid signal"));
    }
    let frames = signal.len() / hop + 1;
    let bins = fft_size / 2 + 1;
    let mut result = vec![Complex32::new(0.0, 0.0); bins * frames];
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);
    let scale = if normalized {
        (fft_size as f32).sqrt().recip()
    } else {
        1.0
    };
    let mut buffer = vec![Complex32::new(0.0, 0.0); fft_size];
    for frame in 0..frames {
        for index in 0..fft_size {
            let source = reflect_index(frame * hop + index, fft_size / 2, signal.len());
            let coefficient = window.map_or(1.0, |values| values[index]);
            buffer[index] = Complex32::new(signal[source] * coefficient, 0.0);
        }
        fft.process(&mut buffer);
        for frequency in 0..bins {
            result[frequency * frames + frame] = buffer[frequency] * scale;
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn centered_istft(
    spectrum: &[Complex32],
    bins: usize,
    frames: usize,
    fft_size: usize,
    hop: usize,
    window: Option<&[f32]>,
    normalized: bool,
    length: usize,
) -> Result<Vec<f32>, AppError> {
    if spectrum.len() != bins * frames || bins != fft_size / 2 + 1 {
        return Err(invalid("native ISTFT received an invalid spectrum"));
    }
    let full_length = fft_size + hop * frames.saturating_sub(1);
    let mut result = vec![0.0_f32; full_length];
    let mut envelope = vec![0.0_f32; full_length];
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_inverse(fft_size);
    let scale = if normalized {
        (fft_size as f32).sqrt().recip()
    } else {
        (fft_size as f32).recip()
    };
    let mut buffer = vec![Complex32::new(0.0, 0.0); fft_size];
    for frame in 0..frames {
        buffer.fill(Complex32::new(0.0, 0.0));
        for frequency in 0..bins {
            buffer[frequency] = spectrum[frequency * frames + frame];
        }
        for frequency in 1..bins - 1 {
            buffer[fft_size - frequency] = buffer[frequency].conj();
        }
        fft.process(&mut buffer);
        for index in 0..fft_size {
            let coefficient = window.map_or(1.0, |values| values[index]);
            let destination = frame * hop + index;
            result[destination] += buffer[index].re * scale * coefficient;
            envelope[destination] += coefficient * coefficient;
        }
    }
    let start = fft_size / 2;
    if start + length > result.len() {
        return Err(invalid("native ISTFT cannot satisfy the requested length"));
    }
    (start..start + length)
        .map(|index| {
            (envelope[index] > 1e-11)
                .then_some(result[index] / envelope[index])
                .ok_or_else(|| invalid("native ISTFT window has an uncovered sample"))
        })
        .collect()
}

fn stereo_44100(source: &DecodedAudio) -> Result<Vec<f32>, AppError> {
    if source.sample_rate == 0 || source.frames == 0 || source.channels == 0 {
        return Err(invalid(
            "native stem inference requires valid decoded audio",
        ));
    }
    if source.samples.len() != source.frames * source.channels {
        return Err(invalid("native stem inference received malformed PCM"));
    }
    let stereo = source
        .samples
        .chunks_exact(source.channels)
        .flat_map(|frame| {
            let left = frame[0];
            let right = if source.channels > 1 { frame[1] } else { left };
            [left, right]
        })
        .collect::<Vec<_>>();
    if source.sample_rate == SAMPLE_RATE {
        return Ok(stereo);
    }
    let output_frames = (source.frames as f64 * f64::from(SAMPLE_RATE)
        / f64::from(source.sample_rate))
    .round() as usize;
    let mut channels = [
        Vec::with_capacity(source.frames),
        Vec::with_capacity(source.frames),
    ];
    for frame in stereo.chunks_exact(CHANNELS) {
        channels[0].push(frame[0]);
        channels[1].push(frame[1]);
    }
    let left = resample_mono_to(&channels[0], source.sample_rate, SAMPLE_RATE, output_frames)?;
    let right = resample_mono_to(&channels[1], source.sample_rate, SAMPLE_RATE, output_frames)?;
    Ok(left
        .into_iter()
        .zip(right)
        .flat_map(|(left, right)| [left, right])
        .collect())
}

fn mixture_normalization(interleaved: &[f32]) -> Result<(f32, f32), AppError> {
    if interleaved.is_empty() || interleaved.len() % CHANNELS != 0 {
        return Err(invalid("stem normalization received invalid PCM"));
    }
    let frames = interleaved.len() / CHANNELS;
    let mean = interleaved
        .chunks_exact(CHANNELS)
        .map(|frame| (f64::from(frame[0]) + f64::from(frame[1])) * 0.5)
        .sum::<f64>()
        / frames as f64;
    let variance = interleaved
        .chunks_exact(CHANNELS)
        .map(|frame| (f64::from(frame[0]) + f64::from(frame[1])) * 0.5 - mean)
        .map(|value| value * value)
        .sum::<f64>()
        / frames as f64;
    let deviation = variance.sqrt() + 1e-8;
    if !mean.is_finite() || !deviation.is_finite() || deviation <= 0.0 {
        return Err(invalid("stem normalization produced invalid statistics"));
    }
    Ok((mean as f32, deviation as f32))
}

fn interleaved_to_channel_major(interleaved: &[f32]) -> Vec<f32> {
    let frames = interleaved.len() / CHANNELS;
    let mut output = vec![0.0; interleaved.len()];
    for frame in 0..frames {
        for channel in 0..CHANNELS {
            output[channel * frames + frame] = interleaved[frame * CHANNELS + channel];
        }
    }
    output
}

fn centered_zero_padded_chunk(
    input: &[f32],
    offset: usize,
    count: usize,
    target: usize,
) -> Vec<f32> {
    let total = input.len() / CHANNELS;
    let missing = target - count;
    let logical_start = offset as isize - (missing / 2) as isize;
    let mut output = vec![0.0; target * CHANNELS];
    for local in 0..target {
        let source = logical_start + local as isize;
        if let Ok(source) = usize::try_from(source) {
            if source < total {
                output[local * CHANNELS..local * CHANNELS + CHANNELS]
                    .copy_from_slice(&input[source * CHANNELS..source * CHANNELS + CHANNELS]);
            }
        }
    }
    output
}

fn triangular_window(size: usize) -> Vec<f32> {
    let half = size / 2;
    let maximum = half as f32;
    (0..size)
        .map(|index| {
            let value = if index < half {
                index + 1
            } else {
                size - index
            };
            value as f32 / maximum
        })
        .collect()
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|index| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * index as f32 / size as f32).cos())
        .collect()
}

fn reflect_pad_mono(input: &[f32], left: usize, right: usize) -> Vec<f32> {
    (-(left as isize)..input.len() as isize + right as isize)
        .map(|index| input[reflect_signed(index, input.len())])
        .collect()
}

fn reflect_pad_stereo(input: &[f32], left: usize, right: usize) -> Vec<f32> {
    let frames = input.len() / CHANNELS;
    let mut output = Vec::with_capacity((left + frames + right) * CHANNELS);
    for logical in -(left as isize)..frames as isize + right as isize {
        let source = reflect_signed(logical, frames);
        output.extend_from_slice(&input[source * CHANNELS..source * CHANNELS + CHANNELS]);
    }
    output
}

fn padded_stereo_chunk(input: &[f32], start: usize, count: usize, target: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(target * CHANNELS);
    result.extend_from_slice(&input[start * CHANNELS..(start + count) * CHANNELS]);
    for local in count..target {
        let source = if count > target / 2 {
            reflect_signed(local as isize, count)
        } else {
            count
        };
        if source < count {
            result.extend_from_slice(
                &input[(start + source) * CHANNELS..(start + source + 1) * CHANNELS],
            );
        } else {
            result.extend_from_slice(&[0.0; CHANNELS]);
        }
    }
    result
}

fn linear_fade_window(size: usize, fade: usize) -> Vec<f32> {
    let mut output = vec![1.0; size];
    for index in 0..fade {
        let fraction = index as f32 / (fade - 1) as f32;
        output[index] = fraction;
        output[size - fade + index] = 1.0 - fraction;
    }
    output
}

fn reflect_index(padded_index: usize, pad: usize, length: usize) -> usize {
    reflect_signed(padded_index as isize - pad as isize, length)
}

fn reflect_signed(mut index: isize, length: usize) -> usize {
    if length <= 1 {
        return 0;
    }
    let last = length as isize - 1;
    while index < 0 || index > last {
        index = if index < 0 { -index } else { last * 2 - index };
    }
    index as usize
}

#[allow(clippy::too_many_arguments)]
fn infer_one(
    backend: &dyn InferenceBackend,
    program: &Path,
    inputs: &[NativeTensor],
    scratch: &Path,
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<NativeTensor, AppError> {
    let mut output = infer(
        backend,
        program,
        inputs,
        scratch,
        audio_duration,
        sequence,
        measurements,
    )?;
    if output.len() != 1 {
        return Err(invalid("stem program returned an invalid output count"));
    }
    Ok(output.remove(0))
}

#[allow(clippy::too_many_arguments)]
fn infer(
    backend: &dyn InferenceBackend,
    program: &Path,
    inputs: &[NativeTensor],
    scratch: &Path,
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<Vec<NativeTensor>, AppError> {
    let invocation = *sequence;
    *sequence += 1;
    let input_paths = inputs
        .iter()
        .enumerate()
        .map(|(index, tensor)| {
            let path = scratch.join(format!("stem-{invocation}-input-{index}.tensor"));
            write_tensor_file(&path, &tensor.dimensions, &tensor.values)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let output = scratch.join(format!("stem-{invocation}-output"));
    let measurement = backend.infer(&InferenceRequest {
        program,
        inputs: &input_paths,
        output_directory: &output,
        audio_duration: Some(audio_duration),
    })?;
    let tensors = (0..measurement.output_count)
        .map(|index| read_tensor_file(&output.join(format!("{index}.tensor"))))
        .collect::<Result<Vec<_>, _>>()?;
    measurements.push(measurement);
    for input in input_paths {
        let _ = fs::remove_file(input);
    }
    let _ = fs::remove_dir_all(output);
    Ok(tensors)
}

fn invalid(message: &str) -> AppError {
    AppError::NativeInference(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_backend::{BackendKind, ExecuTorchBackend};

    #[test]
    fn centered_rectangular_stft_round_trips() {
        let signal = (0..8_192)
            .map(|index| (index as f32 * 0.013).sin())
            .collect::<Vec<_>>();
        let spectrum = centered_stft(&signal, 256, 64, None, true).unwrap();
        let frames = signal.len() / 64 + 1;
        let output =
            centered_istft(&spectrum, 129, frames, 256, 64, None, true, signal.len()).unwrap();
        let error = signal
            .iter()
            .zip(output)
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f32, f32::max);
        assert!(error < 1e-4, "round-trip error {error}");
    }

    #[test]
    fn stereo_padding_preserves_shape_and_finiteness() {
        let input = (0..40).map(|value| value as f32).collect::<Vec<_>>();
        let padded = reflect_pad_stereo(&input, 4, 6);
        assert_eq!(padded.len(), input.len() + 20);
        assert!(padded.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn demucs_native_spectrogram_matches_reference() {
        let input = (0..DEMUCS_CHUNK)
            .flat_map(|index| {
                let index = index as f32;
                [(index * 0.001).sin() * 0.1, (index * 0.0013).cos() * 0.08]
            })
            .collect::<Vec<_>>();
        let (magnitude, mixture) = demucs_inputs(&input).unwrap();
        assert_eq!(magnitude.dimensions, [1, 4, 2_048, 336]);
        assert_eq!(mixture.dimensions, [1, 2, DEMUCS_CHUNK]);
        let squared_sum = magnitude
            .values
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        let maximum = magnitude
            .values
            .iter()
            .copied()
            .map(f32::abs)
            .fold(0.0, f32::max);
        assert!((squared_sum - 2_816.651_9).abs() < 0.5, "{squared_sum}");
        assert!((maximum - 2.4224553).abs() < 1e-3, "{maximum}");
    }

    #[test]
    fn real_scnet_partitions_match_reference_when_explicitly_qualified() {
        let (Some(worker), Some(programs)) = (
            std::env::var_os("SONARCAN_EXECUTORCH_WORKER"),
            std::env::var_os("SONARCAN_SCNET_PTE_DIR"),
        ) else {
            return;
        };
        let backend = ExecuTorchBackend::new(BackendKind::Mlx, worker.into()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let input = (0..SCNET_CHUNK)
            .flat_map(|index| {
                let index = index as f32;
                [(index * 0.001).sin() * 0.1, (index * 0.0013).cos() * 0.08]
            })
            .collect::<Vec<_>>();
        let mut sequence = 0;
        let mut measurements = Vec::new();
        let output = infer_scnet_chunk(
            &backend,
            Path::new(&programs),
            temporary.path(),
            &input,
            Duration::from_secs_f64(SCNET_CHUNK as f64 / f64::from(SAMPLE_RATE)),
            &mut sequence,
            &mut measurements,
        )
        .unwrap();
        assert_eq!(measurements.len(), 12);
        assert_eq!(output.len(), SOURCES * CHANNELS * SCNET_CHUNK);
        let squared_sum = output.iter().map(|value| value * value).sum::<f32>();
        let maximum = output.iter().copied().map(f32::abs).fold(0.0, f32::max);
        let probes =
            [0, 1, 485_099, 485_100, 970_199, 1_940_400, 3_880_799].map(|index| output[index]);
        eprintln!("SCNet sq={squared_sum} max={maximum} probes={probes:?}");
        assert!((squared_sum - 3_574.975_6).abs() / 3_574.975_6 < 5e-4);
        assert!((maximum - 0.103899).abs() < 1e-3);
        let expected = [
            -0.046505775,
            0.07073941,
            0.015930887,
            -0.028202547,
            -0.009150323,
            -0.06330806,
            -0.0005675404,
        ];
        for (actual, expected) in probes.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 2e-3, "{actual} != {expected}");
        }
    }

    #[test]
    fn real_demucs_partition_matches_reference_when_explicitly_qualified() {
        let (Some(worker), Some(program)) = (
            std::env::var_os("SONARCAN_EXECUTORCH_WORKER"),
            std::env::var_os("SONARCAN_DEMUCS_PTE"),
        ) else {
            return;
        };
        let backend = ExecuTorchBackend::new(BackendKind::Mlx, worker.into()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let input = (0..DEMUCS_CHUNK)
            .flat_map(|index| {
                let index = index as f32;
                [(index * 0.001).sin() * 0.1, (index * 0.0013).cos() * 0.08]
            })
            .collect::<Vec<_>>();
        let mut sequence = 0;
        let mut measurements = Vec::new();
        let output = infer_demucs_chunk(
            &backend,
            Path::new(&program),
            temporary.path(),
            &input,
            Duration::from_secs_f64(DEMUCS_CHUNK as f64 / f64::from(SAMPLE_RATE)),
            &mut sequence,
            &mut measurements,
        )
        .unwrap();
        assert_eq!(measurements.len(), 1);
        assert_eq!(output.len(), SOURCES * CHANNELS * DEMUCS_CHUNK);
        let squared_sum = output.iter().map(|value| value * value).sum::<f32>();
        let maximum = output.iter().copied().map(f32::abs).fold(0.0, f32::max);
        let probes =
            [0, 1, 343_979, 343_980, 687_959, 1_375_920, 2_751_839].map(|index| output[index]);
        eprintln!("HTDemucs sq={squared_sum} max={maximum} probes={probes:?}");
        assert!((squared_sum - 1_988.146_6).abs() / 1_988.146_6 < 5e-3);
        assert!((maximum - 0.086764924).abs() < 1e-4);
        let expected = [
            0.0063269436,
            0.0043689576,
            -0.027089398,
            0.013313099,
            -0.02249486,
            0.0071809543,
            0.0004379936,
        ];
        for (actual, expected) in probes.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 2e-3, "{actual} != {expected}");
        }
    }
}
