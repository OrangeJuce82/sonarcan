use std::{fs::File, path::Path};

use symphonia::core::{
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioMetadata {
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

pub fn probe(path: &Path) -> Result<AudioMetadata, AppError> {
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
        .map_err(|error| AppError::InvalidAudio {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

    let track = probed
        .format
        .default_track()
        .ok_or_else(|| AppError::InvalidAudio {
            path: path.to_path_buf(),
            reason: "the file contains no default audio track".to_owned(),
        })?;

    let parameters = &track.codec_params;
    let duration_seconds = match (parameters.n_frames, parameters.sample_rate) {
        (Some(frames), Some(sample_rate)) if sample_rate > 0 => {
            Some(frames as f64 / sample_rate as f64)
        }
        _ => parameters
            .time_base
            .zip(parameters.n_frames)
            .map(|(time_base, frames)| time_base.calc_time(frames).seconds as f64),
    };

    Ok(AudioMetadata {
        duration_seconds,
        sample_rate: parameters.sample_rate,
        channels: parameters.channels.map(|channels| channels.count() as u16),
    })
}
