use std::{fs, fs::File, path::Path};

use serde::{Deserialize, Serialize};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as SymphoniaError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};
use uuid::Uuid;

use crate::{error::AppError, project};

const CACHE_VERSION: u32 = 1;
const TARGET_BUCKETS: usize = 32_768;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformPeak {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformData {
    pub cache_version: u32,
    pub track_id: Uuid,
    pub duration_seconds: f64,
    pub peaks: Vec<WaveformPeak>,
}

pub fn load_or_generate(package_path: &Path, track_id: Uuid) -> Result<WaveformData, AppError> {
    let cache_path = package_path
        .join("Analysis")
        .join("waveform")
        .join(format!("{track_id}.json"));
    if let Ok(bytes) = fs::read(&cache_path) {
        if let Ok(cached) = serde_json::from_slice::<WaveformData>(&bytes) {
            if cached.cache_version == CACHE_VERSION && cached.track_id == track_id {
                return Ok(cached);
            }
        }
    }

    let media_path = project::track_media_path(package_path, track_id)?;
    let waveform = generate(&media_path, track_id)?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    let temporary = cache_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(&waveform)?;
    fs::write(&temporary, bytes).map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &cache_path).map_err(|error| AppError::io(&cache_path, error))?;
    Ok(waveform)
}

fn generate(path: &Path, track_id: Uuid) -> Result<WaveformData, AppError> {
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
        .map_err(|error| invalid_audio(path, error))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AppError::InvalidAudio {
            path: path.to_path_buf(),
            reason: "the file contains no default audio track".to_owned(),
        })?;
    let decoder_track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let expected_frames = track.codec_params.n_frames.unwrap_or(0) as usize;
    let frames_per_bucket = if expected_frames == 0 {
        2_048
    } else {
        expected_frames.div_ceil(TARGET_BUCKETS).max(1)
    };
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| invalid_audio(path, error))?;

    let mut peaks = Vec::with_capacity(TARGET_BUCKETS.min(expected_frames));
    let mut bucket_min = 1.0_f32;
    let mut bucket_max = -1.0_f32;
    let mut bucket_frames = 0_usize;
    let mut decoded_frames = 0_u64;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(error) => return Err(invalid_audio(path, error)),
        };
        if packet.track_id() != decoder_track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(invalid_audio(path, error)),
        };
        let channels = decoded.spec().channels.count();
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        samples.copy_interleaved_ref(decoded);
        for frame in samples.samples().chunks(channels) {
            for sample in frame {
                bucket_min = bucket_min.min(*sample);
                bucket_max = bucket_max.max(*sample);
            }
            bucket_frames += 1;
            decoded_frames += 1;
            if bucket_frames >= frames_per_bucket {
                peaks.push(WaveformPeak {
                    min: bucket_min,
                    max: bucket_max,
                });
                bucket_min = 1.0;
                bucket_max = -1.0;
                bucket_frames = 0;
            }
        }
    }
    if bucket_frames > 0 {
        peaks.push(WaveformPeak {
            min: bucket_min,
            max: bucket_max,
        });
    }
    if peaks.len() > TARGET_BUCKETS {
        peaks = reduce_peaks(&peaks, TARGET_BUCKETS);
    }

    Ok(WaveformData {
        cache_version: CACHE_VERSION,
        track_id,
        duration_seconds: decoded_frames as f64 / sample_rate as f64,
        peaks,
    })
}

fn reduce_peaks(source: &[WaveformPeak], target: usize) -> Vec<WaveformPeak> {
    let group_size = source.len().div_ceil(target);
    source
        .chunks(group_size)
        .map(|group| WaveformPeak {
            min: group.iter().map(|peak| peak.min).fold(1.0, f32::min),
            max: group.iter().map(|peak| peak.max).fold(-1.0, f32::max),
        })
        .collect()
}

fn invalid_audio(path: &Path, error: SymphoniaError) -> AppError {
    AppError::InvalidAudio {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}
