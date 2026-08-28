use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use rusty_chromaprint::{match_fingerprints, Configuration, Fingerprinter};
use serde::{Deserialize, Serialize};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as SymphoniaError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

use crate::error::AppError;

const FINGERPRINT_VERSION: u8 = 2;
const MAX_FINGERPRINT_SECONDS: u64 = 10;
const MAX_FINGERPRINT_FILE_BYTES: u64 = 256 * 1024;
const MAX_DURATION_DIFFERENCE_SECONDS: f64 = 2.0;
const MAX_DURATION_DIFFERENCE_RATIO: f64 = 0.01;
const MAX_MATCH_SCORE: f64 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioFingerprint {
    version: u8,
    values: Vec<u32>,
}

impl AudioFingerprint {
    fn new(values: Vec<u32>) -> Self {
        Self {
            version: FINGERPRINT_VERSION,
            values,
        }
    }
}

pub fn load(path: &Path) -> Result<Option<AudioFingerprint>, AppError> {
    let Ok(file) = File::open(path) else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(MAX_FINGERPRINT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::io(path, error))?;
    if bytes.len() as u64 > MAX_FINGERPRINT_FILE_BYTES {
        return Ok(None);
    }
    Ok(serde_json::from_slice::<AudioFingerprint>(&bytes)
        .ok()
        .filter(|fingerprint| fingerprint.version == FINGERPRINT_VERSION))
}

pub fn save(path: &Path, fingerprint: &AudioFingerprint) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    let contents = serde_json::to_vec(fingerprint)?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| AppError::io(&temporary, error))?;
        file.write_all(&contents)
            .map_err(|error| AppError::io(&temporary, error))?;
        file.sync_data()
            .map_err(|error| AppError::io(&temporary, error))?;
        fs::rename(&temporary, path).map_err(|error| AppError::io(path, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn calculate(path: &Path) -> Result<AudioFingerprint, AppError> {
    let file = File::open(path).map_err(|error| AppError::io(path, error))?;
    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| fingerprint_error(path, error))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AppError::AudioFingerprint {
            path: path.to_path_buf(),
            reason: "the file contains no default audio track".to_owned(),
        })?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| AppError::AudioFingerprint {
            path: path.to_path_buf(),
            reason: "the audio sample rate is unavailable".to_owned(),
        })?;
    let channels = track
        .codec_params
        .channels
        .map(|value| value.count())
        .ok_or_else(|| AppError::AudioFingerprint {
            path: path.to_path_buf(),
            reason: "the audio channel count is unavailable".to_owned(),
        })?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| fingerprint_error(path, error))?;
    let mut fingerprinter = Fingerprinter::new(&Configuration::preset_test2());
    fingerprinter
        .start(sample_rate, channels as u32)
        .map_err(|error| AppError::AudioFingerprint {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let sample_limit = sample_rate as u64 * channels as u64 * MAX_FINGERPRINT_SECONDS;
    let mut samples_consumed = 0_u64;

    while samples_consumed < sample_limit {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(fingerprint_error(path, error)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(fingerprint_error(path, error)),
        };
        let mut buffer = SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        let remaining = usize::try_from(sample_limit - samples_consumed).unwrap_or(usize::MAX);
        let samples = &buffer.samples()[..buffer.samples().len().min(remaining)];
        fingerprinter.consume(samples);
        samples_consumed += samples.len() as u64;
    }

    fingerprinter.finish();
    Ok(AudioFingerprint::new(fingerprinter.fingerprint().to_vec()))
}

pub fn are_duplicates(
    first: &AudioFingerprint,
    first_duration: Option<f64>,
    second: &AudioFingerprint,
    second_duration: Option<f64>,
) -> bool {
    if first.version != FINGERPRINT_VERSION || second.version != FINGERPRINT_VERSION {
        return false;
    }
    if durations_are_incompatible(first_duration, second_duration) {
        return false;
    }
    if first.values.is_empty() || second.values.is_empty() {
        return false;
    }
    if first.values == second.values {
        return true;
    }

    let configuration = Configuration::preset_test2();
    let available_match_seconds = configuration.item_duration_in_seconds()
        * first.values.len().min(second.values.len()) as f32;
    let required_match_seconds = (available_match_seconds * 0.6).clamp(3.0, 6.0);
    match_fingerprints(&first.values, &second.values, &configuration).is_ok_and(|segments| {
        segments.iter().any(|segment| {
            segment.score <= MAX_MATCH_SCORE
                && segment.duration(&configuration) >= required_match_seconds
        })
    })
}

fn durations_are_incompatible(first: Option<f64>, second: Option<f64>) -> bool {
    first.zip(second).is_some_and(|(first, second)| {
        let allowed =
            MAX_DURATION_DIFFERENCE_SECONDS.max(first.min(second) * MAX_DURATION_DIFFERENCE_RATIO);
        (first - second).abs() > allowed
    })
}

fn fingerprint_error(path: &Path, error: SymphoniaError) -> AppError {
    AppError::AudioFingerprint {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_equivalent_fingerprints_with_small_lossy_changes() {
        let original = synthetic_fingerprint(0);
        let lossy = synthetic_fingerprint(48);

        assert!(are_duplicates(&original, Some(45.0), &lossy, Some(45.1)));
    }

    #[test]
    fn rejects_matching_sections_when_total_durations_are_different() {
        let fingerprint = synthetic_fingerprint(0);

        assert!(!are_duplicates(
            &fingerprint,
            Some(45.0),
            &fingerprint,
            Some(60.0)
        ));
    }

    fn synthetic_fingerprint(quantization: i16) -> AudioFingerprint {
        let sample_rate = 44_100_u32;
        let channels = 2_u32;
        let mut samples = Vec::with_capacity(sample_rate as usize * channels as usize * 45);
        for frame in 0..sample_rate as usize * 45 {
            let time = frame as f64 / sample_rate as f64;
            let section = (time / 3.0).floor() as usize;
            let frequency = [196.0, 246.94, 293.66, 392.0][section % 4];
            let value = ((std::f64::consts::TAU * frequency * time).sin() * 24_000.0) as i16;
            let value = if quantization == 0 {
                value
            } else {
                value / quantization * quantization
            };
            samples.extend_from_slice(&[value, value]);
        }
        let mut fingerprinter = Fingerprinter::new(&Configuration::preset_test2());
        fingerprinter.start(sample_rate, channels).unwrap();
        fingerprinter.consume(&samples);
        fingerprinter.finish();
        AudioFingerprint::new(fingerprinter.fingerprint().to_vec())
    }
}
