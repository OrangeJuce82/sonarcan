use std::sync::{mpsc, Arc};

use arc_swap::ArcSwap;
use rustfft::{num_complex::Complex32, FftPlanner};
use serde::Serialize;

use crate::audio_engine::DecodedAudio;

const FFT_SIZE: usize = 2_048;
const DISPLAY_BANDS: usize = 64;

struct SpectrumRequest {
    audio: Arc<DecodedAudio>,
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

    pub fn request(&self, audio: Arc<DecodedAudio>, position: usize) -> SpectrumFrame {
        let _ = self.requests.try_send(SpectrumRequest { audio, position });
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
        latest.store(Arc::new(to_display_bands(
            &buffer,
            request.audio.sample_rate,
        )));
    }
}

fn fill_window(request: &SpectrumRequest, buffer: &mut [Complex32]) {
    let audio = &request.audio;
    let start = request.position.saturating_sub(FFT_SIZE / 2);
    for (index, target) in buffer.iter_mut().enumerate() {
        let frame = (start + index).min(audio.frames.saturating_sub(1));
        let mono = (0..audio.channels)
            .map(|channel| audio.samples[frame * audio.channels + channel])
            .sum::<f32>()
            / audio.channels as f32;
        let window =
            0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (FFT_SIZE - 1) as f32).cos();
        *target = Complex32::new(mono * window, 0.0);
    }
}

fn to_display_bands(spectrum: &[Complex32], sample_rate: u32) -> SpectrumFrame {
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
            position: FFT_SIZE / 2,
        };
        let mut buffer = vec![Complex32::default(); FFT_SIZE];
        fill_window(&request, &mut buffer);
        FftPlanner::<f32>::new()
            .plan_fft_forward(FFT_SIZE)
            .process(&mut buffer);
        let frame = to_display_bands(&buffer, sample_rate);
        assert_eq!(frame.bands.len(), DISPLAY_BANDS);
        assert!(frame.bands.iter().copied().fold(0.0_f32, f32::max) > 0.7);
    }
}
