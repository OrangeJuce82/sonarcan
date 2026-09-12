//! Native LV-Chordia dictionary observation model and Viterbi decoder.

use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

use crate::{chord_contract::TimedChord, error::AppError};

const LOGIT_WIDTH: usize = 100;
const HEAD_OFFSETS: [usize; 6] = [0, 73, 86, 90, 94, 97];
const HEAD_WIDTHS: [usize; 6] = [73, 13, 4, 4, 3, 3];
const FRAME_SECONDS: f64 = 512.0 / 22_050.0;
const MAX_FRAMES: usize = 500_000;
const MAX_CHORDS_PER_MODE: usize = 4_096;
const EXPECTED_MODES: [&str; 3] = ["essential", "standard", "complete"];
const CATALOG_JSON: &str = include_str!("lv_decoder_catalog.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    format_version: u32,
    transition_penalty: f32,
    modes: BTreeMap<String, Vec<CatalogChord>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogChord {
    array: [i16; 6],
    raw: String,
    label: String,
    strength_class: usize,
}

static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();

pub fn decode_modes(
    probabilities: &[f32],
    frames: usize,
) -> Result<BTreeMap<String, Vec<TimedChord>>, AppError> {
    if frames == 0
        || frames > MAX_FRAMES
        || probabilities.len() != frames.saturating_mul(LOGIT_WIDTH)
        || probabilities
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.001).contains(value))
    {
        return Err(invalid("invalid probability tensor"));
    }
    let catalog = catalog()?;
    catalog
        .modes
        .iter()
        .map(|(mode, chords)| {
            decode_mode(probabilities, frames, chords, catalog.transition_penalty)
                .map(|segments| (mode.clone(), segments))
        })
        .collect()
}

fn catalog() -> Result<&'static Catalog, AppError> {
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(CATALOG_JSON).map_err(|e| e.to_string())?;
            validate_catalog(&catalog)?;
            Ok(catalog)
        })
        .as_ref()
        .map_err(|message| invalid(&format!("invalid embedded catalog: {message}")))
}

fn validate_catalog(catalog: &Catalog) -> Result<(), String> {
    if catalog.format_version != 1
        || !catalog.transition_penalty.is_finite()
        || !(0.0..=100.0).contains(&catalog.transition_penalty)
        || catalog.modes.len() != EXPECTED_MODES.len()
    {
        return Err("invalid catalog header".into());
    }
    for mode in EXPECTED_MODES {
        let chords = catalog
            .modes
            .get(mode)
            .ok_or_else(|| format!("missing {mode} mode"))?;
        if chords.is_empty() || chords.len() > MAX_CHORDS_PER_MODE {
            return Err(format!("invalid {mode} chord count"));
        }
        for (index, chord) in chords.iter().enumerate() {
            let valid_array = (0..6).all(|head| {
                let minimum = if head == 0 { 0 } else { -1 };
                let maximum = if head == 1 {
                    HEAD_WIDTHS[head] as i16 - 1
                } else {
                    HEAD_WIDTHS[head] as i16
                };
                chord.array[head] >= minimum && chord.array[head] < maximum
            });
            let valid_text = |value: &str, maximum: usize| {
                !value.is_empty()
                    && value.len() <= maximum
                    && value.chars().all(|character| !character.is_control())
            };
            if !valid_array
                || chord.strength_class >= HEAD_WIDTHS[0]
                || !valid_text(&chord.raw, 96)
                || !valid_text(&chord.label, 64)
                || (index == 0 && (chord.raw != "N" || chord.array != [0, -1, -1, -1, -1, -1]))
            {
                return Err(format!("invalid {mode} chord at index {index}"));
            }
        }
    }
    Ok(())
}

fn decode_mode(
    probabilities: &[f32],
    frames: usize,
    chords: &[CatalogChord],
    transition_penalty: f32,
) -> Result<Vec<TimedChord>, AppError> {
    let chord_count = chords.len();
    let decision_bytes = chord_count.div_ceil(8);
    let allocation = frames
        .checked_mul(decision_bytes)
        .ok_or_else(|| invalid("decoder history size overflow"))?;
    let mut same_predecessor = vec![0_u8; allocation];
    let mut global_predecessor = vec![0_u16; frames];
    let mut previous = vec![f32::NEG_INFINITY; chord_count];
    previous[0] = observation(probabilities, 0, &chords[0]);

    for (frame, predecessor) in global_predecessor.iter_mut().enumerate().skip(1) {
        let global = argmax(&previous);
        *predecessor = u16::try_from(global)
            .map_err(|_| invalid("decoder catalog exceeds predecessor range"))?;
        let changed = previous[global] - transition_penalty;
        let row = frame * decision_bytes;
        let mut next = Vec::with_capacity(chord_count);
        for (state, chord) in chords.iter().enumerate() {
            let stay = previous[state] > changed;
            if stay {
                same_predecessor[row + state / 8] |= 1 << (state % 8);
            }
            next.push(previous[state].max(changed) + observation(probabilities, frame, chord));
        }
        previous = next;
    }

    let mut states = vec![0_u16; frames];
    states[frames - 1] = u16::try_from(argmax(&previous))
        .map_err(|_| invalid("decoder catalog exceeds state range"))?;
    for frame in (1..frames).rev() {
        let state = usize::from(states[frame]);
        let stays = same_predecessor[frame * decision_bytes + state / 8] & (1 << (state % 8)) != 0;
        states[frame - 1] = if stays {
            states[frame]
        } else {
            global_predecessor[frame]
        };
    }

    let mut segments = Vec::new();
    let mut start = 0;
    for frame in 0..frames {
        if frame + 1 == frames || states[frame + 1] != states[frame] {
            let chord = &chords[usize::from(states[frame])];
            let strength = (start..=frame)
                .map(|position| probabilities[position * LOGIT_WIDTH + chord.strength_class])
                .sum::<f32>()
                / (frame + 1 - start) as f32;
            segments.push(TimedChord {
                label: chord.label.clone(),
                source_label: chord.raw.clone(),
                start_seconds: rounded_seconds(start),
                end_seconds: rounded_seconds(frame + 1),
                bass: None,
                strength,
            });
            start = frame + 1;
        }
    }
    Ok(segments)
}

fn observation(probabilities: &[f32], frame: usize, chord: &CatalogChord) -> f32 {
    let row = frame * LOGIT_WIDTH;
    let mut value = probabilities[row + chord.array[0] as usize].ln();
    value += probabilities[row + HEAD_OFFSETS[1] + (chord.array[1] + 1) as usize].ln();
    for head in 2..6 {
        if chord.array[head] >= 0 {
            value += probabilities[row + HEAD_OFFSETS[head] + chord.array[head] as usize].ln();
        }
    }
    value
}

fn argmax(values: &[f32]) -> usize {
    let mut best = 0;
    for index in 1..values.len() {
        if values[index] > values[best] {
            best = index;
        }
    }
    best
}

fn rounded_seconds(frame: usize) -> f64 {
    (frame as f64 * FRAME_SECONDS * 1_000_000.0).round() / 1_000_000.0
}

fn invalid(message: &str) -> AppError {
    AppError::ChordAnalysis(format!("LV-Chordia decoder: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_bounded_and_complete() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.modes["essential"].len(), 169);
        assert_eq!(catalog.modes["standard"].len(), 301);
        assert_eq!(catalog.modes["complete"].len(), 2_737);
    }

    #[test]
    fn uniform_heads_remain_in_forced_no_chord_state() {
        let mut probabilities = vec![0.0; 4 * LOGIT_WIDTH];
        for frame in probabilities.chunks_exact_mut(LOGIT_WIDTH) {
            let mut offset = 0;
            for width in HEAD_WIDTHS {
                for value in &mut frame[offset..offset + width] {
                    *value = 1.0 / width as f32;
                }
                offset += width;
            }
        }
        let modes = decode_modes(&probabilities, 4).unwrap();
        for segments in modes.values() {
            assert_eq!(segments.len(), 1);
            assert_eq!(segments[0].label, "N");
            assert_eq!(segments[0].source_label, "N");
            assert_eq!(segments[0].start_seconds, 0.0);
            assert_eq!(segments[0].end_seconds, 0.092_88);
        }
    }

    #[test]
    fn piecewise_probabilities_match_the_python_xhmm_reference() {
        let mut probabilities = vec![0.0; 150 * LOGIT_WIDTH];
        for (position, frame) in probabilities.chunks_exact_mut(LOGIT_WIDTH).enumerate() {
            let targets = if position < 30 {
                [0, 0, 0, 0, 0, 0]
            } else if position < 90 {
                [1, 1, 0, 0, 0, 0]
            } else {
                [15, 3, 0, 0, 0, 0]
            };
            let mut offset = 0;
            for (head, width) in HEAD_WIDTHS.into_iter().enumerate() {
                frame[offset..offset + width].fill(0.1 / (width - 1) as f32);
                frame[offset + targets[head]] = 0.9;
                offset += width;
            }
        }
        for segments in decode_modes(&probabilities, 150).unwrap().values() {
            assert_eq!(
                segments
                    .iter()
                    .map(|segment| (
                        segment.source_label.as_str(),
                        segment.start_seconds,
                        segment.end_seconds
                    ))
                    .collect::<Vec<_>>(),
                [
                    ("N", 0.0, 0.696_599),
                    ("C:maj", 0.696_599, 2.089_796),
                    ("D:min", 2.089_796, 3.482_993),
                ]
            );
            assert!(segments
                .iter()
                .all(|segment| (segment.strength - 0.9).abs() < 2e-6));
        }
    }
}
