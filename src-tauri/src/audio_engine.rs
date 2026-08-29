use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use arc_swap::ArcSwapOption;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
};
use serde::{Deserialize, Serialize};
use signalsmith_stretch::Stretch;
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as SymphoniaError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};
use tracing::info;

use crate::{error::AppError, spectrum, stem_contract::STEM_COUNT};

const NO_LOOP: u64 = u64::MAX;
const CROSSFADE_SECONDS: f64 = 0.005;
const GAIN_RAMP_SECONDS: f64 = 0.04;
const SEEK_TRANSITION_SECONDS: f64 = 0.008;
const MAX_DECODED_TRACKS: usize = 3;
const MAX_DECODED_CACHE_BYTES: usize = 384 * 1024 * 1024;
const MAX_DSP_OUTPUT_FRAMES: usize = 1_024;
const MAX_DSP_INPUT_FRAMES: usize = MAX_DSP_OUTPUT_FRAMES * 2 + 8;
const MAX_DSP_PREROLL_FRAMES: usize = 65_536;
const PCM_CACHE_MAGIC: &[u8; 8] = b"SACPCM01";

#[derive(Debug)]
pub(crate) struct DecodedAudio {
    pub(crate) samples: Vec<f32>,
    pub(crate) channels: usize,
    pub(crate) sample_rate: u32,
    pub(crate) frames: usize,
}

pub(crate) struct StemSet {
    pub(crate) stems: [Arc<DecodedAudio>; STEM_COUNT],
}

struct CachedAudio {
    audio: Arc<DecodedAudio>,
    file_size: u64,
    modified: Option<SystemTime>,
}

#[derive(Default)]
struct DecodeCache {
    entries: HashMap<PathBuf, CachedAudio>,
    recent: VecDeque<PathBuf>,
    loading: HashSet<PathBuf>,
}

struct SharedState {
    audio: ArcSwapOption<DecodedAudio>,
    stems: ArcSwapOption<StemSet>,
    stem_gain_bits: [AtomicU32; STEM_COUNT],
    stem_muted: [AtomicBool; STEM_COUNT],
    stem_soloed: [AtomicBool; STEM_COUNT],
    playing: AtomicBool,
    position_bits: AtomicU64,
    position_generation: AtomicU64,
    loop_a: AtomicU64,
    loop_b: AtomicU64,
    loop_cycle_armed: AtomicBool,
    loop_waiting_for_a: AtomicBool,
    volume_bits: AtomicU32,
    playback_rate_bits: AtomicU64,
    pitch_semitones_bits: AtomicU32,
    grid_bpm_bits: AtomicU64,
    beat_grid_offset_bits: AtomicU64,
    metronome_enabled: AtomicBool,
    metronome_volume_bits: AtomicU32,
    trainer_enabled: AtomicBool,
    trainer_repetitions: AtomicU32,
    trainer_increment_bits: AtomicU64,
    trainer_target_bits: AtomicU64,
    trainer_loop_count: AtomicU32,
    end_behavior: AtomicU32,
    ended_generation: AtomicU64,
    underruns: AtomicU64,
    output_peak_bits: AtomicU32,
    output_peak_left_bits: AtomicU32,
    output_peak_right_bits: AtomicU32,
}

pub struct AudioEngine {
    shared: Arc<SharedState>,
    decode_cache: (Mutex<DecodeCache>, Condvar),
    load_generation: AtomicU64,
    loaded_path: Mutex<Option<PathBuf>>,
    output_sample_rate: u32,
    output_channels: u16,
    spectrum: spectrum::SpectrumWorker,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub loaded: bool,
    pub playing: bool,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub output_sample_rate: u32,
    pub output_channels: u16,
    pub underruns: u64,
    pub output_peak: f32,
    pub output_peak_left: f32,
    pub output_peak_right: f32,
    pub playback_rate: f64,
    pub pitch_semitones: f32,
    pub grid_bpm: Option<f64>,
    pub beat_grid_offset_seconds: f64,
    pub metronome_enabled: bool,
    pub metronome_volume: f32,
    pub trainer_enabled: bool,
    pub trainer_repetitions: u32,
    pub trainer_increment: f64,
    pub trainer_target_rate: f64,
    pub trainer_loop_count: u32,
    pub end_behavior: EndBehavior,
    pub ended_generation: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EndBehavior {
    Restart,
    Advance,
    Stop,
}

impl EndBehavior {
    const fn code(self) -> u32 {
        match self {
            Self::Restart => 0,
            Self::Advance => 1,
            Self::Stop => 2,
        }
    }

    const fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Restart,
            1 => Self::Advance,
            _ => Self::Stop,
        }
    }
}

impl AudioEngine {
    pub fn new() -> Result<Self, AppError> {
        let shared = Arc::new(SharedState {
            audio: ArcSwapOption::empty(),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(false),
            position_bits: AtomicU64::new(0_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(0.8_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(false),
            trainer_repetitions: AtomicU32::new(3),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        });
        let thread_state = Arc::clone(&shared);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("sonarcan-audio".to_owned())
            .spawn(move || {
                let result = start_audio_thread(thread_state);
                match result {
                    Ok((stream, sample_rate, channels)) => {
                        let _ = ready_tx.send(Ok((sample_rate, channels)));
                        let _stream = stream;
                        loop {
                            std::thread::park();
                        }
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })
            .map_err(|error| AppError::AudioEngine(error.to_string()))?;
        let (output_sample_rate, output_channels) = ready_rx
            .recv()
            .map_err(|error| AppError::AudioEngine(error.to_string()))??;
        Ok(Self {
            shared,
            decode_cache: (Mutex::new(DecodeCache::default()), Condvar::new()),
            load_generation: AtomicU64::new(0),
            loaded_path: Mutex::new(None),
            output_sample_rate,
            output_channels,
            spectrum: spectrum::SpectrumWorker::new(),
        })
    }

    pub fn begin_load(&self) -> u64 {
        self.load_generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn load(&self, path: &Path, generation: u64) -> Result<AudioStatus, AppError> {
        let decoded = self.cached_or_decode(path)?;
        if generation != self.load_generation.load(Ordering::Acquire) {
            return Ok(self.status());
        }
        self.shared.playing.store(false, Ordering::Release);
        self.shared.stems.store(None);
        self.shared.audio.store(Some(decoded));
        if let Ok(mut loaded_path) = self.loaded_path.lock() {
            *loaded_path = Some(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
        }
        self.seek(0.0);
        self.clear_loop();
        Ok(self.status())
    }

    pub fn preload(&self, path: &Path) -> Result<(), AppError> {
        self.cached_or_decode(path).map(|_| ())
    }

    pub(crate) fn decoded_for_analysis(&self, path: &Path) -> Result<Arc<DecodedAudio>, AppError> {
        self.cached_or_decode(path)
    }

    fn cached_or_decode(&self, path: &Path) -> Result<Arc<DecodedAudio>, AppError> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let metadata = path
            .metadata()
            .map_err(|error| AppError::io(&path, error))?;
        let modified = metadata.modified().ok();
        let source_size = metadata.len();
        {
            let mut cache = self.decode_cache.0.lock().map_err(|_| {
                AppError::AudioEngine("decoded-audio cache is unavailable".to_owned())
            })?;
            loop {
                if let Some(entry) = cache.entries.get(&path) {
                    if entry.file_size == source_size && entry.modified == modified {
                        let audio = Arc::clone(&entry.audio);
                        touch_cache_entry(&mut cache.recent, &path);
                        return Ok(audio);
                    }
                }
                if !cache.loading.contains(&path) {
                    cache.loading.insert(path.clone());
                    break;
                }
                cache = self.decode_cache.1.wait(cache).map_err(|_| {
                    AppError::AudioEngine("decoded-audio cache is unavailable".to_owned())
                })?;
            }
        }

        let load_started = Instant::now();
        let decoded = if let Some(cached) = load_decoded_cache(&path, source_size, modified) {
            info!(path = %path.display(), elapsed_ms = load_started.elapsed().as_millis(), "loaded persistent PCM cache");
            cached
        } else {
            let cache_audio = Arc::new(decode(&path)?);
            info!(path = %path.display(), elapsed_ms = load_started.elapsed().as_millis(), "decoded compressed audio");
            let write_audio = Arc::clone(&cache_audio);
            let write_path = path.clone();
            std::thread::Builder::new()
                .name("sonarcan-pcm-cache".to_owned())
                .spawn(move || {
                    let _ = store_decoded_cache(&write_path, source_size, modified, &write_audio);
                })
                .ok();
            cache_audio
        };
        let mut cache =
            self.decode_cache.0.lock().map_err(|_| {
                AppError::AudioEngine("decoded-audio cache is unavailable".to_owned())
            })?;
        cache.loading.remove(&path);
        self.decode_cache.1.notify_all();
        let audio = decoded;
        cache.entries.insert(
            path.clone(),
            CachedAudio {
                audio: Arc::clone(&audio),
                file_size: source_size,
                modified,
            },
        );
        touch_cache_entry(&mut cache.recent, &path);
        while (cache.recent.len() > MAX_DECODED_TRACKS
            || decoded_cache_bytes(&cache) > MAX_DECODED_CACHE_BYTES)
            && cache.recent.len() > 1
        {
            if let Some(expired) = cache.recent.pop_back() {
                cache.entries.remove(&expired);
            }
        }
        Ok(audio)
    }

    pub fn play(&self) {
        if self.shared.audio.load().is_some() {
            let loop_a = self.shared.loop_a.load(Ordering::Acquire);
            let loop_b = self.shared.loop_b.load(Ordering::Acquire);
            let valid_loop = loop_a != NO_LOOP && loop_b > loop_a;
            let position = f64::from_bits(self.shared.position_bits.load(Ordering::Acquire));
            if valid_loop {
                self.shared
                    .position_generation
                    .fetch_add(1, Ordering::AcqRel);
                if position >= loop_b as f64 {
                    self.shared
                        .position_bits
                        .store((loop_a as f64).to_bits(), Ordering::Release);
                    self.shared.loop_cycle_armed.store(true, Ordering::Release);
                    self.shared
                        .loop_waiting_for_a
                        .store(false, Ordering::Release);
                } else {
                    set_loop_cycle_state_for_position(&self.shared, position);
                }
            }
            self.shared.playing.store(true, Ordering::Release);
        }
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Release);
    }

    pub fn seek(&self, seconds: f64) {
        let Some(audio) = self.shared.audio.load_full() else {
            return;
        };
        let frame = (seconds.max(0.0) * audio.sample_rate as f64).min(audio.frames as f64);
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
        self.shared
            .position_bits
            .store(frame.to_bits(), Ordering::Release);
        set_loop_cycle_state_for_position(&self.shared, frame);
    }

    pub fn set_loop(&self, a_seconds: Option<f64>, b_seconds: Option<f64>) {
        let Some(audio) = self.shared.audio.load_full() else {
            return;
        };
        let to_frame = |seconds: f64| (seconds.max(0.0) * audio.sample_rate as f64) as u64;
        self.shared.loop_a.store(
            a_seconds.map(to_frame).unwrap_or(NO_LOOP),
            Ordering::Release,
        );
        self.shared.loop_b.store(
            b_seconds.map(to_frame).unwrap_or(NO_LOOP),
            Ordering::Release,
        );
        let position = f64::from_bits(self.shared.position_bits.load(Ordering::Acquire));
        self.shared.trainer_loop_count.store(0, Ordering::Release);
        set_loop_cycle_state_for_position(&self.shared, position);
    }

    pub fn clear_loop(&self) {
        self.shared.loop_a.store(NO_LOOP, Ordering::Release);
        self.shared.loop_b.store(NO_LOOP, Ordering::Release);
        self.shared.loop_cycle_armed.store(false, Ordering::Release);
        self.shared
            .loop_waiting_for_a
            .store(false, Ordering::Release);
    }

    pub fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    }

    pub(crate) fn activate_stems(
        &self,
        source_path: &Path,
        stems: [Arc<DecodedAudio>; STEM_COUNT],
    ) -> Result<(), AppError> {
        let expected = source_path
            .canonicalize()
            .unwrap_or_else(|_| source_path.to_path_buf());
        if self
            .loaded_path
            .lock()
            .map_err(|_| AppError::AudioEngine("loaded-track identity is unavailable".into()))?
            .as_ref()
            != Some(&expected)
        {
            return Err(AppError::AudioEngine(
                "the separated track is no longer selected".into(),
            ));
        }
        let Some(audio) = self.shared.audio.load_full() else {
            return Err(AppError::AudioEngine(
                "cannot activate stems without a loaded track".to_owned(),
            ));
        };
        if stems
            .iter()
            .any(|stem| stem.sample_rate != audio.sample_rate || stem.frames != audio.frames)
        {
            return Err(AppError::AudioEngine(
                "stem cache does not match the loaded track".to_owned(),
            ));
        }
        self.shared.stems.store(Some(Arc::new(StemSet { stems })));
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    pub fn disable_stems(&self) {
        self.shared.stems.store(None);
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
    }

    pub fn set_stem_mix(&self, index: usize, gain: f32, muted: bool, soloed: bool) {
        if index >= STEM_COUNT {
            return;
        }
        self.shared.stem_gain_bits[index].store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Release);
        self.shared.stem_muted[index].store(muted, Ordering::Release);
        self.shared.stem_soloed[index].store(soloed, Ordering::Release);
    }

    pub fn set_playback_rate(&self, rate: f64) {
        self.shared
            .playback_rate_bits
            .store(rate.clamp(0.5, 2.0).to_bits(), Ordering::Release);
    }

    pub fn set_pitch_semitones(&self, semitones: f32) {
        self.shared
            .pitch_semitones_bits
            .store(semitones.clamp(-12.0, 12.0).to_bits(), Ordering::Release);
    }

    pub fn set_beat_grid(&self, bpm: Option<f64>, offset_seconds: f64) {
        self.shared.grid_bpm_bits.store(
            bpm.filter(|value| value.is_finite())
                .map(|value| value.clamp(30.0, 300.0))
                .unwrap_or(0.0)
                .to_bits(),
            Ordering::Release,
        );
        self.shared
            .beat_grid_offset_bits
            .store(offset_seconds.max(0.0).to_bits(), Ordering::Release);
    }

    pub fn set_metronome(&self, enabled: bool, volume: f32) {
        self.shared
            .metronome_enabled
            .store(enabled, Ordering::Release);
        self.shared
            .metronome_volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    }

    pub fn set_loop_trainer(
        &self,
        enabled: bool,
        repetitions: u32,
        increment: f64,
        target_rate: f64,
    ) {
        let current = f64::from_bits(self.shared.playback_rate_bits.load(Ordering::Acquire));
        let target_rate = target_rate.clamp(0.5, 2.0);
        self.shared
            .trainer_enabled
            .store(enabled && target_rate > current, Ordering::Release);
        self.shared
            .trainer_repetitions
            .store(repetitions.clamp(1, 99), Ordering::Release);
        self.shared
            .trainer_increment_bits
            .store(increment.clamp(0.01, 0.25).to_bits(), Ordering::Release);
        self.shared
            .trainer_target_bits
            .store(target_rate.to_bits(), Ordering::Release);
        self.shared.trainer_loop_count.store(0, Ordering::Release);
        let position = f64::from_bits(self.shared.position_bits.load(Ordering::Acquire));
        set_loop_cycle_state_for_position(&self.shared, position);
    }

    pub fn set_end_behavior(&self, behavior: EndBehavior) {
        self.shared
            .end_behavior
            .store(behavior.code(), Ordering::Release);
    }

    pub fn status(&self) -> AudioStatus {
        let audio = self.shared.audio.load_full();
        let (position_seconds, duration_seconds) = audio.as_ref().map_or((0.0, 0.0), |audio| {
            (
                f64::from_bits(self.shared.position_bits.load(Ordering::Acquire))
                    / audio.sample_rate as f64,
                audio.frames as f64 / audio.sample_rate as f64,
            )
        });
        AudioStatus {
            loaded: audio.is_some(),
            playing: self.shared.playing.load(Ordering::Acquire),
            position_seconds,
            duration_seconds,
            output_sample_rate: self.output_sample_rate,
            output_channels: self.output_channels,
            underruns: self.shared.underruns.load(Ordering::Relaxed),
            output_peak: f32::from_bits(self.shared.output_peak_bits.load(Ordering::Relaxed)),
            output_peak_left: f32::from_bits(
                self.shared.output_peak_left_bits.load(Ordering::Relaxed),
            ),
            output_peak_right: f32::from_bits(
                self.shared.output_peak_right_bits.load(Ordering::Relaxed),
            ),
            playback_rate: f64::from_bits(self.shared.playback_rate_bits.load(Ordering::Acquire)),
            pitch_semitones: f32::from_bits(
                self.shared.pitch_semitones_bits.load(Ordering::Acquire),
            ),
            grid_bpm: match f64::from_bits(self.shared.grid_bpm_bits.load(Ordering::Acquire)) {
                bpm if bpm > 0.0 => Some(bpm),
                _ => None,
            },
            beat_grid_offset_seconds: f64::from_bits(
                self.shared.beat_grid_offset_bits.load(Ordering::Acquire),
            ),
            metronome_enabled: self.shared.metronome_enabled.load(Ordering::Acquire),
            metronome_volume: f32::from_bits(
                self.shared.metronome_volume_bits.load(Ordering::Acquire),
            ),
            trainer_enabled: self.shared.trainer_enabled.load(Ordering::Acquire),
            trainer_repetitions: self.shared.trainer_repetitions.load(Ordering::Acquire),
            trainer_increment: f64::from_bits(
                self.shared.trainer_increment_bits.load(Ordering::Acquire),
            ),
            trainer_target_rate: f64::from_bits(
                self.shared.trainer_target_bits.load(Ordering::Acquire),
            ),
            trainer_loop_count: self.shared.trainer_loop_count.load(Ordering::Acquire),
            end_behavior: EndBehavior::from_code(self.shared.end_behavior.load(Ordering::Acquire)),
            ended_generation: self.shared.ended_generation.load(Ordering::Acquire),
        }
    }

    pub fn spectrum(&self) -> spectrum::SpectrumFrame {
        let Some(audio) = self.shared.audio.load_full() else {
            return self.spectrum.latest();
        };
        let position = f64::from_bits(self.shared.position_bits.load(Ordering::Acquire)) as usize;
        self.spectrum.request(audio, position)
    }
}

fn touch_cache_entry(recent: &mut VecDeque<PathBuf>, path: &Path) {
    recent.retain(|entry| entry != path);
    recent.push_front(path.to_path_buf());
}

fn decoded_cache_bytes(cache: &DecodeCache) -> usize {
    cache
        .entries
        .values()
        .map(|entry| entry.audio.samples.len() * size_of::<f32>())
        .sum()
}

fn start_audio_thread(shared: Arc<SharedState>) -> Result<(Stream, u32, u16), AppError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| {
        AppError::AudioEngine("no default audio output device is available".to_owned())
    })?;
    let supported = device
        .default_output_config()
        .map_err(|error| AppError::AudioEngine(error.to_string()))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let stream = build_stream(&device, &config, sample_format, shared)?;
    stream
        .play()
        .map_err(|error| AppError::AudioEngine(error.to_string()))?;
    Ok((stream, config.sample_rate.0, config.channels))
}

fn build_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    shared: Arc<SharedState>,
) -> Result<Stream, AppError> {
    let result = match format {
        SampleFormat::F32 => build_typed_stream::<f32>(device, config, shared),
        SampleFormat::I16 => build_typed_stream::<i16>(device, config, shared),
        SampleFormat::U16 => build_typed_stream::<u16>(device, config, shared),
        other => {
            return Err(AppError::AudioEngine(format!(
                "unsupported output sample format: {other}"
            )))
        }
    };
    result.map_err(|error| AppError::AudioEngine(error.to_string()))
}

fn build_typed_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<SharedState>,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let output_rate = config.sample_rate.0;
    let error_state = Arc::clone(&shared);
    let mut renderer = RealtimeRenderer::new(channels, output_rate);
    device.build_output_stream(
        config,
        move |output: &mut [T], _| renderer.render(output, &shared),
        move |_| {
            error_state.underruns.fetch_add(1, Ordering::Relaxed);
            error_state.playing.store(false, Ordering::Release);
        },
        None,
    )
}

struct RealtimeRenderer {
    channels: usize,
    output_rate: u32,
    stretch: Stretch,
    input: Vec<f32>,
    processed: Vec<f32>,
    preroll: Vec<f32>,
    rate_remainder: f64,
    last_generation: u64,
    last_audio: usize,
    last_pitch: f32,
    smoothed_rate: f64,
    smoothed_pitch: f32,
    smoothed_volume: f32,
    smoothed_stem_gains: [f32; STEM_COUNT],
    gains_initialized: bool,
    dsp_active: bool,
    seek_transition: SeekTransition,
}

struct SeekTransition {
    generation: u64,
    total_frames: u32,
    remaining_frames: u32,
    anchor: Vec<f32>,
    last_output: Vec<f32>,
}

impl SeekTransition {
    fn new(channels: usize, output_rate: u32) -> Self {
        Self {
            generation: u64::MAX,
            total_frames: (output_rate as f64 * SEEK_TRANSITION_SECONDS)
                .round()
                .max(1.0) as u32,
            remaining_frames: 0,
            anchor: vec![0.0; channels],
            last_output: vec![0.0; channels],
        }
    }

    fn begin_frame(&mut self, generation: u64) -> f32 {
        if generation != self.generation {
            self.generation = generation;
            self.anchor.copy_from_slice(&self.last_output);
            self.remaining_frames = self.total_frames;
        }
        if self.remaining_frames == 0 {
            1.0
        } else {
            (self.total_frames - self.remaining_frames) as f32 / self.total_frames as f32
        }
    }

    fn smooth_sample(&mut self, channel: usize, value: f32, blend: f32) -> f32 {
        let smoothed = self.anchor[channel] + (value - self.anchor[channel]) * blend;
        self.last_output[channel] = smoothed;
        smoothed
    }

    fn end_frame(&mut self) {
        self.remaining_frames = self.remaining_frames.saturating_sub(1);
    }

    fn clear_output(&mut self) {
        self.anchor.fill(0.0);
        self.last_output.fill(0.0);
        self.remaining_frames = 0;
    }
}

impl RealtimeRenderer {
    fn new(channels: usize, output_rate: u32) -> Self {
        let mut stretch = Stretch::preset_default(channels as u32, output_rate);
        let mut input = vec![0.0; MAX_DSP_INPUT_FRAMES * channels];
        let mut processed = vec![0.0; MAX_DSP_OUTPUT_FRAMES * channels];
        stretch.process(&input, &mut processed);
        stretch.reset();
        input.fill(0.0);
        processed.fill(0.0);
        Self {
            channels,
            output_rate,
            stretch,
            input,
            processed,
            preroll: vec![0.0; MAX_DSP_PREROLL_FRAMES * channels],
            rate_remainder: 0.0,
            last_generation: u64::MAX,
            last_audio: 0,
            last_pitch: f32::NAN,
            smoothed_rate: 1.0,
            smoothed_pitch: 0.0,
            smoothed_volume: 0.8,
            smoothed_stem_gains: [1.0; STEM_COUNT],
            gains_initialized: false,
            dsp_active: false,
            seek_transition: SeekTransition::new(channels, output_rate),
        }
    }

    fn render<T>(&mut self, output: &mut [T], shared: &SharedState)
    where
        T: SizedSample + FromSample<f32>,
    {
        let target_rate = f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire));
        let target_pitch = f32::from_bits(shared.pitch_semitones_bits.load(Ordering::Acquire));
        let frames = output.len() / self.channels;
        let block_seconds = frames as f64 / self.output_rate as f64;
        let blend = 1.0 - (-block_seconds / GAIN_RAMP_SECONDS).exp();
        let target_volume = f32::from_bits(shared.volume_bits.load(Ordering::Relaxed));
        let target_stem_gains = target_stem_gains(shared);
        if !self.gains_initialized {
            self.smoothed_volume = target_volume;
            self.smoothed_stem_gains = target_stem_gains;
            self.gains_initialized = true;
        } else {
            smooth_gain(&mut self.smoothed_volume, target_volume, blend as f32);
            for (current, target) in self.smoothed_stem_gains.iter_mut().zip(target_stem_gains) {
                smooth_gain(current, target, blend as f32);
            }
        }
        self.smoothed_rate += (target_rate - self.smoothed_rate) * blend;
        self.smoothed_pitch += (target_pitch - self.smoothed_pitch) * blend as f32;
        if (target_rate - self.smoothed_rate).abs() < 0.000_05 {
            self.smoothed_rate = target_rate;
        }
        if (target_pitch - self.smoothed_pitch).abs() < 0.000_5 {
            self.smoothed_pitch = target_pitch;
        }
        if (self.smoothed_rate - 1.0).abs() < 0.000_001 && self.smoothed_pitch.abs() < 0.000_1 {
            self.dsp_active = false;
            let smoothed_stem_gains = self.smoothed_stem_gains;
            render_with_gains(
                output,
                self.channels,
                self.output_rate,
                shared,
                self.smoothed_volume,
                &smoothed_stem_gains,
                Some(&mut self.seek_transition),
            );
            return;
        }
        let smoothed_stem_gains = self.smoothed_stem_gains;
        self.render_stretched(
            output,
            shared,
            self.smoothed_rate,
            self.smoothed_pitch,
            self.smoothed_volume,
            &smoothed_stem_gains,
        );
    }

    fn render_stretched<T>(
        &mut self,
        output: &mut [T],
        shared: &SharedState,
        rate: f64,
        pitch: f32,
        volume: f32,
        stem_gains: &[f32; STEM_COUNT],
    ) where
        T: SizedSample + FromSample<f32>,
    {
        let silence = T::from_sample(0.0);
        output.fill(silence);
        if !shared.playing.load(Ordering::Acquire) {
            self.seek_transition.clear_output();
            publish_output_peak(shared, 0.0);
            return;
        }
        let Some(audio) = shared.audio.load_full() else {
            self.seek_transition.clear_output();
            publish_output_peak(shared, 0.0);
            return;
        };
        let generation = shared.position_generation.load(Ordering::Acquire);
        let audio_pointer = Arc::as_ptr(&audio) as usize;
        let mut position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
        let source_step = audio.sample_rate as f64 / self.output_rate as f64;
        let loop_a = shared.loop_a.load(Ordering::Acquire);
        let loop_b = shared.loop_b.load(Ordering::Acquire);
        let valid_loop = loop_a != NO_LOOP && loop_b != NO_LOOP && loop_b > loop_a;
        let full_track_training = !valid_loop && shared.trainer_enabled.load(Ordering::Acquire);
        let restart_at_end = EndBehavior::from_code(shared.end_behavior.load(Ordering::Acquire))
            == EndBehavior::Restart;
        let repeat_full_track = full_track_training || (!valid_loop && restart_at_end);
        let needs_reset = !self.dsp_active
            || generation != self.last_generation
            || audio_pointer != self.last_audio;
        let pitch_changed = (pitch - self.last_pitch).abs() >= 0.000_1;
        if needs_reset {
            self.reset_at(
                shared,
                &audio,
                position,
                source_step,
                rate,
                loop_a,
                loop_b,
                valid_loop,
                stem_gains,
            );
        }
        // reset() restores Signalsmith's configured defaults, including a
        // neutral transposition. Always apply pitch after a reset.
        if pitch_changed || needs_reset {
            self.stretch.set_transpose_factor_semitones(pitch, None);
            self.last_pitch = pitch;
        }
        self.dsp_active = true;
        self.last_generation = generation;
        self.last_audio = audio_pointer;

        let mut output_peak = 0.0_f32;
        let mut channel_peaks = [0.0_f32; 2];
        for output_chunk in output.chunks_mut(MAX_DSP_OUTPUT_FRAMES * self.channels) {
            let output_frames = output_chunk.len() / self.channels;
            let output_start_position = position;
            let exact_input = output_frames as f64 * rate + self.rate_remainder;
            let input_frames = (exact_input.floor() as usize).min(MAX_DSP_INPUT_FRAMES);
            self.rate_remainder = exact_input - input_frames as f64;
            for frame in 0..input_frames {
                if valid_loop {
                    process_loop_boundary(shared, &mut position, loop_a, loop_b);
                }
                if position >= audio.frames as f64 {
                    if repeat_full_track {
                        if full_track_training {
                            handle_loop_trainer(shared);
                        }
                        position %= audio.frames as f64;
                    } else {
                        signal_advance_at_end(shared);
                        shared.playing.store(false, Ordering::Release);
                        return;
                    }
                }
                for channel in 0..self.channels {
                    // This is the only DSP path: select/mix the six stems at
                    // the source position first, then feed that single mix to
                    // Signalsmith once for combined tempo and pitch changes.
                    self.input[frame * self.channels + channel] = playback_sample(
                        shared,
                        &audio,
                        position,
                        channel,
                        if repeat_full_track { 0 } else { loop_a },
                        if repeat_full_track {
                            audio.frames as u64
                        } else {
                            loop_b
                        },
                        valid_loop || repeat_full_track,
                        stem_gains,
                    );
                }
                position += source_step;
            }
            let input_samples = input_frames * self.channels;
            let output_samples = output_frames * self.channels;
            self.stretch.process(
                &self.input[..input_samples],
                &mut self.processed[..output_samples],
            );
            for (frame_index, frame) in output_chunk.chunks_mut(self.channels).enumerate() {
                let seek_blend = self.seek_transition.begin_frame(generation);
                let metronome_position = wrap_loop_position(
                    output_start_position + frame_index as f64 * source_step * rate,
                    loop_a,
                    loop_b,
                    valid_loop,
                );
                let click = metronome_sample(metronome_position, audio.sample_rate, rate, shared);
                for (channel, target) in frame.iter_mut().enumerate() {
                    let value = apply_master_gain(
                        self.processed[frame_index * self.channels + channel],
                        click,
                        volume,
                    );
                    let value = self
                        .seek_transition
                        .smooth_sample(channel, value, seek_blend);
                    output_peak = output_peak.max(value.abs());
                    if channel < channel_peaks.len() {
                        channel_peaks[channel] = channel_peaks[channel].max(value.abs());
                    }
                    *target = T::from_sample(value);
                }
                self.seek_transition.end_frame();
            }
        }
        if generation == shared.position_generation.load(Ordering::Acquire) {
            shared
                .position_bits
                .store(position.to_bits(), Ordering::Release);
        }
        publish_output_peaks(
            shared,
            output_peak,
            channel_peaks[0],
            if self.channels > 1 {
                channel_peaks[1]
            } else {
                channel_peaks[0]
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn reset_at(
        &mut self,
        shared: &SharedState,
        audio: &DecodedAudio,
        position: f64,
        source_step: f64,
        rate: f64,
        loop_a: u64,
        loop_b: u64,
        valid_loop: bool,
        stem_gains: &[f32; STEM_COUNT],
    ) {
        self.stretch.reset();
        self.rate_remainder = 0.0;
        let requested = self.stretch.input_latency()
            + (rate * self.stretch.output_latency() as f64).ceil() as usize;
        let frames = requested.clamp(1, MAX_DSP_PREROLL_FRAMES);
        let mut preroll_position = position - frames as f64 * source_step;
        let preroll_can_wrap = valid_loop && position >= loop_a as f64;
        for frame in 0..frames {
            if preroll_can_wrap {
                while preroll_position < loop_a as f64 {
                    preroll_position += (loop_b - loop_a) as f64;
                }
            } else {
                preroll_position = preroll_position.max(0.0);
            }
            for channel in 0..self.channels {
                self.preroll[frame * self.channels + channel] = playback_sample(
                    shared,
                    audio,
                    preroll_position,
                    channel,
                    loop_a,
                    loop_b,
                    valid_loop,
                    stem_gains,
                );
            }
            preroll_position += source_step;
        }
        self.stretch
            .seek(&self.preroll[..frames * self.channels], rate);
    }
}

fn wrap_loop_position(position: f64, loop_a: u64, loop_b: u64, valid_loop: bool) -> f64 {
    if valid_loop && position >= loop_b as f64 {
        loop_a as f64 + (position - loop_b as f64) % (loop_b - loop_a) as f64
    } else {
        position
    }
}

fn set_loop_cycle_state_for_position(shared: &SharedState, position: f64) {
    let loop_a = shared.loop_a.load(Ordering::Acquire);
    let loop_b = shared.loop_b.load(Ordering::Acquire);
    let valid_loop = loop_a != NO_LOOP && loop_b > loop_a;
    let at_loop_start = valid_loop && (position - loop_a as f64).abs() < f64::EPSILON;
    let before_loop = valid_loop && position < loop_a as f64;
    shared
        .loop_cycle_armed
        .store(at_loop_start, Ordering::Release);
    shared
        .loop_waiting_for_a
        .store(before_loop, Ordering::Release);
}

fn process_loop_boundary(shared: &SharedState, position: &mut f64, loop_a: u64, loop_b: u64) {
    if loop_a == NO_LOOP || loop_b <= loop_a {
        return;
    }
    if shared.loop_waiting_for_a.load(Ordering::Relaxed) && *position >= loop_a as f64 {
        shared.loop_waiting_for_a.store(false, Ordering::Relaxed);
        shared.loop_cycle_armed.store(true, Ordering::Relaxed);
    }
    if *position < loop_b as f64 {
        return;
    }
    if shared.loop_cycle_armed.load(Ordering::Relaxed) {
        handle_loop_trainer(shared);
    }
    *position = wrap_loop_position(*position, loop_a, loop_b, true);
    shared.loop_cycle_armed.store(true, Ordering::Relaxed);
    shared.loop_waiting_for_a.store(false, Ordering::Relaxed);
}

#[cfg(test)]
fn render<T>(output: &mut [T], output_channels: usize, output_rate: u32, shared: &SharedState)
where
    T: SizedSample + FromSample<f32>,
{
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::Relaxed));
    let stem_gains = target_stem_gains(shared);
    render_with_gains(
        output,
        output_channels,
        output_rate,
        shared,
        volume,
        &stem_gains,
        None,
    );
}

fn render_with_gains<T>(
    output: &mut [T],
    output_channels: usize,
    output_rate: u32,
    shared: &SharedState,
    volume: f32,
    stem_gains: &[f32; STEM_COUNT],
    mut seek_transition: Option<&mut SeekTransition>,
) where
    T: SizedSample + FromSample<f32>,
{
    let silence = T::from_sample(0.0);
    output.fill(silence);
    if !shared.playing.load(Ordering::Acquire) {
        if let Some(transition) = seek_transition.as_deref_mut() {
            transition.clear_output();
        }
        publish_output_peak(shared, 0.0);
        return;
    }
    let Some(audio) = shared.audio.load_full() else {
        if let Some(transition) = seek_transition.as_deref_mut() {
            transition.clear_output();
        }
        publish_output_peak(shared, 0.0);
        return;
    };
    let generation = shared.position_generation.load(Ordering::Acquire);
    let mut position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
    let step = audio.sample_rate as f64 / output_rate as f64;
    let loop_a = shared.loop_a.load(Ordering::Acquire);
    let loop_b = shared.loop_b.load(Ordering::Acquire);
    let valid_loop = loop_a != NO_LOOP && loop_b != NO_LOOP && loop_b > loop_a;
    let full_track_training = !valid_loop && shared.trainer_enabled.load(Ordering::Acquire);
    let restart_at_end =
        EndBehavior::from_code(shared.end_behavior.load(Ordering::Acquire)) == EndBehavior::Restart;
    let repeat_full_track = full_track_training || (!valid_loop && restart_at_end);

    let mut output_peak = 0.0_f32;
    let mut channel_peaks = [0.0_f32; 2];
    for frame in output.chunks_mut(output_channels) {
        let seek_blend = seek_transition
            .as_deref_mut()
            .map_or(1.0, |transition| transition.begin_frame(generation));
        if valid_loop {
            process_loop_boundary(shared, &mut position, loop_a, loop_b);
        }
        if position >= audio.frames as f64 {
            if repeat_full_track {
                if full_track_training {
                    handle_loop_trainer(shared);
                }
                position %= audio.frames as f64;
            } else {
                signal_advance_at_end(shared);
                shared.playing.store(false, Ordering::Release);
                break;
            }
        }
        let click = metronome_sample(position, audio.sample_rate, 1.0, shared);
        for (channel, sample) in frame.iter_mut().enumerate() {
            let value = playback_sample(
                shared,
                &audio,
                position,
                channel,
                if repeat_full_track { 0 } else { loop_a },
                if repeat_full_track {
                    audio.frames as u64
                } else {
                    loop_b
                },
                valid_loop || repeat_full_track,
                stem_gains,
            );
            let mut output_value = apply_master_gain(value, click, volume);
            if let Some(transition) = seek_transition.as_deref_mut() {
                output_value = transition.smooth_sample(channel, output_value, seek_blend);
            }
            output_peak = output_peak.max(output_value.abs());
            if channel < channel_peaks.len() {
                channel_peaks[channel] = channel_peaks[channel].max(output_value.abs());
            }
            *sample = T::from_sample(output_value);
        }
        if let Some(transition) = seek_transition.as_deref_mut() {
            transition.end_frame();
        }
        position += step;
    }
    if generation == shared.position_generation.load(Ordering::Acquire) {
        shared
            .position_bits
            .store(position.to_bits(), Ordering::Release);
    }
    publish_output_peaks(
        shared,
        output_peak,
        channel_peaks[0],
        if output_channels > 1 {
            channel_peaks[1]
        } else {
            channel_peaks[0]
        },
    );
}

fn publish_output_peak(shared: &SharedState, block_peak: f32) {
    publish_output_peaks(shared, block_peak, block_peak, block_peak);
}

fn publish_output_peaks(shared: &SharedState, block_peak: f32, left_peak: f32, right_peak: f32) {
    let previous = f32::from_bits(shared.output_peak_bits.load(Ordering::Relaxed));
    shared
        .output_peak_bits
        .store(block_peak.max(previous * 0.86).to_bits(), Ordering::Relaxed);
    let previous_left = f32::from_bits(shared.output_peak_left_bits.load(Ordering::Relaxed));
    shared.output_peak_left_bits.store(
        left_peak.max(previous_left * 0.86).to_bits(),
        Ordering::Relaxed,
    );
    let previous_right = f32::from_bits(shared.output_peak_right_bits.load(Ordering::Relaxed));
    shared.output_peak_right_bits.store(
        right_peak.max(previous_right * 0.86).to_bits(),
        Ordering::Relaxed,
    );
}

fn apply_master_gain(music: f32, metronome: f32, volume: f32) -> f32 {
    ((music + metronome) * volume).clamp(-1.0, 1.0)
}

fn smooth_gain(current: &mut f32, target: f32, blend: f32) {
    *current += (target - *current) * blend.clamp(0.0, 1.0);
    if (*current - target).abs() < 0.000_5 {
        *current = target;
    }
}

fn target_stem_gains(shared: &SharedState) -> [f32; STEM_COUNT] {
    let any_solo = shared
        .stem_soloed
        .iter()
        .any(|value| value.load(Ordering::Relaxed));
    std::array::from_fn(|index| {
        let muted = shared.stem_muted[index].load(Ordering::Relaxed);
        let soloed = shared.stem_soloed[index].load(Ordering::Relaxed);
        if muted || (any_solo && !soloed) {
            0.0
        } else {
            f32::from_bits(shared.stem_gain_bits[index].load(Ordering::Relaxed))
        }
    })
}

fn handle_loop_trainer(shared: &SharedState) {
    if !shared.trainer_enabled.load(Ordering::Relaxed) {
        return;
    }
    let repetitions = shared.trainer_repetitions.load(Ordering::Relaxed).max(1);
    let completed = shared.trainer_loop_count.fetch_add(1, Ordering::AcqRel) + 1;
    if completed < repetitions {
        return;
    }
    shared.trainer_loop_count.store(0, Ordering::Release);
    let current = f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire));
    let increment = f64::from_bits(shared.trainer_increment_bits.load(Ordering::Relaxed));
    let target = f64::from_bits(shared.trainer_target_bits.load(Ordering::Relaxed));
    let next = (current + increment).min(target);
    shared
        .playback_rate_bits
        .store(next.to_bits(), Ordering::Release);
    if next >= target - 0.000_001 {
        shared.trainer_enabled.store(false, Ordering::Release);
    }
}

fn signal_advance_at_end(shared: &SharedState) {
    if EndBehavior::from_code(shared.end_behavior.load(Ordering::Relaxed)) == EndBehavior::Advance {
        shared.ended_generation.fetch_add(1, Ordering::AcqRel);
    }
}

fn metronome_sample(
    source_position: f64,
    source_rate: u32,
    playback_rate: f64,
    shared: &SharedState,
) -> f32 {
    if !shared.metronome_enabled.load(Ordering::Relaxed) {
        return 0.0;
    }
    let bpm = f64::from_bits(shared.grid_bpm_bits.load(Ordering::Relaxed));
    if !(30.0..=300.0).contains(&bpm) {
        return 0.0;
    }
    let offset = f64::from_bits(shared.beat_grid_offset_bits.load(Ordering::Relaxed));
    let seconds = source_position / source_rate as f64;
    let period = 60.0 / bpm;
    let grid_position = (seconds - offset) / period;
    let source_elapsed = (grid_position - grid_position.floor()) * period;
    let output_elapsed = source_elapsed / playback_rate.max(0.01);
    const CLICK_SECONDS: f64 = 0.035;
    if output_elapsed >= CLICK_SECONDS {
        return 0.0;
    }
    let beat = grid_position.floor() as i64;
    let accented = beat.rem_euclid(4) == 0;
    let frequency = if accented { 1_760.0 } else { 1_180.0 };
    let envelope = (1.0 - output_elapsed / CLICK_SECONDS).powi(3) as f32;
    let oscillator = (std::f64::consts::TAU * frequency * output_elapsed).sin() as f32;
    let volume = f32::from_bits(shared.metronome_volume_bits.load(Ordering::Relaxed));
    oscillator * envelope * volume * if accented { 0.42 } else { 0.28 }
}

fn sample_with_loop_crossfade(
    audio: &DecodedAudio,
    position: f64,
    output_channel: usize,
    loop_a: u64,
    loop_b: u64,
    valid_loop: bool,
) -> f32 {
    let primary = interpolated_sample(audio, position, output_channel);
    if !valid_loop {
        return primary;
    }
    let loop_length = loop_b - loop_a;
    let crossfade = ((audio.sample_rate as f64 * CROSSFADE_SECONDS) as u64)
        .min(loop_length / 2)
        .max(1);
    let remaining = loop_b as f64 - position;
    if remaining <= 0.0 || remaining >= crossfade as f64 {
        return primary;
    }
    let mix = 1.0 - remaining as f32 / crossfade as f32;
    let wrapped_position = loop_a as f64 + (crossfade as f64 - remaining);
    primary * (1.0 - mix) + interpolated_sample(audio, wrapped_position, output_channel) * mix
}

#[allow(clippy::too_many_arguments)]
fn playback_sample(
    shared: &SharedState,
    audio: &DecodedAudio,
    position: f64,
    output_channel: usize,
    loop_a: u64,
    loop_b: u64,
    valid_loop: bool,
    stem_gains: &[f32; STEM_COUNT],
) -> f32 {
    let Some(stems) = shared.stems.load_full() else {
        return sample_with_loop_crossfade(
            audio,
            position,
            output_channel,
            loop_a,
            loop_b,
            valid_loop,
        );
    };
    stems
        .stems
        .iter()
        .enumerate()
        .fold(0.0, |mix, (index, stem)| {
            let gain = stem_gains[index];
            if gain <= 0.0 {
                return mix;
            }
            mix + sample_with_loop_crossfade(
                stem,
                position,
                output_channel,
                loop_a,
                loop_b,
                valid_loop,
            ) * gain
        })
}

fn interpolated_sample(audio: &DecodedAudio, position: f64, output_channel: usize) -> f32 {
    let first_frame = position.floor() as usize;
    let next_frame = (first_frame + 1).min(audio.frames.saturating_sub(1));
    let fraction = (position - first_frame as f64) as f32;
    let channel = output_channel.min(audio.channels.saturating_sub(1));
    let first = audio.samples[first_frame * audio.channels + channel];
    let next = audio.samples[next_frame * audio.channels + channel];
    first + (next - first) * fraction
}

fn decoded_cache_path(source: &Path) -> Option<PathBuf> {
    let package = source.parent()?.parent()?;
    let file_name = source.file_name()?.to_string_lossy();
    Some(
        package
            .join("Cache")
            .join("decoded")
            .join(format!("{file_name}.pcm")),
    )
}

fn modified_stamp(modified: Option<SystemTime>) -> u64 {
    modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn load_decoded_cache(
    source: &Path,
    source_size: u64,
    modified: Option<SystemTime>,
) -> Option<Arc<DecodedAudio>> {
    let cache_path = decoded_cache_path(source)?;
    let mut file = File::open(cache_path).ok()?;
    let mut magic = [0_u8; 8];
    file.read_exact(&mut magic).ok()?;
    if &magic != PCM_CACHE_MAGIC
        || read_u64(&mut file)? != source_size
        || read_u64(&mut file)? != modified_stamp(modified)
    {
        return None;
    }
    let sample_rate = read_u32(&mut file)?;
    let channels = read_u32(&mut file)? as usize;
    let frames = read_u64(&mut file)? as usize;
    let sample_count = frames.checked_mul(channels)?;
    if sample_rate == 0
        || channels == 0
        || sample_count > MAX_DECODED_CACHE_BYTES / size_of::<f32>()
    {
        return None;
    }
    let expected_bytes = 40_u64.checked_add((sample_count * size_of::<f32>()) as u64)?;
    if file.metadata().ok()?.len() != expected_bytes {
        return None;
    }
    let mut samples = vec![0.0_f32; sample_count];
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(
            samples.as_mut_ptr().cast::<u8>(),
            sample_count * size_of::<f32>(),
        )
    };
    file.read_exact(bytes).ok()?;
    Some(Arc::new(DecodedAudio {
        samples,
        channels,
        sample_rate,
        frames,
    }))
}

fn store_decoded_cache(
    source: &Path,
    source_size: u64,
    modified: Option<SystemTime>,
    audio: &DecodedAudio,
) -> std::io::Result<()> {
    let Some(cache_path) = decoded_cache_path(source) else {
        return Ok(());
    };
    let Some(parent) = cache_path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)?;
    let temporary = cache_path.with_extension("pcm.tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(PCM_CACHE_MAGIC)?;
    file.write_all(&source_size.to_le_bytes())?;
    file.write_all(&modified_stamp(modified).to_le_bytes())?;
    file.write_all(&audio.sample_rate.to_le_bytes())?;
    file.write_all(&(audio.channels as u32).to_le_bytes())?;
    file.write_all(&(audio.frames as u64).to_le_bytes())?;
    let bytes = unsafe {
        std::slice::from_raw_parts(
            audio.samples.as_ptr().cast::<u8>(),
            audio.samples.len() * size_of::<f32>(),
        )
    };
    file.write_all(bytes)?;
    file.sync_data()?;
    fs::rename(temporary, cache_path)?;
    Ok(())
}

fn read_u32(reader: &mut impl Read) -> Option<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Option<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn decode(path: &Path) -> Result<DecodedAudio, AppError> {
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
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| invalid_audio(path, error))?;
    let mut samples = Vec::new();
    let mut channels = 0;
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
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(invalid_audio(path, error)),
        };
        channels = decoded.spec().channels.count();
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buffer.samples());
    }
    if channels == 0 || samples.is_empty() {
        return Err(AppError::InvalidAudio {
            path: path.to_path_buf(),
            reason: "the audio stream is empty".to_owned(),
        });
    }
    let frames = samples.len() / channels;
    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate,
        frames,
    })
}

pub(crate) fn decode_stem_file(path: &Path) -> Result<DecodedAudio, AppError> {
    decode(path)
}

fn invalid_audio(path: &Path, error: SymphoniaError) -> AppError {
    AppError::InvalidAudio {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_wraps_inside_the_realtime_renderer_without_silence() {
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples: vec![0.5; 100],
                channels: 1,
                sample_rate: 1_000,
                frames: 100,
            }))),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(18_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(10),
            loop_b: AtomicU64::new(20),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(true),
            trainer_repetitions: AtomicU32::new(2),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1.1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };
        let mut output = [0_f32; 64];

        render(&mut output, 1, 1_000, &shared);

        assert!(output
            .iter()
            .all(|sample| (*sample - 0.5).abs() < f32::EPSILON));
        let position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
        assert!((10.0..20.0).contains(&position));
        assert!(shared.playing.load(Ordering::Acquire));
        assert_eq!(
            f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire)),
            1.1
        );
        assert!(!shared.trainer_enabled.load(Ordering::Acquire));
    }

    #[test]
    fn crossfade_smooths_a_discontinuous_loop_boundary() {
        let audio = DecodedAudio {
            samples: (0..30)
                .map(|frame| if frame < 10 { -1.0 } else { 1.0 })
                .collect(),
            channels: 1,
            sample_rate: 1_000,
            frames: 30,
        };
        let values: Vec<f32> = (15..20)
            .map(|position| sample_with_loop_crossfade(&audio, position as f64, 0, 0, 20, true))
            .collect();

        assert_eq!(values[0], 1.0);
        assert!(values
            .windows(2)
            .all(|pair| (pair[1] - pair[0]).abs() <= 0.5));
        assert!(values.last().copied().unwrap() < 0.0);
    }

    #[test]
    fn trainer_replays_and_increments_a_complete_track_without_an_ab_loop() {
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples: vec![0.25; 12],
                channels: 1,
                sample_rate: 1_000,
                frames: 12,
            }))),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(10_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(0.8_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(true),
            trainer_repetitions: AtomicU32::new(2),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };
        let mut output = [0_f32; 30];

        render(&mut output, 1, 1_000, &shared);

        assert!(shared.playing.load(Ordering::Acquire));
        let rate = f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire));
        assert!((rate - 0.85).abs() < f64::EPSILON);
        assert!(output.iter().all(|sample| sample.is_finite()));

        shared.trainer_enabled.store(false, Ordering::Release);
        shared
            .end_behavior
            .store(EndBehavior::Restart.code(), Ordering::Release);
        shared.playing.store(true, Ordering::Release);
        shared
            .position_bits
            .store(11_f64.to_bits(), Ordering::Release);
        render(&mut output[..5], 1, 1_000, &shared);
        assert!(shared.playing.load(Ordering::Acquire));

        shared
            .end_behavior
            .store(EndBehavior::Advance.code(), Ordering::Release);
        shared.playing.store(true, Ordering::Release);
        shared
            .position_bits
            .store(11_f64.to_bits(), Ordering::Release);
        render(&mut output[..5], 1, 1_000, &shared);
        assert!(!shared.playing.load(Ordering::Acquire));
        assert_eq!(shared.ended_generation.load(Ordering::Acquire), 1);
    }

    #[test]
    fn play_preserves_positions_before_and_inside_loop_and_restarts_after_b() {
        let shared = Arc::new(SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples: vec![0.0; 100],
                channels: 1,
                sample_rate: 1_000,
                frames: 100,
            }))),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(false),
            position_bits: AtomicU64::new(50_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(10),
            loop_b: AtomicU64::new(20),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(false),
            trainer_repetitions: AtomicU32::new(3),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        });
        let engine = AudioEngine {
            shared,
            decode_cache: (Mutex::new(DecodeCache::default()), Condvar::new()),
            load_generation: AtomicU64::new(0),
            loaded_path: Mutex::new(None),
            output_sample_rate: 1_000,
            output_channels: 1,
            spectrum: spectrum::SpectrumWorker::new(),
        };

        engine.play();

        assert_eq!(
            f64::from_bits(engine.shared.position_bits.load(Ordering::Acquire)),
            10.0
        );
        assert!(engine.shared.playing.load(Ordering::Acquire));

        engine.pause();
        engine
            .shared
            .position_bits
            .store(5_f64.to_bits(), Ordering::Release);
        engine.play();
        assert_eq!(
            f64::from_bits(engine.shared.position_bits.load(Ordering::Acquire)),
            5.0
        );

        engine.pause();
        engine
            .shared
            .position_bits
            .store(15_f64.to_bits(), Ordering::Release);
        engine.play();
        assert_eq!(
            f64::from_bits(engine.shared.position_bits.load(Ordering::Acquire)),
            15.0
        );
    }

    #[test]
    fn loop_trainer_does_not_count_lead_in_before_a() {
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples: vec![0.25; 100],
                channels: 1,
                sample_rate: 1_000,
                frames: 100,
            }))),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(0_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(10),
            loop_b: AtomicU64::new(20),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(true),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(true),
            trainer_repetitions: AtomicU32::new(1),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1.05_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };
        let mut output = [0_f32; 11];

        render(&mut output, 1, 1_000, &shared);

        assert_eq!(
            f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire)),
            1.0
        );
        assert_eq!(shared.trainer_loop_count.load(Ordering::Acquire), 0);
        assert!(shared.trainer_enabled.load(Ordering::Acquire));

        render(&mut output, 1, 1_000, &shared);

        assert_eq!(
            f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire)),
            1.05
        );
        assert!(!shared.trainer_enabled.load(Ordering::Acquire));
    }

    #[test]
    fn stretched_renderer_advances_source_at_the_selected_rate() {
        let frames = 48_000;
        let samples = (0..frames)
            .map(|frame| ((frame as f32 * 440.0 * std::f32::consts::TAU) / 48_000.0).sin())
            .collect();
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples,
                channels: 1,
                sample_rate: 48_000,
                frames,
            }))),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(12_000_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(0.75_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(3_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(120_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            trainer_enabled: AtomicBool::new(false),
            trainer_repetitions: AtomicU32::new(3),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };
        let mut renderer = RealtimeRenderer::new(1, 48_000);
        let mut output = [0_f32; 512];

        renderer.render(&mut output, &shared);

        let position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
        assert!((12_384.0..12_512.0).contains(&position));
        assert!((0.75..1.0).contains(&renderer.smoothed_rate));
        assert!(output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn stem_mix_is_pitch_shifted_once_after_mixing() {
        let sample_rate = 48_000_u32;
        let frames = sample_rate as usize * 2;
        let silent = Arc::new(DecodedAudio {
            samples: vec![0.0; frames],
            channels: 1,
            sample_rate,
            frames,
        });
        let tone = Arc::new(DecodedAudio {
            samples: (0..frames)
                .map(|frame| {
                    (std::f32::consts::TAU * 440.0 * frame as f32 / sample_rate as f32).sin()
                })
                .collect(),
            channels: 1,
            sample_rate,
            frames,
        });
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::clone(&silent))),
            stems: ArcSwapOption::from(Some(Arc::new(StemSet {
                stems: [
                    tone,
                    Arc::clone(&silent),
                    Arc::clone(&silent),
                    Arc::clone(&silent),
                    Arc::clone(&silent),
                    silent,
                ],
            }))),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(0_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(12_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(0_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0_f32.to_bits()),
            trainer_enabled: AtomicBool::new(false),
            trainer_repetitions: AtomicU32::new(3),
            trainer_increment_bits: AtomicU64::new(0_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };
        let mut renderer = RealtimeRenderer::new(1, sample_rate);
        let mut output = vec![0_f32; sample_rate as usize];

        renderer.render(&mut output, &shared);

        let crossings = output[sample_rate as usize / 2..]
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count();
        assert!(
            (420..=460).contains(&crossings),
            "measured {crossings} cycles in half a second"
        );
    }

    #[test]
    fn metronome_is_silent_between_beats_and_clicks_on_the_grid() {
        let shared = SharedState {
            audio: ArcSwapOption::empty(),
            stems: ArcSwapOption::empty(),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(0_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(1_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            grid_bpm_bits: AtomicU64::new(120_f64.to_bits()),
            beat_grid_offset_bits: AtomicU64::new(0_f64.to_bits()),
            metronome_enabled: AtomicBool::new(true),
            metronome_volume_bits: AtomicU32::new(1_f32.to_bits()),
            trainer_enabled: AtomicBool::new(false),
            trainer_repetitions: AtomicU32::new(3),
            trainer_increment_bits: AtomicU64::new(0.05_f64.to_bits()),
            trainer_target_bits: AtomicU64::new(1_f64.to_bits()),
            trainer_loop_count: AtomicU32::new(0),
            end_behavior: AtomicU32::new(EndBehavior::Stop.code()),
            ended_generation: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            output_peak_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_left_bits: AtomicU32::new(0_f32.to_bits()),
            output_peak_right_bits: AtomicU32::new(0_f32.to_bits()),
        };

        let on_beat = metronome_sample(0.0, 48_000, 1.0, &shared);
        let just_after_beat = metronome_sample(240.0, 48_000, 1.0, &shared);
        let between_beats = metronome_sample(12_000.0, 48_000, 1.0, &shared);

        assert_eq!(on_beat, 0.0);
        assert_ne!(just_after_beat, 0.0);
        assert_eq!(between_beats, 0.0);
    }

    #[test]
    fn master_volume_controls_music_and_metronome_together() {
        assert_eq!(apply_master_gain(0.4, 0.2, 0.0), 0.0);
        assert!((apply_master_gain(0.4, 0.2, 0.5) - 0.3).abs() < f32::EPSILON);
        assert_eq!(apply_master_gain(1.0, 1.0, 1.0), 1.0);
    }

    #[test]
    fn gain_ramp_moves_progressively_toward_mute_and_restore() {
        let mut gain = 1.0;
        smooth_gain(&mut gain, 0.0, 0.25);
        assert!(gain > 0.0 && gain < 1.0);
        let muted_step = gain;
        smooth_gain(&mut gain, 1.0, 0.25);
        assert!(gain > muted_step && gain < 1.0);
    }

    #[test]
    fn seek_transition_preserves_continuity_then_reaches_the_new_signal() {
        let mut transition = SeekTransition::new(1, 1_000);
        transition.generation = 4;
        transition.last_output[0] = 0.75;

        let first_blend = transition.begin_frame(5);
        let first = transition.smooth_sample(0, -0.75, first_blend);
        transition.end_frame();
        assert!((first - 0.75).abs() < f32::EPSILON);

        for _ in 1..transition.total_frames {
            let blend = transition.begin_frame(5);
            transition.smooth_sample(0, -0.75, blend);
            transition.end_frame();
        }
        let settled_blend = transition.begin_frame(5);
        let settled = transition.smooth_sample(0, -0.75, settled_blend);
        assert_eq!(settled_blend, 1.0);
        assert!((settled + 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn signalsmith_applies_fractional_and_octave_pitch_transposition() {
        let sample_rate = 48_000_u32;
        let input: Vec<f32> = (0..sample_rate as usize * 2)
            .map(|frame| (std::f32::consts::TAU * 440.0 * frame as f32 / sample_rate as f32).sin())
            .collect();
        let mut output = vec![0.0_f32; input.len()];
        let mut stretch = Stretch::preset_default(1, sample_rate);
        stretch.set_transpose_factor_semitones(12.0, None);
        assert!(stretch.exact(&input, &mut output));
        let start = sample_rate as usize / 2;
        let crossings = output[start..start + sample_rate as usize]
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count();
        assert!((850..=910).contains(&crossings), "measured {crossings} Hz");

        stretch.set_transpose_factor_semitones(0.01, None);
    }

    #[test]
    fn decoded_pcm_cache_round_trips_without_loss() {
        let temp = tempfile::tempdir().unwrap();
        let audio_directory = temp.path().join("Project.sac/Audio");
        fs::create_dir_all(&audio_directory).unwrap();
        let source = audio_directory.join("track.wav");
        fs::write(&source, b"source fingerprint").unwrap();
        let metadata = source.metadata().unwrap();
        let audio = DecodedAudio {
            samples: vec![-0.5, 0.25, 0.75, -1.0],
            channels: 2,
            sample_rate: 48_000,
            frames: 2,
        };

        store_decoded_cache(&source, metadata.len(), metadata.modified().ok(), &audio).unwrap();
        let cached = load_decoded_cache(&source, metadata.len(), metadata.modified().ok()).unwrap();

        assert_eq!(cached.samples, audio.samples);
        assert_eq!(cached.channels, audio.channels);
        assert_eq!(cached.sample_rate, audio.sample_rate);
        assert_eq!(cached.frames, audio.frames);
    }
}
