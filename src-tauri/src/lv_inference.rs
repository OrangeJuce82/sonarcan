//! Backend-neutral native orchestration for the five-member LV-Chordia ensemble.

use std::{fs, path::Path, time::Duration};

use crate::{
    error::AppError,
    inference_backend::{
        read_tensor_file, write_tensor_file, InferenceBackend, InferenceMeasurement,
        InferenceRequest, NativeTensor,
    },
    lv_cqt::LvCqt,
};

const MEMBERS: usize = 5;
const LAYERS: usize = 10;
const INPUT_BINS: usize = 252;
const FEATURE_WIDTH: usize = 240;
const HIDDEN_WIDTH: usize = 96;
const LOGIT_WIDTH: usize = 100;
const CONVOLUTION_CHUNK: usize = 64;
const CLASSIFIER_CHUNK: usize = 64;
const RECURRENT_CHUNKS: [usize; 4] = [64, 16, 4, 1];

#[derive(Clone, Debug)]
pub struct LvEnsembleOutput {
    pub frames: usize,
    /// Frame-major probabilities for the concatenated six chord heads.
    pub probabilities: Vec<f32>,
    pub measurements: Vec<InferenceMeasurement>,
}

pub fn infer_ensemble(
    backend: &dyn InferenceBackend,
    program_directory: &Path,
    cqt: &LvCqt,
    scratch_directory: &Path,
    audio_duration: Duration,
) -> Result<LvEnsembleOutput, AppError> {
    validate_inputs(program_directory, cqt, scratch_directory)?;
    let mut probability_sum = vec![0.0_f32; cqt.frames * LOGIT_WIDTH];
    let mut measurements = Vec::new();
    let mut sequence = 0_usize;
    for member in 0..MEMBERS {
        let mut value = NativeTensor {
            dimensions: vec![1, 1, cqt.frames, INPUT_BINS],
            values: cqt.values.clone(),
        };
        for layer in 0..LAYERS {
            value = run_convolution_layer(
                backend,
                &program_directory.join(format!("lv-chordia-s{member}-convolution-{layer}.pte")),
                value,
                scratch_directory,
                audio_duration,
                &mut sequence,
                &mut measurements,
                matches!(layer, 2 | 5 | 7).then_some(if layer == 7 { 4 } else { 3 }),
            )?;
        }
        let features = convolution_to_features(value, cqt.frames)?;
        let forward = run_recurrent(
            backend,
            program_directory,
            member,
            "forward",
            &features,
            false,
            scratch_directory,
            audio_duration,
            &mut sequence,
            &mut measurements,
        )?;
        let backward = run_recurrent(
            backend,
            program_directory,
            member,
            "backward",
            &features,
            true,
            scratch_directory,
            audio_duration,
            &mut sequence,
            &mut measurements,
        )?;
        let recurrent = forward
            .chunks_exact(HIDDEN_WIDTH)
            .zip(backward.chunks_exact(HIDDEN_WIDTH))
            .flat_map(|(forward, backward)| forward.iter().chain(backward).copied())
            .collect::<Vec<_>>();
        let logits = run_classifier(
            backend,
            &program_directory.join(format!(
                "lv-chordia-s{member}-classifier-{CLASSIFIER_CHUNK}.pte"
            )),
            &recurrent,
            cqt.frames,
            scratch_directory,
            audio_duration,
            &mut sequence,
            &mut measurements,
        )?;
        softmax_heads_accumulate(&logits, &mut probability_sum)?;
    }
    for probability in &mut probability_sum {
        *probability /= MEMBERS as f32;
    }
    Ok(LvEnsembleOutput {
        frames: cqt.frames,
        probabilities: probability_sum,
        measurements,
    })
}

fn validate_inputs(programs: &Path, cqt: &LvCqt, scratch: &Path) -> Result<(), AppError> {
    if cqt.frames == 0
        || cqt.values.len() != cqt.frames.saturating_mul(INPUT_BINS)
        || cqt.values.iter().any(|value| !value.is_finite())
    {
        return Err(invalid("LV-Chordia received an invalid CQT tensor"));
    }
    for (path, description) in [(programs, "program"), (scratch, "scratch")] {
        let metadata = fs::symlink_metadata(path).map_err(|error| AppError::io(path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid(&format!(
                "LV-Chordia {description} path is not a regular directory"
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_convolution_layer(
    backend: &dyn InferenceBackend,
    program: &Path,
    input: NativeTensor,
    scratch: &Path,
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
    pool: Option<usize>,
) -> Result<NativeTensor, AppError> {
    if input.dimensions.len() != 4 || input.dimensions[0] != 1 {
        return Err(invalid("LV-Chordia convolution input has an invalid shape"));
    }
    let channels = input.dimensions[1];
    let frames = input.dimensions[2];
    let frequencies = input.dimensions[3];
    let padded_frames = frames.div_ceil(CONVOLUTION_CHUNK) * CONVOLUTION_CHUNK;
    let mut assembled: Option<NativeTensor> = None;
    for cursor in (0..padded_frames).step_by(CONVOLUTION_CHUNK) {
        let mut chunk = vec![0.0_f32; channels * (CONVOLUTION_CHUNK + 2) * frequencies];
        for channel in 0..channels {
            for local_frame in 0..CONVOLUTION_CHUNK + 2 {
                let Some(source_frame) = cursor
                    .checked_add(local_frame)
                    .and_then(|value| value.checked_sub(1))
                else {
                    continue;
                };
                if source_frame >= frames {
                    continue;
                }
                let source = (channel * frames + source_frame) * frequencies;
                let destination = (channel * (CONVOLUTION_CHUNK + 2) + local_frame) * frequencies;
                chunk[destination..destination + frequencies]
                    .copy_from_slice(&input.values[source..source + frequencies]);
            }
        }
        let output = infer_one(
            backend,
            program,
            &[NativeTensor {
                dimensions: vec![1, channels, CONVOLUTION_CHUNK + 2, frequencies],
                values: chunk,
            }],
            scratch,
            audio_duration,
            sequence,
            measurements,
        )?;
        if output.dimensions.len() != 4
            || output.dimensions[0] != 1
            || output.dimensions[2] != CONVOLUTION_CHUNK + 2
        {
            return Err(invalid("LV-Chordia convolution returned an invalid shape"));
        }
        let target = assembled.get_or_insert_with(|| NativeTensor {
            dimensions: vec![1, output.dimensions[1], padded_frames, output.dimensions[3]],
            values: vec![0.0; output.dimensions[1] * padded_frames * output.dimensions[3]],
        });
        if target.dimensions[1] != output.dimensions[1]
            || target.dimensions[3] != output.dimensions[3]
        {
            return Err(invalid("LV-Chordia convolution chunks changed shape"));
        }
        let output_channels = output.dimensions[1];
        let output_frequencies = output.dimensions[3];
        for channel in 0..output_channels {
            for local_frame in 0..CONVOLUTION_CHUNK {
                let source =
                    (channel * (CONVOLUTION_CHUNK + 2) + local_frame + 1) * output_frequencies;
                let destination =
                    (channel * padded_frames + cursor + local_frame) * output_frequencies;
                target.values[destination..destination + output_frequencies]
                    .copy_from_slice(&output.values[source..source + output_frequencies]);
            }
        }
    }
    let mut output =
        assembled.ok_or_else(|| invalid("LV-Chordia convolution produced no chunks"))?;
    truncate_time(&mut output, frames)?;
    instance_norm_selu(&mut output)?;
    if let Some(width) = pool {
        output = max_pool_frequency(output, width)?;
    }
    Ok(output)
}

fn truncate_time(tensor: &mut NativeTensor, frames: usize) -> Result<(), AppError> {
    let channels = tensor.dimensions[1];
    let old_frames = tensor.dimensions[2];
    let frequencies = tensor.dimensions[3];
    if frames > old_frames {
        return Err(invalid("LV-Chordia cannot extend a convolution tensor"));
    }
    let mut values = Vec::with_capacity(channels * frames * frequencies);
    for channel in 0..channels {
        let start = channel * old_frames * frequencies;
        values.extend_from_slice(&tensor.values[start..start + frames * frequencies]);
    }
    tensor.dimensions[2] = frames;
    tensor.values = values;
    Ok(())
}

fn instance_norm_selu(tensor: &mut NativeTensor) -> Result<(), AppError> {
    let channels = tensor.dimensions[1];
    let area = tensor.dimensions[2].saturating_mul(tensor.dimensions[3]);
    if area == 0 || tensor.values.len() != channels.saturating_mul(area) {
        return Err(invalid("LV-Chordia normalization shape is invalid"));
    }
    for channel in tensor.values.chunks_exact_mut(area) {
        let mean = channel.iter().map(|value| f64::from(*value)).sum::<f64>() / area as f64;
        let variance = channel
            .iter()
            .map(|value| (f64::from(*value) - mean).powi(2))
            .sum::<f64>()
            / area as f64;
        let inverse = 1.0 / (variance + 1e-5).sqrt();
        for value in channel {
            let normalized = ((f64::from(*value) - mean) * inverse) as f32;
            *value = if normalized > 0.0 {
                1.050_701 * normalized
            } else {
                1.050_701 * 1.673_263_2 * normalized.exp_m1()
            };
        }
    }
    Ok(())
}

fn max_pool_frequency(input: NativeTensor, width: usize) -> Result<NativeTensor, AppError> {
    let channels = input.dimensions[1];
    let frames = input.dimensions[2];
    let frequencies = input.dimensions[3];
    if frequencies % width != 0 {
        return Err(invalid(
            "LV-Chordia pooling width does not divide its input",
        ));
    }
    let output_frequencies = frequencies / width;
    let mut values = Vec::with_capacity(channels * frames * output_frequencies);
    for channel in 0..channels {
        for frame in 0..frames {
            let base = (channel * frames + frame) * frequencies;
            for frequency in 0..output_frequencies {
                values.push(
                    input.values[base + frequency * width..base + (frequency + 1) * width]
                        .iter()
                        .copied()
                        .fold(f32::NEG_INFINITY, f32::max),
                );
            }
        }
    }
    Ok(NativeTensor {
        dimensions: vec![1, channels, frames, output_frequencies],
        values,
    })
}

fn convolution_to_features(input: NativeTensor, frames: usize) -> Result<Vec<f32>, AppError> {
    if input.dimensions.as_slice() != [1, 80, frames, 3] {
        return Err(invalid(
            "LV-Chordia feature extractor returned an invalid shape",
        ));
    }
    let mut output = vec![0.0; frames * FEATURE_WIDTH];
    for frame in 0..frames {
        for channel in 0..80 {
            for frequency in 0..3 {
                output[frame * FEATURE_WIDTH + channel * 3 + frequency] =
                    input.values[(channel * frames + frame) * 3 + frequency];
            }
        }
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn run_recurrent(
    backend: &dyn InferenceBackend,
    programs: &Path,
    member: usize,
    direction: &str,
    features: &[f32],
    reverse: bool,
    scratch: &Path,
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<Vec<f32>, AppError> {
    let frames = features.len() / FEATURE_WIDTH;
    let mut hidden = NativeTensor {
        dimensions: vec![1, 1, HIDDEN_WIDTH],
        values: vec![0.0; HIDDEN_WIDTH],
    };
    let mut cell = hidden.clone();
    let mut output = vec![0.0; frames * HIDDEN_WIDTH];
    let mut cursor = 0;
    while cursor < frames {
        let count = frames - cursor;
        let program_count = RECURRENT_CHUNKS
            .into_iter()
            .find(|candidate| *candidate <= count)
            .ok_or_else(|| invalid("LV-Chordia recurrent chunk is unavailable"))?;
        {
            let mut chunk = vec![0.0; program_count * FEATURE_WIDTH];
            for local in 0..program_count {
                let logical = cursor + local;
                let source_frame = if reverse {
                    frames - 1 - logical
                } else {
                    logical
                };
                chunk[local * FEATURE_WIDTH..(local + 1) * FEATURE_WIDTH].copy_from_slice(
                    &features[source_frame * FEATURE_WIDTH..(source_frame + 1) * FEATURE_WIDTH],
                );
            }
            let outputs = infer_many(
                backend,
                &programs.join(format!(
                    "lv-chordia-s{member}-{direction}-{program_count}.pte"
                )),
                &[
                    NativeTensor {
                        dimensions: vec![1, program_count, FEATURE_WIDTH],
                        values: chunk,
                    },
                    hidden,
                    cell,
                ],
                scratch,
                audio_duration,
                sequence,
                measurements,
            )?;
            if outputs.len() != 3
                || outputs[0].dimensions.as_slice() != [1, program_count, HIDDEN_WIDTH]
            {
                return Err(invalid(
                    "LV-Chordia recurrent program returned an invalid shape",
                ));
            }
            for local in 0..program_count {
                let logical = cursor + local;
                let destination_frame = if reverse {
                    frames - 1 - logical
                } else {
                    logical
                };
                output[destination_frame * HIDDEN_WIDTH..(destination_frame + 1) * HIDDEN_WIDTH]
                    .copy_from_slice(
                        &outputs[0].values[local * HIDDEN_WIDTH..(local + 1) * HIDDEN_WIDTH],
                    );
            }
            hidden = outputs[1].clone();
            cell = outputs[2].clone();
            cursor += program_count;
        }
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn run_classifier(
    backend: &dyn InferenceBackend,
    program: &Path,
    recurrent: &[f32],
    frames: usize,
    scratch: &Path,
    audio_duration: Duration,
    sequence: &mut usize,
    measurements: &mut Vec<InferenceMeasurement>,
) -> Result<Vec<f32>, AppError> {
    let padded = frames.div_ceil(CLASSIFIER_CHUNK) * CLASSIFIER_CHUNK;
    let mut output = vec![0.0; frames * LOGIT_WIDTH];
    for cursor in (0..padded).step_by(CLASSIFIER_CHUNK) {
        let mut input = vec![0.0; CLASSIFIER_CHUNK * HIDDEN_WIDTH * 2];
        let count = (frames - cursor).min(CLASSIFIER_CHUNK);
        input[..count * HIDDEN_WIDTH * 2].copy_from_slice(
            &recurrent[cursor * HIDDEN_WIDTH * 2..(cursor + count) * HIDDEN_WIDTH * 2],
        );
        let result = infer_one(
            backend,
            program,
            &[NativeTensor {
                dimensions: vec![1, CLASSIFIER_CHUNK, HIDDEN_WIDTH * 2],
                values: input,
            }],
            scratch,
            audio_duration,
            sequence,
            measurements,
        )?;
        if result.dimensions.as_slice() != [1, CLASSIFIER_CHUNK, LOGIT_WIDTH] {
            return Err(invalid("LV-Chordia classifier returned an invalid shape"));
        }
        output[cursor * LOGIT_WIDTH..(cursor + count) * LOGIT_WIDTH]
            .copy_from_slice(&result.values[..count * LOGIT_WIDTH]);
    }
    Ok(output)
}

fn softmax_heads_accumulate(logits: &[f32], sum: &mut [f32]) -> Result<(), AppError> {
    const WIDTHS: [usize; 6] = [73, 13, 4, 4, 3, 3];
    if logits.len() != sum.len() || logits.len() % LOGIT_WIDTH != 0 {
        return Err(invalid("LV-Chordia logits have an invalid shape"));
    }
    for (frame, destination) in logits
        .chunks_exact(LOGIT_WIDTH)
        .zip(sum.chunks_exact_mut(LOGIT_WIDTH))
    {
        let mut offset = 0;
        for width in WIDTHS {
            let head = &frame[offset..offset + width];
            let maximum = head.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let denominator = head
                .iter()
                .map(|value| (*value - maximum).exp())
                .sum::<f32>();
            for index in 0..width {
                destination[offset + index] += (head[index] - maximum).exp() / denominator;
            }
            offset += width;
        }
    }
    Ok(())
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
    let mut outputs = infer_many(
        backend,
        program,
        inputs,
        scratch,
        audio_duration,
        sequence,
        measurements,
    )?;
    if outputs.len() != 1 {
        return Err(invalid(
            "LV-Chordia program returned an unexpected output count",
        ));
    }
    Ok(outputs.remove(0))
}

#[allow(clippy::too_many_arguments)]
fn infer_many(
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
            let path = scratch.join(format!("lv-{invocation}-input-{index}.tensor"));
            write_tensor_file(&path, &tensor.dimensions, &tensor.values)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let output = scratch.join(format!("lv-{invocation}-output"));
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
    fn instance_norm_and_pool_are_bounded_and_finite() {
        let mut tensor = NativeTensor {
            dimensions: vec![1, 2, 2, 6],
            values: (0..24).map(|value| value as f32).collect(),
        };
        instance_norm_selu(&mut tensor).unwrap();
        assert!(tensor.values.iter().all(|value| value.is_finite()));
        let pooled = max_pool_frequency(tensor, 3).unwrap();
        assert_eq!(pooled.dimensions, [1, 2, 2, 2]);
        assert_eq!(pooled.values.len(), 8);
    }

    #[test]
    fn head_softmax_normalizes_each_output_family() {
        let mut sum = vec![0.0; LOGIT_WIDTH];
        softmax_heads_accumulate(&vec![0.0; LOGIT_WIDTH], &mut sum).unwrap();
        let mut offset = 0;
        for width in [73, 13, 4, 4, 3, 3] {
            assert!((sum[offset..offset + width].iter().sum::<f32>() - 1.0).abs() < 1e-6);
            offset += width;
        }
    }

    #[test]
    fn real_partition_suite_runs_when_explicitly_qualified() {
        let (Some(worker), Some(programs)) = (
            std::env::var_os("SONARCAN_EXECUTORCH_WORKER"),
            std::env::var_os("SONARCAN_LV_PTE_DIR"),
        ) else {
            return;
        };
        let backend = ExecuTorchBackend::new(BackendKind::Mlx, worker.into()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let result = infer_ensemble(
            &backend,
            Path::new(&programs),
            &LvCqt {
                frames: 37,
                values: vec![1.0; 37 * INPUT_BINS],
            },
            temporary.path(),
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(result.frames, 37);
        assert_eq!(result.probabilities.len(), 37 * LOGIT_WIDTH);
        assert_eq!(result.measurements.len(), MEMBERS * (LAYERS + 9));
        for frame in result.probabilities.chunks_exact(LOGIT_WIDTH) {
            let mut offset = 0;
            for width in [73, 13, 4, 4, 3, 3] {
                assert!((frame[offset..offset + width].iter().sum::<f32>() - 1.0).abs() < 1e-4);
                offset += width;
            }
        }
        let squared_sum = result
            .probabilities
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        let maximum = result.probabilities.iter().copied().fold(0.0_f32, f32::max);
        let probes = [(0, 0), (0, 72), (0, 73), (10, 50), (20, 90), (36, 99)]
            .map(|(frame, class)| result.probabilities[frame * LOGIT_WIDTH + class]);
        assert!(
            (squared_sum - 77.179_75).abs() < 2e-3,
            "squared_sum={squared_sum}, probes={probes:?}"
        );
        assert!((maximum - 0.846_254_94).abs() < 2e-4, "maximum={maximum}");
        for ((frame, class), expected) in [
            ((0, 0), 0.165_049_4),
            ((0, 72), 0.004_935_332_6),
            ((0, 73), 0.229_702_79),
            ((10, 50), 0.003_498_032),
            ((20, 90), 0.145_555_03),
            ((36, 99), 0.112_463_01),
        ] {
            let actual = result.probabilities[frame * LOGIT_WIDTH + class];
            assert!(
                (actual - expected).abs() < 2e-4,
                "{frame}:{class}: {actual}"
            );
        }
    }
}
