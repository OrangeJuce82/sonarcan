use std::sync::{mpsc, Arc};

use arc_swap::ArcSwap;
use rustfft::{num_complex::Complex32, FftPlanner};
use serde::Serialize;

use crate::audio_engine::{DecodedAudio, StemChannelGains, StemSet};
#[cfg(test)]
use crate::stem_contract::STEM_COUNT;

const FFT_SIZE: usize = 2_048;
const DISPLAY_BANDS: usize = 64;

struct SpectrumRequest {
    audio: Arc<DecodedAudio>,
    stems: Option<Arc<StemSet>>,
    stem_gains: StemChannelGains,
    position: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumFrame {
    pub bands: Vec<f32>,
    pub minimum_hz: f32,
    pub maximum_hz: f32,
}

pub struct SpectrumWorker {
    requests: mpsc::SyncSender<SpectrumRequest>,
    latest: Arc<ArcSwap<SpectrumFrame>>,
}

impl SpectrumWorker {
    pub fn new() -> Self {
        let (requests, receiver) = mpsc::sync_channel::<SpectrumRequest>(1);
        let latest = Arc::new(ArcSwap::from_pointee(SpectrumFrame {
            bands: vec![0.0; DISPLAY_BANDS],
            minimum_hz: 30.0,
            maximum_hz: 20_000.0,
        }));
        let worker_latest = Arc::clone(&latest);
        std::thread::Builder::new()
            .name("sonarcan-spectrum".to_owned())
            .spawn(move || run_worker(receiver, worker_latest))
            .expect("failed to start spectrum worker");
        Self { requests, latest }
    }

    pub fn request(
        &self,
        audio: Arc<DecodedAudio>,
        stems: Option<Arc<StemSet>>,
        stem_gains: StemChannelGains,
        position: usize,
    ) -> SpectrumFrame {
        let _ = self.requests.try_send(SpectrumRequest {
            audio,
            stems,
            stem_gains,
            position,
        });
        self.latest.load_full().as_ref().clone()
    }

    pub fn latest(&self) -> SpectrumFrame {
        self.latest.load_full().as_ref().clone()
    }
}

fn run_worker(receiver: mpsc::Receiver<SpectrumRequest>, latest: Arc<ArcSwap<SpectrumFrame>>) {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut buffer = vec![Complex32::default(); FFT_SIZE];
    while let Ok(request) = receiver.recv() {
        fill_window(&request, &mut buffer);
        fft.process(&mut buffer);
        latest.store(Arc::new(to_display_frame(&request, &buffer)));
    }
}

fn fill_window(request: &SpectrumRequest, buffer: &mut [Complex32]) {
    let start = request.position.saturating_sub(FFT_SIZE / 2);
    for (index, target) in buffer.iter_mut().enumerate() {
        let frame = (start + index).min(request.audio.frames.saturating_sub(1));
        let channels = visualization_channels(request);
        let mono = (0..channels)
            .map(|channel| visualization_sample(request, frame, channel))
            .sum::<f32>()
            / channels as f32;
        let window =
            0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (FFT_SIZE - 1) as f32).cos();
        *target = Complex32::new(mono * window, 0.0);
    }
}

fn to_display_frame(request: &SpectrumRequest, spectrum: &[Complex32]) -> SpectrumFrame {
    let sample_rate = request.audio.sample_rate;
    let nyquist = sample_rate as f32 / 2.0;
    let minimum = 30.0_f32;
    let maximum = nyquist.min(20_000.0).max(minimum);
    let mut bands = Vec::with_capacity(DISPLAY_BANDS);
    for band in 0..DISPLAY_BANDS {
        let frequency = minimum * (maximum / minimum).powf(band as f32 / DISPLAY_BANDS as f32);
        let next_frequency =
            minimum * (maximum / minimum).powf((band + 1) as f32 / DISPLAY_BANDS as f32);
        let first = ((frequency / sample_rate as f32) * FFT_SIZE as f32) as usize;
        let last = (((next_frequency / sample_rate as f32) * FFT_SIZE as f32).ceil() as usize)
            .max(first + 1)
            .min(FFT_SIZE / 2);
        let magnitude = spectrum[first.min(FFT_SIZE / 2 - 1)..last]
            .iter()
            .map(|value| value.norm())
            .fold(0.0_f32, f32::max)
            / FFT_SIZE as f32;
        let decibels = 20.0 * magnitude.max(1.0e-6).log10();
        bands.push(((decibels + 72.0) / 72.0).clamp(0.0, 1.0));
    }
    SpectrumFrame {
        bands,
        minimum_hz: minimum,
        maximum_hz: maximum,
    }
}

fn visualization_channels(request: &SpectrumRequest) -> usize {
    if request.stems.is_some() {
        2
    } else {
        request.audio.channels
    }
}

fn visualization_sample(request: &SpectrumRequest, frame: usize, output_channel: usize) -> f32 {
    let Some(stems) = request.stems.as_deref() else {
        let channel = output_channel.min(request.audio.channels.saturating_sub(1));
        return request.audio.samples[frame * request.audio.channels + channel];
    };

    let channel = output_channel.min(1);
    stems
        .stems
        .iter()
        .zip(request.stem_gains)
        .filter_map(|(stem, gains)| {
            let gain = gains[channel];
            if gain <= 0.0 {
                return None;
            }
            let stem_channel = output_channel.min(stem.channels.saturating_sub(1));
            Some(stem.samples[frame * stem.channels + stem_channel] * gain)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_places_a_tone_above_the_noise_floor() {
        let sample_rate = 48_000;
        let audio = DecodedAudio {
            samples: (0..FFT_SIZE)
                .map(|frame| {
                    (std::f32::consts::TAU * 1_000.0 * frame as f32 / sample_rate as f32).sin()
                })
                .collect(),
            channels: 1,
            sample_rate,
            frames: FFT_SIZE,
        };
        let request = SpectrumRequest {
            audio: Arc::new(audio),
            stems: None,
            stem_gains: [[1.0; 2]; STEM_COUNT],
            position: FFT_SIZE / 2,
        };
        let mut buffer = vec![Complex32::default(); FFT_SIZE];
        fill_window(&request, &mut buffer);
        FftPlanner::<f32>::new()
            .plan_fft_forward(FFT_SIZE)
            .process(&mut buffer);
        let frame = to_display_frame(&request, &buffer);
        assert_eq!(frame.bands.len(), DISPLAY_BANDS);
        assert!(frame.bands.iter().copied().fold(0.0_f32, f32::max) > 0.7);
    }

    #[test]
    fn visualization_uses_the_active_stem_mix_and_channel_gains() {
        let stereo = |left: f32, right: f32| {
            Arc::new(DecodedAudio {
                samples: (0..FFT_SIZE).flat_map(|_| [left, right]).collect(),
                sample_rate: 48_000,
                channels: 2,
                frames: FFT_SIZE,
            })
        };
        let mut stems = std::array::from_fn(|_| stereo(0.0, 0.0));
        stems[0] = stereo(0.8, 0.4);
        stems[1] = stereo(0.3, 0.6);
        let request = SpectrumRequest {
            audio: stereo(0.1, 0.2),
            stems: Some(Arc::new(StemSet { stems })),
            stem_gains: [[1.0, 0.5], [0.0, 0.0], [0.0; 2], [0.0; 2]],
            position: FFT_SIZE / 2,
        };

        assert!((visualization_sample(&request, 0, 0) - 0.8).abs() < f32::EPSILON);
        assert!((visualization_sample(&request, 0, 1) - 0.2).abs() < f32::EPSILON);
    }
}
