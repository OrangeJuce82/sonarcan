//! Typed, bounded contract for LV-Chordia output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

const MODE_NAMES: [&str; 3] = ["essential", "standard", "complete"];
const MAX_SEGMENTS_PER_MODE: usize = 8_192;
const MAX_DOWNBEATS: usize = 65_536;
const MAX_BEATS: usize = 262_144;
const MAX_DURATION_SECONDS: f64 = 24.0 * 60.0 * 60.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimedChord {
    pub label: String,
    pub source_label: String,
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
    pub model_version: String,
    pub downbeat_model_version: String,
    pub bpm: Option<f64>,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub modes: BTreeMap<String, Vec<TimedChord>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerAnalysis {
    model_version: String,
    downbeat_model_version: String,
    bpm: Option<f64>,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
    modes: BTreeMap<String, Vec<TimedChord>>,
}

impl WorkerAnalysis {
    pub fn validate(self, track_id: Uuid, cache_version: u32) -> Result<ChordAnalysis, AppError> {
        if self.model_version.len() > 96 || !self.model_version.starts_with("lv-chordia@") {
            return Err(invalid_output("invalid model version"));
        }
        if self.downbeat_model_version.len() > 96
            || !self.downbeat_model_version.starts_with("beat-this@")
        {
            return Err(invalid_output("invalid downbeat model version"));
        }
        if self.modes.len() != MODE_NAMES.len()
            || MODE_NAMES
                .iter()
                .any(|name| !self.modes.contains_key(*name))
        {
            return Err(invalid_output("the three LV-Chordia modes are required"));
        }
        for segments in self.modes.values() {
            validate_segments(segments)?;
        }
        validate_positions(&self.beats, MAX_BEATS, "beat")?;
        validate_positions(&self.downbeats, MAX_DOWNBEATS, "downbeat")?;
        if self
            .bpm
            .is_some_and(|bpm| !bpm.is_finite() || !(30.0..=300.0).contains(&bpm))
        {
            return Err(invalid_output("BPM is outside accepted bounds"));
        }
        if self.downbeats.iter().any(|downbeat| {
            self.beats
                .binary_search_by(|beat| beat.total_cmp(downbeat))
                .is_err()
        }) {
            return Err(invalid_output(
                "a downbeat is not present in the beat sequence",
            ));
        }
        Ok(ChordAnalysis {
            cache_version,
            track_id,
            model_version: self.model_version,
            downbeat_model_version: self.downbeat_model_version,
            bpm: self.bpm,
            beats: self.beats,
            downbeats: self.downbeats,
            modes: self.modes,
        })
    }
}

fn validate_positions(positions: &[f64], maximum: usize, name: &str) -> Result<(), AppError> {
    if positions.len() > maximum {
        return Err(invalid_output(&format!("too many {name}s")));
    }
    let mut previous = -1.0;
    for &position in positions {
        if !position.is_finite()
            || position < 0.0
            || position > MAX_DURATION_SECONDS
            || position <= previous
        {
            return Err(invalid_output(&format!(
                "{name} position is outside accepted bounds"
            )));
        }
        previous = position;
    }
    Ok(())
}

fn validate_segments(segments: &[TimedChord]) -> Result<(), AppError> {
    if segments.len() > MAX_SEGMENTS_PER_MODE {
        return Err(invalid_output("too many chord segments"));
    }
    let mut previous_end = 0.0;
    for chord in segments {
        let label_valid = !chord.label.is_empty()
            && chord.label.len() <= 64
            && chord.label.chars().all(|character| {
                character.is_ascii_alphanumeric() || "#b/()+-°,*".contains(character)
            });
        let source_label_valid = !chord.source_label.is_empty()
            && chord.source_label.len() <= 96
            && chord.source_label.chars().all(|character| {
                character.is_ascii_alphanumeric() || "#:b/()+-°,*".contains(character)
            });
        let scalar_valid = chord.start_seconds.is_finite()
            && chord.end_seconds.is_finite()
            && chord.strength.is_finite()
            && chord.start_seconds >= previous_end - 0.001
            && chord.start_seconds >= 0.0
            && chord.end_seconds > chord.start_seconds
            && chord.end_seconds <= MAX_DURATION_SECONDS
            && (0.0..=1.001).contains(&chord.strength);
        if !label_valid || !source_label_valid || !scalar_valid || chord.bass.is_some() {
            return Err(invalid_output("chord segment is outside accepted bounds"));
        }
        previous_end = chord.end_seconds;
    }
    Ok(())
}

fn invalid_output(message: &str) -> AppError {
    AppError::ChordAnalysis(format!("invalid LV-Chordia output: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(label: &str) -> TimedChord {
        TimedChord {
            label: label.into(),
            source_label: "C:sus2".into(),
            start_seconds: 0.0,
            end_seconds: 1.0,
            bass: None,
            strength: 0.8,
        }
    }

    #[test]
    fn accepts_exactly_three_bounded_lv_chordia_modes() {
        let modes = MODE_NAMES
            .into_iter()
            .map(|name| (name.into(), vec![segment("Csus2")]))
            .collect();
        let result = WorkerAnalysis {
            model_version: "lv-chordia@test".into(),
            downbeat_model_version: "beat-this@test".into(),
            bpm: Some(120.0),
            beats: vec![0.2, 0.7, 1.2, 1.7, 2.2],
            downbeats: vec![0.2, 2.2],
            modes,
        }
        .validate(Uuid::nil(), 9)
        .unwrap();
        assert_eq!(result.modes["standard"][0].label, "Csus2");
        assert_eq!(result.modes["essential"][0].label, "Csus2");
    }

    #[test]
    fn rejects_missing_modes_and_non_finite_values() {
        let mut modes = BTreeMap::new();
        modes.insert("standard".into(), vec![segment("C")]);
        assert!(WorkerAnalysis {
            model_version: "lv-chordia@test".into(),
            downbeat_model_version: "beat-this@test".into(),
            bpm: None,
            beats: vec![],
            downbeats: vec![],
            modes
        }
        .validate(Uuid::nil(), 9)
        .is_err());
        let mut invalid = segment("C");
        invalid.strength = f32::NAN;
        assert!(validate_segments(&[invalid]).is_err());
        assert!(validate_positions(&[2.0, 1.0], MAX_DOWNBEATS, "downbeat").is_err());
    }
}
