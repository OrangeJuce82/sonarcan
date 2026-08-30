//! Pure, deterministic chord scoring and temporal decoding.
//!
//! Feature extraction is deliberately outside this module: librosa describes
//! the signal, while this engine owns musical interpretation and labelling.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PITCH_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const MAX_CANDIDATES: usize = 8;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub chroma: [f32; 12],
    pub bass_chroma: [f32; 12],
    #[serde(default)]
    pub bass_strength: f32,
    pub silence: f32,
    pub ambiguity: f32,
    #[serde(default)]
    pub key_root: Option<u8>,
    #[serde(default)]
    pub key_minor: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedChordFeatures {
    pub feature_version: u32,
    pub duration_seconds: f64,
    pub key_root: Option<u8>,
    pub key_minor: Option<bool>,
    pub segments: Vec<FeatureSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimedChord {
    pub label: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bass: Option<String>,
    pub strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChordAnalysis {
    pub cache_version: u32,
    pub track_id: Uuid,
    pub chords: Vec<TimedChord>,
    pub simple_chords: Vec<TimedChord>,
}

#[derive(Clone, Copy)]
struct Quality {
    suffix: &'static str,
    intervals: &'static [u8],
}

const QUALITIES: &[Quality] = &[
    Quality {
        suffix: "",
        intervals: &[0, 4, 7],
    },
    Quality {
        suffix: "m",
        intervals: &[0, 3, 7],
    },
    Quality {
        suffix: "7",
        intervals: &[0, 4, 7, 10],
    },
    Quality {
        suffix: "maj7",
        intervals: &[0, 4, 7, 11],
    },
    Quality {
        suffix: "m7",
        intervals: &[0, 3, 7, 10],
    },
    Quality {
        suffix: "dim",
        intervals: &[0, 3, 6],
    },
    Quality {
        suffix: "m7b5",
        intervals: &[0, 3, 6, 10],
    },
    Quality {
        suffix: "aug",
        intervals: &[0, 4, 8],
    },
    Quality {
        suffix: "sus2",
        intervals: &[0, 2, 7],
    },
    Quality {
        suffix: "sus4",
        intervals: &[0, 5, 7],
    },
    Quality {
        suffix: "6",
        intervals: &[0, 4, 7, 9],
    },
    Quality {
        suffix: "m6",
        intervals: &[0, 3, 7, 9],
    },
    Quality {
        suffix: "9",
        intervals: &[0, 2, 4, 7, 10],
    },
    Quality {
        suffix: "7b5",
        intervals: &[0, 4, 6, 10],
    },
    Quality {
        suffix: "7#5",
        intervals: &[0, 4, 8, 10],
    },
];

const SIMPLE_QUALITIES: &[Quality] = &[
    Quality {
        suffix: "",
        intervals: &[0, 4, 7],
    },
    Quality {
        suffix: "m",
        intervals: &[0, 3, 7],
    },
    Quality {
        suffix: "dim",
        intervals: &[0, 3, 6],
    },
    Quality {
        suffix: "m7b5",
        intervals: &[0, 3, 6, 10],
    },
    Quality {
        suffix: "aug",
        intervals: &[0, 4, 8],
    },
];

#[derive(Debug, Clone)]
struct Candidate {
    label: String,
    bass: Option<String>,
    root: Option<u8>,
    score: f32,
}

pub fn decode(
    track_id: Uuid,
    cache_version: u32,
    features: &ExtractedChordFeatures,
) -> ChordAnalysis {
    ChordAnalysis {
        cache_version,
        track_id,
        chords: decode_sequence(features, QUALITIES),
        simple_chords: decode_sequence(features, SIMPLE_QUALITIES),
    }
}

fn decode_sequence(features: &ExtractedChordFeatures, qualities: &[Quality]) -> Vec<TimedChord> {
    let candidate_sets = features
        .segments
        .iter()
        .map(|segment| {
            // Key compatibility is passage-level context. Feeding the key
            // estimated from this same short segment back into its emission
            // score creates a circular bias (for example, a noisy C chord can
            // vote itself into C minor). Keep local estimates as observations
            // for future modulation modelling, not as the current label's
            // harmonic prior.
            candidates(segment, features.key_root, features.key_minor, qualities)
        })
        .collect::<Vec<_>>();
    if candidate_sets.is_empty() {
        return Vec::new();
    }

    let mut costs = candidate_sets[0]
        .iter()
        .map(|candidate| candidate.score)
        .collect::<Vec<_>>();
    let mut parents = vec![Vec::<usize>::new(); candidate_sets.len()];
    for index in 1..candidate_sets.len() {
        let duration = (features.segments[index].end_seconds
            - features.segments[index].start_seconds)
            .max(0.0) as f32;
        let mut next_costs = vec![f32::NEG_INFINITY; candidate_sets[index].len()];
        let mut next_parents = vec![0; candidate_sets[index].len()];
        for (next_index, next) in candidate_sets[index].iter().enumerate() {
            for (previous_index, previous) in candidate_sets[index - 1].iter().enumerate() {
                let score =
                    costs[previous_index] + next.score + transition_score(previous, next, duration);
                if score > next_costs[next_index] {
                    next_costs[next_index] = score;
                    next_parents[next_index] = previous_index;
                }
            }
        }
        costs = next_costs;
        parents[index] = next_parents;
    }

    let mut selected = vec![0; candidate_sets.len()];
    selected[candidate_sets.len() - 1] = argmax(&costs);
    for index in (1..candidate_sets.len()).rev() {
        selected[index - 1] = parents[index][selected[index]];
    }

    let mut chords = Vec::<TimedChord>::new();
    for (index, selected_index) in selected.into_iter().enumerate() {
        let candidate = &candidate_sets[index][selected_index];
        let runner_up = candidate_sets[index]
            .iter()
            .enumerate()
            .filter(|(candidate_index, _)| *candidate_index != selected_index)
            .map(|(_, value)| value.score)
            .fold(f32::NEG_INFINITY, f32::max);
        let strength = ((candidate.score - runner_up).max(0.0) / 0.8).clamp(0.0, 1.0);
        let segment = &features.segments[index];
        if let Some(previous) = chords
            .last_mut()
            .filter(|previous| previous.label == candidate.label && previous.bass == candidate.bass)
        {
            previous.end_seconds = segment.end_seconds;
            previous.strength = (previous.strength + strength) * 0.5;
        } else {
            chords.push(TimedChord {
                label: candidate.label.clone(),
                start_seconds: segment.start_seconds,
                end_seconds: segment.end_seconds,
                bass: candidate.bass.clone(),
                strength,
            });
        }
    }
    let mut chords = stabilize_timed_chords(chords);
    // Silence before the first audible harmony and after the final fade is not
    // a musical chord segment. Keep N inside the passage, where it carries
    // useful information, but do not expose boundary padding as a chord card.
    if chords.first().is_some_and(|chord| chord.label == "N") {
        chords.remove(0);
    }
    if chords.last().is_some_and(|chord| chord.label == "N") {
        chords.pop();
    }
    chords
}

fn stabilize_timed_chords(mut chords: Vec<TimedChord>) -> Vec<TimedChord> {
    const MAX_TRANSIENT_INVERSION_SECONDS: f64 = 1.2;
    const MAX_WEAK_EXCURSION_SECONDS: f64 = 0.75;
    const MAX_WEAK_EXCURSION_STRENGTH: f32 = 0.45;

    // A walking or syncopated bass should not turn every chord tone into a new
    // inversion on the score. Sustained inversions remain available.
    for chord in &mut chords {
        if chord.bass.is_some()
            && chord.end_seconds - chord.start_seconds < MAX_TRANSIENT_INVERSION_SECONDS
        {
            if let Some((base, _)) = chord.label.split_once('/') {
                chord.label = base.to_owned();
                chord.bass = None;
            }
        }
    }

    // Remove a short, weak harmonic excursion only when the same harmony is
    // established on both sides. Strong passing chords and real changes stay.
    for index in 1..chords.len().saturating_sub(1) {
        let duration = chords[index].end_seconds - chords[index].start_seconds;
        let previous_base = chords[index - 1]
            .label
            .split('/')
            .next()
            .unwrap_or("N")
            .to_owned();
        let next_base = chords[index + 1]
            .label
            .split('/')
            .next()
            .unwrap_or("N")
            .to_owned();
        let current_base = chords[index]
            .label
            .split('/')
            .next()
            .unwrap_or("N")
            .to_owned();
        if duration < MAX_WEAK_EXCURSION_SECONDS
            && chords[index].strength < MAX_WEAK_EXCURSION_STRENGTH
            && current_base != "N"
            && previous_base == next_base
            && current_base != previous_base
        {
            chords[index].label = previous_base;
            chords[index].bass = None;
        }
    }

    let mut stable = Vec::<TimedChord>::with_capacity(chords.len());
    for chord in chords {
        if let Some(previous) = stable
            .last_mut()
            .filter(|previous| previous.label == chord.label && previous.bass == chord.bass)
        {
            let previous_duration = (previous.end_seconds - previous.start_seconds) as f32;
            let chord_duration = (chord.end_seconds - chord.start_seconds) as f32;
            previous.end_seconds = chord.end_seconds;
            previous.strength = (previous.strength * previous_duration
                + chord.strength * chord_duration)
                / (previous_duration + chord_duration).max(f32::EPSILON);
        } else {
            stable.push(chord);
        }
    }
    stable
}

fn candidates(
    segment: &FeatureSegment,
    key_root: Option<u8>,
    key_minor: Option<bool>,
    qualities: &[Quality],
) -> Vec<Candidate> {
    let chroma = normalized(segment.chroma);
    let has_bass = segment.bass_chroma.iter().sum::<f32>() > f32::EPSILON;
    let bass_chroma = normalized(segment.bass_chroma);
    let bass_pitch = has_bass.then(|| argmax(&bass_chroma) as u8);
    let mut values = Vec::with_capacity(12 * qualities.len() + 1);
    for root in 0..12_u8 {
        for quality in qualities {
            let tone_mask = quality
                .intervals
                .iter()
                .fold([false; 12], |mut mask, interval| {
                    mask[((root + interval) % 12) as usize] = true;
                    mask
                });
            let chord_energy = chroma
                .iter()
                .enumerate()
                .filter(|(pitch, _)| tone_mask[*pitch])
                .map(|(_, value)| *value)
                .sum::<f32>();
            let foreign_energy = 1.0 - chord_energy;
            let chroma_norm = chroma.iter().map(|value| value * value).sum::<f32>().sqrt();
            let mut template_norm = 0.0;
            let weighted_match = quality
                .intervals
                .iter()
                .enumerate()
                .map(|(index, interval)| {
                    let weight = [1.0_f32, 0.9, 0.75, 0.55, 0.45][index];
                    template_norm += weight * weight;
                    chroma[((root + *interval) % 12) as usize] * weight
                })
                .sum::<f32>();
            let template_similarity =
                weighted_match / (chroma_norm * template_norm.sqrt()).max(f32::EPSILON);
            let missing = quality
                .intervals
                .iter()
                .filter(|interval| chroma[((root + **interval) % 12) as usize] < 0.025)
                .count() as f32;
            let root_energy = chroma[root as usize];
            // Extensions are useful only when their evidence beats a simpler
            // explanation. In full mixes, melody and cymbal energy otherwise
            // turn ordinary triads into a stream of spurious maj7/9 labels.
            let complexity = quality.intervals.len().saturating_sub(3) as f32;
            let quality_prior = match quality.suffix {
                // In a full mix a melodic second or fourth often looks like a
                // suspension for one segment. Require clearer evidence than
                // for the stable major/minor vocabulary.
                "sus2" | "sus4" => -0.10,
                _ => 0.0,
            };
            let mut score = 1.75 * template_similarity
                // A subset chord must not win merely because its lowest note
                // matches the bass. Energy from the omitted chord tones is
                // meaningful evidence (notably D7/F# versus F#dim).
                - 0.85 * foreign_energy
                - 0.08 * missing
                - 0.16 * complexity
                + 0.14 * root_energy
                + quality_prior;
            score += key_score(root, &tone_mask, key_root, key_minor);

            let bass_is_tone = bass_pitch.is_some_and(|bass| tone_mask[bass as usize]);
            if let Some(bass_pitch) = bass_pitch.filter(|_| segment.bass_strength >= 0.12) {
                if bass_is_tone {
                    score += if bass_pitch == root { 0.28 } else { 0.08 };
                } else {
                    score -= 0.18 * bass_chroma[bass_pitch as usize];
                }
            }
            score -= segment.ambiguity.clamp(0.0, 1.0) * 0.25;
            let bass = bass_pitch
                .filter(|bass_pitch| {
                    segment.bass_strength >= 0.12 && bass_is_tone && *bass_pitch != root
                })
                .map(|bass_pitch| PITCH_NAMES[bass_pitch as usize].to_owned());
            let base_label = format!("{}{}", PITCH_NAMES[root as usize], quality.suffix);
            let label = bass
                .as_ref()
                .map_or_else(|| base_label.clone(), |bass| format!("{base_label}/{bass}"));
            values.push(Candidate {
                label,
                bass,
                root: Some(root),
                score,
            });
        }
    }
    values.sort_by(|left, right| right.score.total_cmp(&left.score));
    values.truncate(MAX_CANDIDATES);

    let separation = values
        .first()
        .zip(values.get(1))
        .map_or(0.0, |(first, second)| first.score - second.score);
    let n_score = if segment.silence >= 0.72 {
        2.4 + segment.silence
    } else {
        let best_score = values.first().map_or(0.0, |candidate| candidate.score);
        let unresolved =
            segment.ambiguity.clamp(0.0, 1.0) * (1.0 - (separation / 0.25).clamp(0.0, 1.0));
        // A strong, stable bass is positive harmonic evidence even when the
        // upper voices are dense. Do not let N become a cheap transition
        // bridge between two otherwise different chords (the real-world
        // C–Dm–F regression is the canonical example).
        let tonal_evidence = (segment.bass_strength / 0.25).clamp(0.0, 1.0);
        best_score - 0.50 + 0.38 * unresolved - 0.18 * tonal_evidence
    };
    values.push(Candidate {
        label: "N".into(),
        bass: None,
        root: None,
        score: n_score,
    });
    values
}

fn key_score(root: u8, tones: &[bool; 12], key_root: Option<u8>, key_minor: Option<bool>) -> f32 {
    let (Some(key_root), Some(key_minor)) = (key_root, key_minor) else {
        return 0.0;
    };
    let scale = if key_minor {
        [0, 2, 3, 5, 7, 8, 10]
    } else {
        [0, 2, 4, 5, 7, 9, 11]
    };
    let in_scale = |pitch: u8| scale.contains(&((pitch + 12 - key_root) % 12));
    let root_bonus = if in_scale(root) { 0.12 } else { -0.08 };
    let foreign_tones = tones
        .iter()
        .enumerate()
        .filter(|(pitch, present)| **present && !in_scale(*pitch as u8))
        .count() as f32;
    root_bonus - foreign_tones * 0.035
}

fn transition_score(previous: &Candidate, next: &Candidate, duration: f32) -> f32 {
    if previous.label == next.label {
        return if previous.root.is_none() { -0.04 } else { 0.12 };
    }
    if previous.root.is_none() || next.root.is_none() {
        return -0.06;
    }
    let short_segment_penalty = if duration < 0.65 {
        0.12
    } else if duration < 1.0 {
        0.05
    } else {
        0.0
    };
    let root_distance =
        (previous.root.unwrap() as i16 - next.root.unwrap() as i16).unsigned_abs() as u8;
    let circle_friendly = matches!(root_distance.min(12 - root_distance), 5 | 7);
    -0.12 - short_segment_penalty + if circle_friendly { 0.04 } else { 0.0 }
}

fn normalized(mut values: [f32; 12]) -> [f32; 12] {
    for value in &mut values {
        if !value.is_finite() || *value < 0.0 {
            *value = 0.0;
        }
    }
    let total = values.iter().sum::<f32>();
    if total > f32::EPSILON {
        for value in &mut values {
            *value /= total;
        }
    }
    values
}

fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map_or(0, |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(chroma: [f32; 12], bass: usize) -> FeatureSegment {
        let mut bass_chroma = [0.0; 12];
        bass_chroma[bass] = 1.0;
        FeatureSegment {
            start_seconds: 0.0,
            end_seconds: 2.0,
            chroma,
            bass_chroma,
            bass_strength: 1.0,
            silence: 0.0,
            ambiguity: 0.0,
            key_root: None,
            key_minor: None,
        }
    }

    fn tones(pitches: &[(usize, f32)]) -> [f32; 12] {
        let mut values = [0.0; 12];
        for (pitch, value) in pitches {
            values[*pitch] = *value;
        }
        values
    }

    #[test]
    fn recognizes_major_minor_and_seventh_chords() {
        let cases = [
            (tones(&[(0, 1.0), (4, 0.8), (7, 0.8)]), 0, "C"),
            (tones(&[(9, 1.0), (0, 0.8), (4, 0.8)]), 9, "Am"),
            (tones(&[(7, 1.0), (11, 0.8), (2, 0.8), (5, 0.7)]), 7, "G7"),
            (tones(&[(0, 1.0), (4, 0.8), (8, 0.8)]), 0, "Caug"),
        ];
        for (chroma, bass, expected) in cases {
            let features = ExtractedChordFeatures {
                feature_version: 1,
                duration_seconds: 2.0,
                key_root: None,
                key_minor: None,
                segments: vec![segment(chroma, bass)],
            };
            assert_eq!(decode(Uuid::nil(), 1, &features).chords[0].label, expected);
        }
    }

    #[test]
    fn uses_bass_to_distinguish_c6_from_am7_and_emit_inversions() {
        let shared = tones(&[(0, 0.75), (4, 0.7), (7, 0.65), (9, 1.0)]);
        let c_bass = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 2.0,
            key_root: Some(0),
            key_minor: Some(false),
            segments: vec![segment(shared, 0)],
        };
        let a_bass = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 2.0,
            key_root: Some(9),
            key_minor: Some(true),
            segments: vec![segment(shared, 9)],
        };
        assert_eq!(decode(Uuid::nil(), 1, &c_bass).chords[0].label, "C6");
        assert_eq!(decode(Uuid::nil(), 1, &a_bass).chords[0].label, "Am7");

        let d7 = tones(&[(2, 0.9), (6, 1.0), (9, 0.75), (0, 0.7)]);
        let inversion = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 2.0,
            key_root: Some(7),
            key_minor: Some(false),
            segments: vec![segment(d7, 6)],
        };
        assert_eq!(decode(Uuid::nil(), 1, &inversion).chords[0].label, "D7/F#");
    }

    #[test]
    fn omits_edge_silence_and_merges_stable_segments() {
        let mut first = segment([0.0; 12], 0);
        first.silence = 1.0;
        first.end_seconds = 1.0;
        let mut second = segment(tones(&[(0, 1.0), (4, 0.8), (7, 0.8)]), 0);
        second.start_seconds = 1.0;
        second.end_seconds = 2.0;
        let mut third = second.clone();
        third.start_seconds = 2.0;
        third.end_seconds = 3.0;
        let features = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 3.0,
            key_root: Some(0),
            key_minor: Some(false),
            segments: vec![first, second, third],
        };
        let decoded = decode(Uuid::nil(), 1, &features);
        assert_eq!(decoded.chords.len(), 1);
        assert_eq!(decoded.chords[0].label, "C");
        assert_eq!(decoded.chords[0].start_seconds, 1.0);
        assert_eq!(decoded.chords[0].end_seconds, 3.0);
    }

    #[test]
    fn keeps_n_for_silence_inside_an_audible_passage() {
        let mut c = segment(tones(&[(0, 1.0), (4, 0.8), (7, 0.8)]), 0);
        c.end_seconds = 1.0;
        let mut silence = segment([0.0; 12], 0);
        silence.start_seconds = 1.0;
        silence.end_seconds = 2.0;
        silence.silence = 1.0;
        let mut g = segment(tones(&[(7, 1.0), (11, 0.8), (2, 0.8)]), 7);
        g.start_seconds = 2.0;
        g.end_seconds = 3.0;
        let features = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 3.0,
            key_root: Some(0),
            key_minor: Some(false),
            segments: vec![c, silence, g],
        };
        let decoded = decode(Uuid::nil(), 1, &features);
        assert_eq!(
            decoded
                .chords
                .iter()
                .map(|chord| chord.label.as_str())
                .collect::<Vec<_>>(),
            ["C", "N", "G"]
        );
    }

    #[test]
    fn an_entirely_silent_track_has_no_timed_chord() {
        let mut silence = segment([0.0; 12], 0);
        silence.silence = 1.0;
        let features = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 2.0,
            key_root: None,
            key_minor: None,
            segments: vec![silence],
        };
        assert!(decode(Uuid::nil(), 1, &features).chords.is_empty());
    }

    #[test]
    fn removes_a_weak_bass_driven_excursion_between_stable_chords() {
        let chord = |label: &str, start: f64, end: f64, strength: f32| TimedChord {
            label: label.to_owned(),
            start_seconds: start,
            end_seconds: end,
            bass: None,
            strength,
        };
        let stable = stabilize_timed_chords(vec![
            chord("C", 0.0, 2.0, 0.8),
            chord("G", 2.0, 2.5, 0.2),
            chord("C", 2.5, 4.0, 0.8),
        ]);
        assert_eq!(stable.len(), 1);
        assert_eq!(stable[0].label, "C");
        assert_eq!(stable[0].end_seconds, 4.0);

        let passing = stabilize_timed_chords(vec![
            chord("C", 0.0, 2.0, 0.8),
            chord("G", 2.0, 2.5, 0.8),
            chord("C", 2.5, 4.0, 0.8),
        ]);
        assert_eq!(passing.len(), 3);
    }

    #[test]
    fn suppresses_only_short_inversion_changes() {
        let inversion = |start: f64, end: f64| TimedChord {
            label: "C/E".into(),
            start_seconds: start,
            end_seconds: end,
            bass: Some("E".into()),
            strength: 0.8,
        };
        assert_eq!(
            stabilize_timed_chords(vec![inversion(0.0, 0.8)])[0].label,
            "C"
        );
        assert_eq!(
            stabilize_timed_chords(vec![inversion(0.0, 2.0)])[0].label,
            "C/E"
        );
    }

    #[test]
    fn decodes_hundreds_of_segments_without_inventing_changes() {
        let mut segments = Vec::new();
        for index in 0..600 {
            let variation = (index % 7) as f32 * 0.002;
            let mut value = segment(tones(&[(0, 1.0), (4, 0.82 + variation), (7, 0.78)]), 0);
            value.start_seconds = index as f64 * 0.5;
            value.end_seconds = (index + 1) as f64 * 0.5;
            value.key_root = Some(0);
            value.key_minor = Some(false);
            segments.push(value);
        }
        let features = ExtractedChordFeatures {
            feature_version: 1,
            duration_seconds: 300.0,
            key_root: None,
            key_minor: None,
            segments,
        };
        let decoded = decode(Uuid::nil(), 1, &features);
        assert_eq!(decoded.chords.len(), 1);
        assert_eq!(decoded.chords[0].label, "C");
        assert_eq!(decoded.chords[0].end_seconds, 300.0);
    }

    #[test]
    fn does_not_reduce_real_mix_chroma_to_n() {
        let mut value = segment(
            [
                0.0351, 0.0242, 0.0152, 0.0205, 0.0314, 0.1882, 0.1704, 0.0247, 0.1053, 0.0818,
                0.1645, 0.1387,
            ],
            11,
        );
        value.ambiguity = 0.31;
        value.bass_strength = 0.008;
        value.key_root = Some(6);
        value.key_minor = Some(false);
        let features = ExtractedChordFeatures {
            feature_version: 2,
            duration_seconds: 2.0,
            key_root: Some(1),
            key_minor: Some(false),
            segments: vec![value],
        };
        assert_ne!(decode(Uuid::nil(), 2, &features).chords[0].label, "N");
    }

    #[test]
    fn temporal_context_resolves_a_noisy_d_minor_from_a_real_mix() {
        let observations = [
            // C, Dm, F. These deliberately retain voice, bass movement and
            // percussion leakage from a real commercial mix.
            (
                [
                    0.2845, 0.0493, 0.0617, 0.0399, 0.0832, 0.0267, 0.0390, 0.1389, 0.0437, 0.0288,
                    0.0480, 0.1563,
                ],
                [
                    0.3067, 0.1737, 0.0342, 0.0295, 0.0234, 0.0161, 0.0293, 0.0576, 0.0503, 0.0322,
                    0.0489, 0.1981,
                ],
                0.354,
                0.13,
            ),
            (
                [
                    0.0859, 0.0958, 0.2831, 0.0809, 0.0603, 0.0827, 0.0260, 0.0639, 0.0375, 0.1149,
                    0.0263, 0.0427,
                ],
                [
                    0.0368, 0.1618, 0.3302, 0.2173, 0.0619, 0.0242, 0.0135, 0.0158, 0.0252, 0.0394,
                    0.0324, 0.0415,
                ],
                0.342,
                0.39,
            ),
            (
                [
                    0.1526, 0.0397, 0.0450, 0.0537, 0.1185, 0.3285, 0.0985, 0.0530, 0.0174, 0.0315,
                    0.0207, 0.0409,
                ],
                [
                    0.1184, 0.0728, 0.0343, 0.0490, 0.1365, 0.2799, 0.1693, 0.0229, 0.0092, 0.0156,
                    0.0180, 0.0740,
                ],
                0.395,
                0.06,
            ),
        ];
        let segments = observations
            .into_iter()
            .enumerate()
            .map(|(index, (chroma, bass_chroma, bass_strength, ambiguity))| {
                let bass = argmax(&bass_chroma);
                let mut value = segment(chroma, bass);
                value.bass_chroma = bass_chroma;
                value.bass_strength = bass_strength;
                value.start_seconds = index as f64;
                value.end_seconds = index as f64 + 1.0;
                value.ambiguity = ambiguity;
                value.key_root = Some(if index == 1 { 2 } else { bass as u8 });
                value.key_minor = Some(index == 1);
                value
            })
            .collect();
        let features = ExtractedChordFeatures {
            feature_version: 2,
            duration_seconds: 3.0,
            key_root: Some(0),
            key_minor: Some(false),
            segments,
        };
        let decoded = decode(Uuid::nil(), 2, &features);
        assert!(
            decoded.chords.iter().any(|chord| chord.label == "Dm"),
            "{decoded:?}"
        );
        assert!(
            !decoded.chords.iter().any(|chord| chord.label == "N"),
            "{decoded:?}"
        );
        assert!(
            decoded
                .simple_chords
                .iter()
                .any(|chord| chord.label == "Dm"),
            "{decoded:?}"
        );
        assert!(decoded.simple_chords.iter().all(|chord| {
            let base = chord.label.split('/').next().unwrap_or("N");
            let suffix = base.trim_start_matches(|character: char| {
                character.is_ascii_uppercase() || character == '#'
            });
            base == "N" || matches!(suffix, "" | "m" | "dim" | "m7b5" | "aug")
        }));
    }
}
