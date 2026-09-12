//! Native Beat This! window orchestration and minimal peak decoding.
//!
//! Neural inference remains behind `InferenceBackend`. This module owns only
//! deterministic model-independent tensor layout and timeline decoding.

use std::{fs, path::Path, time::Duration};

use crate::{
    beat_preprocessing::{log_mel_22050, mono_22050, LogMelSpectrogram, MEL_BANDS},
    error::AppError,
    inference_backend::{
        read_tensor_file, write_tensor_file, InferenceBackend, InferenceMeasurement,
        InferenceRequest,
    },
};

pub const WINDOW_FRAMES: usize = 1_500;
pub const BORDER_FRAMES: usize = 6;
pub const RETAINED_FRAMES: usize = WINDOW_FRAMES - 2 * BORDER_FRAMES;
const FRAMES_PER_SECOND: f64 = 50.0;

#[derive(Clone, Debug)]
pub struct BeatModelWindow {
    pub start: isize,
    /// Batch-major `[1, frames, MEL_BANDS]` tensor values.
    pub values: Vec<f32>,
}

impl BeatModelWindow {
    fn frames(&self) -> usize {
        self.values.len() / MEL_BANDS
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MinimalBeatTimeline {
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub bpm: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct BeatInferenceResult {
    pub timeline: MinimalBeatTimeline,
    pub dbn_timeline: MinimalBeatTimeline,
    pub measurements: Vec<InferenceMeasurement>,
}

pub fn infer_audio(
    backend: &dyn InferenceBackend,
    program: &Path,
    interleaved: &[f32],
    channels: usize,
    sample_rate: u32,
    scratch_directory: &Path,
) -> Result<BeatInferenceResult, AppError> {
    let source_frames = interleaved.len().checked_div(channels.max(1)).unwrap_or(0);
    let duration = if sample_rate == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(source_frames as f64 / f64::from(sample_rate))
    };
    let mono = mono_22050(interleaved, channels, sample_rate)?;
    let spectrogram = log_mel_22050(&mono)?;
    infer_fixed_windows(backend, program, &spectrogram, scratch_directory, duration)
}

pub fn infer_fixed_windows(
    backend: &dyn InferenceBackend,
    program: &Path,
    spectrogram: &LogMelSpectrogram,
    scratch_directory: &Path,
    audio_duration: Duration,
) -> Result<BeatInferenceResult, AppError> {
    let scratch_directory = scratch_directory
        .canonicalize()
        .map_err(|error| AppError::io(scratch_directory, error))?;
    let metadata = fs::symlink_metadata(&scratch_directory)
        .map_err(|error| AppError::io(&scratch_directory, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::NativeInference(
            "Beat This scratch path must be a regular directory".into(),
        ));
    }

    let windows = fixed_windows(spectrogram)?;
    let mut predictions = Vec::with_capacity(windows.len());
    let mut measurements = Vec::with_capacity(windows.len());
    for (index, window) in windows.iter().enumerate() {
        let input = scratch_directory.join(format!("window-{index}.tensor"));
        let output = scratch_directory.join(format!("output-{index}"));
        write_tensor_file(&input, &[1, window.frames(), MEL_BANDS], &window.values)?;
        measurements.push(backend.infer(&InferenceRequest {
            program,
            inputs: &[input],
            output_directory: &output,
            audio_duration: Some(audio_duration),
        })?);
        let beat = read_beat_output(&output.join("0.tensor"), window.frames())?;
        let downbeat = read_beat_output(&output.join("1.tensor"), window.frames())?;
        predictions.push((beat, downbeat));
    }
    let (beat_logits, downbeat_logits) =
        aggregate_logits(spectrogram.frames, &windows, &predictions)?;
    Ok(BeatInferenceResult {
        timeline: minimal_timeline(&beat_logits, &downbeat_logits)?,
        dbn_timeline: dbn_timeline(&beat_logits, &downbeat_logits)?,
        measurements,
    })
}

fn read_beat_output(path: &Path, expected_frames: usize) -> Result<Vec<f32>, AppError> {
    let tensor = read_tensor_file(path)?;
    if tensor.values.len() != expected_frames
        || (tensor.dimensions.as_slice() != [expected_frames]
            && tensor.dimensions.as_slice() != [1, expected_frames])
    {
        return Err(AppError::NativeInference(
            "Beat This model returned an unexpected output shape".into(),
        ));
    }
    Ok(tensor.values)
}

/// Reproduce Beat This!'s `split_piece` policy. Long inputs use fixed
/// 1500-frame windows; a short input retains its exact length plus both
/// six-frame zero borders.
pub fn fixed_windows(spectrogram: &LogMelSpectrogram) -> Result<Vec<BeatModelWindow>, AppError> {
    if spectrogram.values.len() != spectrogram.frames.saturating_mul(MEL_BANDS) {
        return Err(AppError::NativeInference(
            "Beat This spectrogram has an inconsistent shape".into(),
        ));
    }

    let frame_count = isize::try_from(spectrogram.frames)
        .map_err(|_| AppError::NativeInference("Beat This frame count overflow".into()))?;
    let step = RETAINED_FRAMES as isize;
    let mut starts = Vec::new();
    let mut start = -(BORDER_FRAMES as isize);
    while start < frame_count - BORDER_FRAMES as isize {
        starts.push(start);
        start += step;
    }
    if spectrogram.frames > RETAINED_FRAMES {
        if let Some(last) = starts.last_mut() {
            *last = frame_count - (WINDOW_FRAMES - BORDER_FRAMES) as isize;
        }
    }

    Ok(starts
        .into_iter()
        .map(|start| {
            let window_frames = if spectrogram.frames < RETAINED_FRAMES {
                spectrogram.frames + 2 * BORDER_FRAMES
            } else {
                WINDOW_FRAMES
            };
            let mut values = vec![0.0; window_frames * MEL_BANDS];
            for destination_frame in 0..window_frames {
                let source_frame = start + destination_frame as isize;
                if !(0..frame_count).contains(&source_frame) {
                    continue;
                }
                let source_frame = source_frame as usize;
                let source = source_frame * MEL_BANDS..(source_frame + 1) * MEL_BANDS;
                let destination =
                    destination_frame * MEL_BANDS..(destination_frame + 1) * MEL_BANDS;
                values[destination].copy_from_slice(&spectrogram.values[source]);
            }
            BeatModelWindow { start, values }
        })
        .collect())
}

/// Reproduce `aggregate_prediction(..., overlap_mode="keep_first")`.
pub fn aggregate_logits(
    frame_count: usize,
    windows: &[BeatModelWindow],
    predictions: &[(Vec<f32>, Vec<f32>)],
) -> Result<(Vec<f32>, Vec<f32>), AppError> {
    if windows.len() != predictions.len() {
        return Err(AppError::NativeInference(
            "Beat This window predictions have an inconsistent shape".into(),
        ));
    }
    let mut beats = vec![-1_000.0; frame_count];
    let mut downbeats = vec![-1_000.0; frame_count];
    for (window, (window_beats, window_downbeats)) in windows.iter().zip(predictions).rev() {
        let window_frames = window.frames();
        if window_beats.len() != window_frames || window_downbeats.len() != window_frames {
            return Err(AppError::NativeInference(
                "Beat This window predictions do not match their input window".into(),
            ));
        }
        for source in BORDER_FRAMES..window_frames - BORDER_FRAMES {
            let destination = window.start + source as isize;
            if let Ok(destination) = usize::try_from(destination) {
                if destination < frame_count {
                    beats[destination] = window_beats[source];
                    downbeats[destination] = window_downbeats[source];
                }
            }
        }
    }
    Ok((beats, downbeats))
}

pub fn minimal_timeline(
    beat_logits: &[f32],
    downbeat_logits: &[f32],
) -> Result<MinimalBeatTimeline, AppError> {
    if beat_logits.len() != downbeat_logits.len()
        || beat_logits
            .iter()
            .chain(downbeat_logits)
            .any(|value| !value.is_finite())
    {
        return Err(AppError::NativeInference(
            "Beat This logits are inconsistent or non-finite".into(),
        ));
    }
    if beat_logits.is_empty() {
        return Ok(MinimalBeatTimeline {
            beats: Vec::new(),
            downbeats: Vec::new(),
            bpm: None,
        });
    }
    let beats = peak_times(beat_logits);
    let raw_downbeats = peak_times(downbeat_logits);
    let mut downbeats = raw_downbeats
        .into_iter()
        .filter_map(|downbeat| {
            beats
                .iter()
                .copied()
                .min_by(|left, right| (left - downbeat).abs().total_cmp(&(right - downbeat).abs()))
        })
        .collect::<Vec<_>>();
    downbeats.dedup_by(|left, right| left == right);
    Ok(MinimalBeatTimeline {
        bpm: bpm_from_beats(&beats),
        beats,
        downbeats,
    })
}

/// Native equivalent of Beat This!'s pinned madmom DBN configuration.
pub fn dbn_timeline(
    beat_logits: &[f32],
    downbeat_logits: &[f32],
) -> Result<MinimalBeatTimeline, AppError> {
    if beat_logits.len() != downbeat_logits.len()
        || beat_logits
            .iter()
            .chain(downbeat_logits)
            .any(|value| !value.is_finite())
    {
        return Err(AppError::NativeInference(
            "Beat This logits are inconsistent or non-finite".into(),
        ));
    }
    let activations = beat_logits
        .iter()
        .zip(downbeat_logits)
        .map(|(&beat, &downbeat)| {
            let beat_probability = bounded_sigmoid(beat);
            let downbeat_probability = bounded_sigmoid(downbeat);
            [
                (beat_probability - downbeat_probability).max(0.000_005),
                downbeat_probability,
            ]
        })
        .collect::<Vec<_>>();
    let Some(first) = activations
        .iter()
        .position(|activation| activation[0] >= 0.05 || activation[1] >= 0.05)
    else {
        return Ok(MinimalBeatTimeline {
            beats: Vec::new(),
            downbeats: Vec::new(),
            bpm: None,
        });
    };
    let last = activations
        .iter()
        .rposition(|activation| activation[0] >= 0.05 || activation[1] >= 0.05)
        .expect("the first threshold crossing guarantees a last crossing")
        + 1;
    let active = &activations[first..last];
    let candidates = [3_usize, 4]
        .into_iter()
        .map(|beats_per_bar| decode_meter(active, beats_per_bar))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = candidates.into_iter();
    let mut best = candidates
        .next()
        .ok_or_else(|| AppError::NativeInference("Beat This DBN has no meter".into()))?;
    for candidate in candidates {
        if candidate.log_probability > best.log_probability {
            best = candidate;
        }
    }
    let beats = best
        .beats
        .iter()
        .map(|&(frame, _)| round_six((frame + first) as f64 / FRAMES_PER_SECOND))
        .collect::<Vec<_>>();
    let downbeats = best
        .beats
        .iter()
        .filter(|(_, beat_number)| *beat_number == 1)
        .map(|&(frame, _)| round_six((frame + first) as f64 / FRAMES_PER_SECOND))
        .collect::<Vec<_>>();
    Ok(MinimalBeatTimeline {
        bpm: bpm_from_beats(&beats),
        beats,
        downbeats,
    })
}

fn bounded_sigmoid(logit: f32) -> f64 {
    let probability = 1.0 / (1.0 + (-f64::from(logit)).exp());
    probability * (1.0 - 1e-5) + 5e-6
}

struct DbmDecode {
    beats: Vec<(usize, usize)>,
    log_probability: f64,
}

#[derive(Clone, Copy)]
struct DbmState {
    beat: usize,
    interval: usize,
    offset: usize,
    pointer: usize,
}

fn decode_meter(activations: &[[f64; 2]], beats_per_bar: usize) -> Result<DbmDecode, AppError> {
    const MIN_INTERVAL: usize = 14;
    const MAX_INTERVAL: usize = 55;
    const INTERVALS: usize = MAX_INTERVAL - MIN_INTERVAL + 1;
    let mut states = Vec::new();
    let mut first_states = vec![vec![0_usize; INTERVALS]; beats_per_bar];
    let mut last_states = vec![vec![0_usize; INTERVALS]; beats_per_bar];
    for beat in 0..beats_per_bar {
        for (interval_index, interval) in (MIN_INTERVAL..=MAX_INTERVAL).enumerate() {
            first_states[beat][interval_index] = states.len();
            for offset in 0..interval {
                let position = beat as f64 + offset as f64 / interval as f64;
                let pointer = if position < 1.0 / 16.0 {
                    2
                } else if position % 1.0 < 1.0 / 16.0 {
                    1
                } else {
                    0
                };
                states.push(DbmState {
                    beat,
                    interval: interval_index,
                    offset,
                    pointer,
                });
            }
            last_states[beat][interval_index] = states.len() - 1;
        }
    }
    if states.len() > u16::MAX as usize {
        return Err(AppError::NativeInference(
            "Beat This DBN state space exceeds its bound".into(),
        ));
    }

    let transition_logs = (MIN_INTERVAL..=MAX_INTERVAL)
        .map(|from| {
            let mut weights = (MIN_INTERVAL..=MAX_INTERVAL)
                .map(|to| (-100.0 * ((to as f64 / from as f64) - 1.0).abs()).exp())
                .map(|weight| if weight <= f64::EPSILON { 0.0 } else { weight })
                .collect::<Vec<_>>();
            let sum = weights.iter().sum::<f64>();
            for weight in &mut weights {
                *weight = if *weight == 0.0 {
                    f64::NEG_INFINITY
                } else {
                    (*weight / sum).ln()
                };
            }
            weights
        })
        .collect::<Vec<_>>();
    let boundary_count = beats_per_bar * INTERVALS;
    let mut boundary_predecessors = vec![0_u8; activations.len().saturating_mul(boundary_count)];
    let initial = -(states.len() as f64).ln();
    let mut previous = states
        .iter()
        .map(|state| initial + observation_log(activations[0], state.pointer))
        .collect::<Vec<_>>();
    let mut current = vec![f64::NEG_INFINITY; states.len()];

    for (frame, &activation) in activations.iter().enumerate().skip(1) {
        for (state_index, state) in states.iter().enumerate() {
            let (score, predecessor) = if state.offset > 0 {
                (previous[state_index - 1], 0)
            } else {
                let previous_beat = (state.beat + beats_per_bar - 1) % beats_per_bar;
                let mut best_score = f64::NEG_INFINITY;
                let mut best_interval = 0_usize;
                for from_interval in 0..INTERVALS {
                    let score = previous[last_states[previous_beat][from_interval]]
                        + transition_logs[from_interval][state.interval];
                    if score > best_score {
                        best_score = score;
                        best_interval = from_interval;
                    }
                }
                (best_score, best_interval)
            };
            current[state_index] = score + observation_log(activation, state.pointer);
            if state.offset == 0 {
                let boundary = state.beat * INTERVALS + state.interval;
                boundary_predecessors[frame * boundary_count + boundary] = predecessor as u8;
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }
    let (mut state_index, &log_probability) = previous
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .ok_or_else(|| AppError::NativeInference("Beat This DBN has no states".into()))?;
    let mut path = vec![0_usize; activations.len()];
    for frame in (0..activations.len()).rev() {
        path[frame] = state_index;
        if frame == 0 {
            break;
        }
        let state = states[state_index];
        state_index = if state.offset > 0 {
            state_index - 1
        } else {
            let boundary = state.beat * INTERVALS + state.interval;
            let previous_interval =
                boundary_predecessors[frame * boundary_count + boundary] as usize;
            let previous_beat = (state.beat + beats_per_bar - 1) % beats_per_bar;
            last_states[previous_beat][previous_interval]
        };
    }

    let mut beats = Vec::new();
    let mut region_start = None;
    for frame in 0..=path.len() {
        let in_beat_range = frame < path.len() && states[path[frame]].pointer >= 1;
        match (region_start, in_beat_range) {
            (None, true) => region_start = Some(frame),
            (Some(left), false) => {
                let right = frame;
                let mut peak = left;
                let mut peak_value = f64::NEG_INFINITY;
                for (offset, activation) in activations[left..right].iter().enumerate() {
                    for value in activation {
                        if *value > peak_value {
                            peak_value = *value;
                            peak = left + offset;
                        }
                    }
                }
                beats.push((peak, states[path[peak]].beat + 1));
                region_start = None;
            }
            _ => {}
        }
    }
    Ok(DbmDecode {
        beats,
        log_probability,
    })
}

fn observation_log(activation: [f64; 2], pointer: usize) -> f64 {
    match pointer {
        1 => activation[0].ln(),
        2 => activation[1].ln(),
        _ => ((1.0 - activation[0] - activation[1]) / 15.0).ln(),
    }
}

fn peak_times(logits: &[f32]) -> Vec<f64> {
    let frames = logits
        .iter()
        .enumerate()
        .filter_map(|(index, &value)| {
            let first = index.saturating_sub(3);
            let last = (index + 4).min(logits.len());
            (value > 0.0
                && logits[first..last]
                    .iter()
                    .all(|candidate| value >= *candidate))
            .then_some(index as f64)
        })
        .collect::<Vec<_>>();
    deduplicate_peaks(&frames)
        .into_iter()
        .map(|frame| round_six(frame / FRAMES_PER_SECOND))
        .collect()
}

fn deduplicate_peaks(peaks: &[f64]) -> Vec<f64> {
    let Some((&first, remaining)) = peaks.split_first() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut mean = first;
    let mut count = 1_u32;
    for &peak in remaining {
        if peak - mean <= 1.0 {
            count += 1;
            mean += (peak - mean) / f64::from(count);
        } else {
            result.push(mean);
            mean = peak;
            count = 1;
        }
    }
    result.push(mean);
    result
}

fn bpm_from_beats(beats: &[f64]) -> Option<f64> {
    let mut intervals = beats
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|interval| (0.2..=2.0).contains(interval))
        .collect::<Vec<_>>();
    if intervals.is_empty() {
        return None;
    }
    intervals.sort_by(f64::total_cmp);
    let middle = intervals.len() / 2;
    let median = if intervals.len() % 2 == 0 {
        (intervals[middle - 1] + intervals[middle]) / 2.0
    } else {
        intervals[middle]
    };
    Some((60.0 / median * 10.0).round() / 10.0)
}

fn round_six(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_backend::BackendKind;

    fn spectrogram(frames: usize) -> LogMelSpectrogram {
        LogMelSpectrogram {
            frames,
            values: (0..frames * MEL_BANDS).map(|value| value as f32).collect(),
        }
    }

    #[test]
    fn fixed_windows_match_the_reference_split_and_keep_first_overlap() {
        let spectrogram = spectrogram(3_000);
        let windows = fixed_windows(&spectrogram).unwrap();
        assert_eq!(
            windows
                .iter()
                .map(|window| window.start)
                .collect::<Vec<_>>(),
            [-6, 1_482, 1_506]
        );
        assert_eq!(windows[0].values[..6 * MEL_BANDS], vec![0.0; 6 * MEL_BANDS]);
        assert_eq!(windows[0].values[6 * MEL_BANDS], 0.0);
        assert_eq!(
            windows[2].values[(WINDOW_FRAMES - 6) * MEL_BANDS..],
            vec![0.0; 6 * MEL_BANDS]
        );

        let predictions = windows
            .iter()
            .enumerate()
            .map(|(index, _)| (vec![index as f32; WINDOW_FRAMES], vec![0.0; WINDOW_FRAMES]))
            .collect::<Vec<_>>();
        let (beats, _) = aggregate_logits(3_000, &windows, &predictions).unwrap();
        assert_eq!(beats[0], 0.0);
        assert_eq!(beats[1_487], 0.0);
        assert_eq!(beats[1_488], 1.0);
        assert_eq!(beats[2_975], 1.0);
        assert_eq!(beats[2_976], 2.0);
    }

    #[test]
    fn short_audio_uses_its_exact_dynamic_shape() {
        let short = fixed_windows(&spectrogram(257)).unwrap();
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].start, -6);
        assert_eq!(short[0].frames(), 269);
        assert_eq!(short[0].values[..6 * MEL_BANDS], vec![0.0; 6 * MEL_BANDS]);
        assert_eq!(short[0].values[6 * MEL_BANDS], 0.0);
        assert_eq!(
            short[0].values[(short[0].frames() - 6) * MEL_BANDS..],
            vec![0.0; 6 * MEL_BANDS]
        );
        assert_eq!(
            fixed_windows(&spectrogram(RETAINED_FRAMES)).unwrap().len(),
            1
        );
    }

    #[test]
    fn minimal_decoder_matches_peak_pooling_deduplication_and_snapping() {
        let mut beat = vec![-1.0; 160];
        let mut downbeat = vec![-1.0; 160];
        beat[25] = 2.0;
        beat[26] = 2.0;
        beat[50] = 1.0;
        beat[75] = 1.0;
        beat[100] = 1.0;
        downbeat[24] = 1.0;
        downbeat[26] = 0.5;
        downbeat[99] = 1.0;
        let timeline = minimal_timeline(&beat, &downbeat).unwrap();
        assert_eq!(timeline.beats, [0.51, 1.0, 1.5, 2.0]);
        assert_eq!(timeline.downbeats, [0.51, 2.0]);
        assert_eq!(timeline.bpm, Some(120.0));
    }

    #[test]
    fn dbn_decoder_matches_the_pinned_madmom_meter_path() {
        let mut beat = vec![-5.0; 1_000];
        let mut downbeat = vec![-7.0; 1_000];
        for frame in (25..1_000).step_by(25) {
            beat[frame] = 5.0;
        }
        for frame in (25..1_000).step_by(100) {
            downbeat[frame] = 6.0;
        }
        let timeline = dbn_timeline(&beat, &downbeat).unwrap();
        assert_eq!(timeline.beats.len(), 39);
        assert_eq!(timeline.beats[0], 0.5);
        assert_eq!(timeline.beats[38], 19.5);
        assert_eq!(
            timeline.downbeats,
            [0.5, 2.5, 4.5, 6.5, 8.5, 10.5, 12.5, 14.5, 16.5, 18.5]
        );
        assert_eq!(timeline.bpm, Some(120.0));
    }

    struct MockBackend;

    impl InferenceBackend for MockBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Mlx
        }

        fn probe(&self, _program: &Path) -> Result<InferenceMeasurement, AppError> {
            unreachable!()
        }

        fn infer(&self, request: &InferenceRequest<'_>) -> Result<InferenceMeasurement, AppError> {
            let input = read_tensor_file(&request.inputs[0])?;
            assert_eq!(input.dimensions[0], 1);
            assert_eq!(input.dimensions[2], MEL_BANDS);
            let frames = input.dimensions[1];
            fs::create_dir(request.output_directory)
                .map_err(|error| AppError::io(request.output_directory, error))?;
            let mut beat = vec![-1.0; frames];
            let mut downbeat = vec![-1.0; frames];
            let (first, second) = if frames == WINDOW_FRAMES {
                (506, 531)
            } else {
                (31, 56)
            };
            beat[first] = 2.0;
            beat[second] = 2.0;
            downbeat[first] = 2.0;
            write_tensor_file(
                &request.output_directory.join("0.tensor"),
                &[1, frames],
                &beat,
            )?;
            write_tensor_file(
                &request.output_directory.join("1.tensor"),
                &[1, frames],
                &downbeat,
            )?;
            Ok(InferenceMeasurement {
                selected_backend: BackendKind::Mlx,
                output_count: 2,
                wall_time_ms: 1.0,
                load_time_ms: 0.2,
                inference_time_ms: 0.8,
                real_time_factor: Some(0.01),
                peak_rss_bytes: 1,
                runtime_bytes: 1,
                program_bytes: 1,
            })
        }
    }

    #[test]
    fn backend_neutral_orchestrator_round_trips_native_tensor_files() {
        let temporary = tempfile::tempdir().unwrap();
        let program = temporary.path().join("beat.pte");
        fs::write(&program, b"test").unwrap();
        let scratch = temporary.path().join("scratch");
        fs::create_dir(&scratch).unwrap();
        let result = infer_fixed_windows(
            &MockBackend,
            &program,
            &spectrogram(RETAINED_FRAMES),
            &scratch,
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.timeline.beats, [10.0, 10.5]);
        assert_eq!(result.timeline.downbeats, [10.0]);
        assert_eq!(result.timeline.bpm, Some(120.0));
    }

    #[test]
    fn native_audio_pipeline_downmixes_resamples_and_runs_a_dynamic_program() {
        let temporary = tempfile::tempdir().unwrap();
        let program = temporary.path().join("beat.pte");
        fs::write(&program, b"test").unwrap();
        let scratch = temporary.path().join("scratch");
        fs::create_dir(&scratch).unwrap();
        let samples = vec![0.0; 48_000 * 2];
        let result = infer_audio(&MockBackend, &program, &samples, 2, 48_000, &scratch).unwrap();
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.timeline.beats, [0.5, 1.0]);
        assert_eq!(result.timeline.downbeats, [0.5]);
        assert_eq!(result.timeline.bpm, Some(120.0));
    }
}
