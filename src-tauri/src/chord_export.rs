use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const MAX_SEGMENTS: usize = 100_000;
const MAX_LABEL_BYTES: usize = 96;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordExportSegment {
    pub time: f64,
    pub duration: f64,
    pub value: String,
    pub confidence: f64,
}

#[derive(Serialize)]
struct Observation<'a> {
    time: f64,
    duration: f64,
    value: &'a str,
    confidence: f64,
}

pub fn export_jams(
    destination: &Path,
    title: &str,
    duration: f64,
    mode: &str,
    segments: &[ChordExportSegment],
) -> Result<(), AppError> {
    if segments.len() > MAX_SEGMENTS
        || !duration.is_finite()
        || duration < 0.0
        || title.len() > 512
        || mode.len() > 64
    {
        return Err(AppError::BackgroundTask("invalid chord export".into()));
    }
    for segment in segments {
        if !segment.time.is_finite()
            || segment.time < 0.0
            || !segment.duration.is_finite()
            || segment.duration < 0.0
            || !segment.confidence.is_finite()
            || !(0.0..=1.0).contains(&segment.confidence)
            || segment.value.is_empty()
            || segment.value.len() > MAX_LABEL_BYTES
            || segment.value.chars().any(char::is_control)
        {
            return Err(AppError::BackgroundTask("invalid chord segment".into()));
        }
    }

    let data = segments
        .iter()
        .map(|segment| Observation {
            time: segment.time,
            duration: segment.duration,
            value: &segment.value,
            confidence: segment.confidence,
        })
        .collect::<Vec<_>>();
    let document = serde_json::json!({
        "jams_version": "0.3.5",
        "file_metadata": {
            "title": title,
            "artist": "",
            "release": "",
            "duration": duration,
            "identifiers": {}
        },
        "annotations": [{
            "namespace": "chord",
            "annotation_metadata": {
                "curator": { "name": "SonArcan", "email": "" },
                "annotator": {},
                "version": "1",
                "corpus": "",
                "annotation_tools": "SonArcan",
                "annotation_rules": "",
                "validation": "",
                "data_source": "automatic"
            },
            "data": data,
            "sandbox": { "sonarcan_chord_mode": mode }
        }],
        "sandbox": {}
    });
    let contents = serde_json::to_vec_pretty(&document)?;
    fs::write(destination, contents).map_err(|error| AppError::io(destination, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_standard_chord_observations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.jams");
        export_jams(
            &path,
            "Song",
            4.0,
            "standard",
            &[
                ChordExportSegment {
                    time: 0.0,
                    duration: 2.0,
                    value: "C:maj".into(),
                    confidence: 0.9,
                },
                ChordExportSegment {
                    time: 2.0,
                    duration: 2.0,
                    value: "N".into(),
                    confidence: 1.0,
                },
            ],
        )
        .unwrap();
        let document: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(document["annotations"][0]["namespace"], "chord");
        assert_eq!(document["annotations"][0]["data"][0]["value"], "C:maj");
    }
}
