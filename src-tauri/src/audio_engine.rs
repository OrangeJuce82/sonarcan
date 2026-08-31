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
const CROSSFADE_SECONDS: f64 = 0.010;
const GAIN_RAMP_SECONDS: f64 = 0.04;
const SEEK_TRANSITION_SECONDS: f64 = 0.008;
const MAX_DECODED_TRACKS: usize = 3;
const MAX_DECODED_CACHE_BYTES: usize = 384 * 1024 * 1024;
const MAX_DSP_OUTPUT_FRAMES: usize = 1_024;
const MAX_DSP_INPUT_FRAMES: usize = MAX_DSP_OUTPUT_FRAMES * 2 + 8;
const MAX_DSP_PREROLL_FRAMES: usize = 65_536;
const MAX_BEATS: usize = 262_144;
const MAX_DOWNBEATS: usize = 65_536;
const MAX_BEAT_POSITION_SECONDS: f64 = 24.0 * 60.0 * 60.0;
const PCM_CACHE_MAGIC: &[u8; 8] = b"SACPCM01";
type StemChannelGains = [[f32; 2]; STEM_COUNT];

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
    stems_enabled: AtomicBool,
    stem_gain_bits: [AtomicU32; STEM_COUNT],
    stem_pan_bits: [AtomicU32; STEM_COUNT],
    stem_muted: [AtomicBool; STEM_COUNT],
    stem_soloed: [AtomicBool; STEM_COUNT],
    stem_peak_bits: [AtomicU32; STEM_COUNT],
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
    beat_timeline: ArcSwapOption<BeatTimeline>,
    metronome_enabled: AtomicBool,
    metronome_volume_bits: AtomicU32,
    metronome_sound: AtomicU32,
    trainer_enabled: AtomicBool,
    trainer_start_bits: AtomicU64,
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

#[derive(Clone, Copy)]
struct BeatPoint {
    seconds: f64,
    downbeat: bool,
}

struct BeatTimeline {
    points: Vec<BeatPoint>,
}

impl BeatTimeline {
    fn from_detected(beats: &[f64], downbeats: &[f64]) -> Result<Self, AppError> {
        let positions_are_valid = |positions: &[f64], maximum: usize| {
            positions.len() <= maximum
                && positions.iter().all(|seconds| {
                    seconds.is_finite() && *seconds >= 0.0 && *seconds <= MAX_BEAT_POSITION_SECONDS
                })
                && positions.windows(2).all(|pair| pair[0] < pair[1])
        };
        if !positions_are_valid(beats, MAX_BEATS)
            || !positions_are_valid(downbeats, MAX_DOWNBEATS)
            || downbeats.iter().any(|downbeat| {
                beats
                    .binary_search_by(|beat| beat.total_cmp(downbeat))
                    .is_err()
            })
        {
            return Err(AppError::AudioEngine("invalid Beat This! timeline".into()));
        }

        let points = beats
            .iter()
            .copied()
            .map(|seconds| BeatPoint {
                seconds,
                downbeat: downbeats
                    .binary_search_by(|value| value.total_cmp(&seconds))
                    .is_ok(),
            })
            .collect();
        Ok(Self { points })
    }
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
    pub stems_enabled: bool,
    pub stem_peaks: [f32; STEM_COUNT],
    pub playback_rate: f64,
    pub pitch_semitones: f32,
    pub metronome_enabled: bool,
    pub metronome_volume: f32,
    pub trainer_enabled: bool,
    pub trainer_start_rate: f64,
    pub trainer_repetitions: u32,
    pub trainer_increment: f64,
    pub trainer_target_rate: f64,
    pub trainer_loop_count: u32,
    pub end_behavior: EndBehavior,
    pub ended_generation: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MetronomeSound {
    #[default]
    Electronic,
    Woodblock,
    Metallic,
}

impl MetronomeSound {
    const fn code(self) -> u32 {
        match self {
            Self::Electronic => 0,
            Self::Woodblock => 1,
            Self::Metallic => 2,
        }
    }

    const fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Woodblock,
            2 => Self::Metallic,
            _ => Self::Electronic,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopTrainerSettings {
    pub enabled: bool,
    pub start_rate: f64,
    pub repetitions: u32,
    pub increment: f64,
    pub target_rate: f64,
    pub loop_a_seconds: Option<f64>,
    pub loop_b_seconds: Option<f64>,
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
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(false),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
            trainer_repetitions: AtomicU32::new(1),
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
        self.shared.beat_timeline.store(None);
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
            Ok(cached)
        } else {
            decode(&path).map(|decoded| {
                let cache_audio = Arc::new(decoded);
                info!(path = %path.display(), elapsed_ms = load_started.elapsed().as_millis(), "decoded compressed audio");
                let write_audio = Arc::clone(&cache_audio);
                let write_path = path.clone();
                std::thread::Builder::new()
                    .name("sonarcan-pcm-cache".to_owned())
                    .spawn(move || {
                        let _ = store_decoded_cache(
                            &write_path,
                            source_size,
                            modified,
                            &write_audio,
                        );
                    })
                    .ok();
                cache_audio
            })
        };
        let mut cache =
            self.decode_cache.0.lock().map_err(|_| {
                AppError::AudioEngine("decoded-audio cache is unavailable".to_owned())
            })?;
        let result = finish_decode_load(&mut cache, &path, source_size, modified, decoded);
        self.decode_cache.1.notify_all();
        result
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

    pub fn seek(&self, seconds: f64) -> bool {
        let Some(audio) = self.shared.audio.load_full() else {
            return false;
        };
        let frame = (seconds.max(0.0) * audio.sample_rate as f64).min(audio.frames as f64);
        let loop_a = self.shared.loop_a.load(Ordering::Acquire);
        let loop_b = self.shared.loop_b.load(Ordering::Acquire);
        let loop_disabled = loop_a != NO_LOOP && loop_b > loop_a && frame >= loop_b as f64;
        if loop_disabled {
            self.clear_loop();
            self.shared.trainer_enabled.store(false, Ordering::Release);
            self.shared.trainer_loop_count.store(0, Ordering::Release);
        }
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
        self.shared
            .position_bits
            .store(frame.to_bits(), Ordering::Release);
        set_loop_cycle_state_for_position(&self.shared, frame);
        loop_disabled
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
        self.shared.stems_enabled.store(true, Ordering::Release);
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    pub fn disable_stems(&self) {
        self.shared.stems_enabled.store(false, Ordering::Release);
        self.shared.stems.store(None);
        for peak in &self.shared.stem_peak_bits {
            peak.store(0_f32.to_bits(), Ordering::Relaxed);
        }
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
    }

    pub fn set_stems_enabled(&self, enabled: bool) -> bool {
        let enabled = enabled && self.shared.stems.load().is_some();
        self.shared.stems_enabled.store(enabled, Ordering::Release);
        self.shared
            .position_generation
            .fetch_add(1, Ordering::AcqRel);
        enabled
    }

    pub fn set_stem_mix(&self, index: usize, gain: f32, pan: f32, muted: bool, soloed: bool) {
        if index >= STEM_COUNT {
            return;
        }
        self.shared.stem_gain_bits[index].store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Release);
        self.shared.stem_pan_bits[index].store(pan.clamp(-1.0, 1.0).to_bits(), Ordering::Release);
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

    pub fn set_beat_timeline(&self, beats: &[f64], downbeats: &[f64]) -> Result<(), AppError> {
        let timeline = BeatTimeline::from_detected(beats, downbeats)?;
        self.shared.beat_timeline.store(Some(Arc::new(timeline)));
        Ok(())
    }

    pub fn set_metronome(&self, enabled: bool, volume: f32, sound: MetronomeSound) {
        self.shared
            .metronome_enabled
            .store(enabled, Ordering::Release);
        self.shared
            .metronome_volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Release);
        self.shared
            .metronome_sound
            .store(sound.code(), Ordering::Release);
    }

    pub fn set_loop_trainer(&self, settings: LoopTrainerSettings) {
        let start_rate = settings.start_rate.clamp(0.5, 1.99);
        let target_rate = settings.target_rate.clamp(0.5, 2.0);
        self.shared
            .trainer_start_bits
            .store(start_rate.to_bits(), Ordering::Release);
        self.shared.trainer_enabled.store(
            settings.enabled && target_rate > start_rate,
            Ordering::Release,
        );
        self.shared
            .trainer_repetitions
            .store(settings.repetitions.clamp(1, 99), Ordering::Release);
        self.shared.trainer_increment_bits.store(
            settings.increment.clamp(0.01, 0.25).to_bits(),
            Ordering::Release,
        );
        self.shared
            .trainer_target_bits
            .store(target_rate.to_bits(), Ordering::Release);
        if settings.enabled && target_rate > start_rate {
            let loop_bounds = match (settings.loop_a_seconds, settings.loop_b_seconds) {
                (Some(a), Some(b)) if a >= 0.0 && b > a => (Some(a), Some(b)),
                _ => self.shared.audio.load_full().map_or((None, None), |audio| {
                    (
                        Some(0.0),
                        Some(audio.frames as f64 / audio.sample_rate as f64),
                    )
                }),
            };
            self.set_loop(loop_bounds.0, loop_bounds.1);
            self.shared
                .playback_rate_bits
                .store(start_rate.to_bits(), Ordering::Release);
        }
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
            stems_enabled: self.shared.stems_enabled.load(Ordering::Acquire),
            stem_peaks: std::array::from_fn(|index| {
                f32::from_bits(self.shared.stem_peak_bits[index].load(Ordering::Relaxed))
            }),
            playback_rate: f64::from_bits(self.shared.playback_rate_bits.load(Ordering::Acquire)),
            pitch_semitones: f32::from_bits(
                self.shared.pitch_semitones_bits.load(Ordering::Acquire),
            ),
            metronome_enabled: self.shared.metronome_enabled.load(Ordering::Acquire),
            metronome_volume: f32::from_bits(
                self.shared.metronome_volume_bits.load(Ordering::Acquire),
            ),
            trainer_enabled: self.shared.trainer_enabled.load(Ordering::Acquire),
            trainer_start_rate: f64::from_bits(
                self.shared.trainer_start_bits.load(Ordering::Acquire),
            ),
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

fn finish_decode_load(
    cache: &mut DecodeCache,
    path: &Path,
    source_size: u64,
    modified: Option<SystemTime>,
    decoded: Result<Arc<DecodedAudio>, AppError>,
) -> Result<Arc<DecodedAudio>, AppError> {
    cache.loading.remove(path);
    let audio = decoded?;
    cache.entries.insert(
        path.to_path_buf(),
        CachedAudio {
            audio: Arc::clone(&audio),
            file_size: source_size,
            modified,
        },
    );
    touch_cache_entry(&mut cache.recent, path);
    while (cache.recent.len() > MAX_DECODED_TRACKS
        || decoded_cache_bytes(cache) > MAX_DECODED_CACHE_BYTES)
        && cache.recent.len() > 1
    {
        if let Some(expired) = cache.recent.pop_back() {
            cache.entries.remove(&expired);
        }
    }
    Ok(audio)
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
    input_position: f64,
    output_position: f64,
    last_generation: u64,
    last_audio: usize,
    last_pitch: f32,
    last_reset_rate: f64,
    smoothed_rate: f64,
    smoothed_pitch: f32,
    smoothed_volume: f32,
    smoothed_stem_gains: StemChannelGains,
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
            input_position: 0.0,
            output_position: 0.0,
            last_generation: u64::MAX,
            last_audio: 0,
            last_pitch: f32::NAN,
            last_reset_rate: f64::NAN,
            smoothed_rate: 1.0,
            smoothed_pitch: 0.0,
            smoothed_volume: 0.8,
            smoothed_stem_gains: [[1.0; 2]; STEM_COUNT],
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
        let audio_pointer = shared
            .audio
            .load()
            .as_ref()
            .map_or(0, |audio| Arc::as_ptr(audio) as usize);
        let audio_changed = audio_pointer != 0 && audio_pointer != self.last_audio;
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
                smooth_gain(&mut current[0], target[0], blend as f32);
                smooth_gain(&mut current[1], target[1], blend as f32);
            }
        }
        let target_uses_dsp =
            (target_rate - 1.0).abs() >= 0.000_001 || target_pitch.abs() >= 0.000_1;
        if audio_changed || (!self.dsp_active && target_uses_dsp) {
            // Starting the DSP with the requested settings lets reset_at()
            // compensate its exact lookahead immediately. A new track must
            // also start from its own settings instead of ramping from the
            // previous track. Subsequent live changes retain the short ramp.
            self.smoothed_rate = target_rate;
            self.smoothed_pitch = target_pitch;
        } else {
            self.smoothed_rate += (target_rate - self.smoothed_rate) * blend;
            self.smoothed_pitch += (target_pitch - self.smoothed_pitch) * blend as f32;
        }
        if (target_rate - self.smoothed_rate).abs() < 0.000_05 {
            self.smoothed_rate = target_rate;
        }
        if (target_pitch - self.smoothed_pitch).abs() < 0.000_5 {
            self.smoothed_pitch = target_pitch;
        }
        if (self.smoothed_rate - 1.0).abs() < 0.000_001 && self.smoothed_pitch.abs() < 0.000_1 {
            self.dsp_active = false;
            self.last_audio = audio_pointer;
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
        stem_gains: &StemChannelGains,
    ) where
        T: SizedSample + FromSample<f32>,
    {
        let silence = T::from_sample(0.0);
        output.fill(silence);
        if !shared.playing.load(Ordering::Acquire) {
            self.seek_transition.clear_output();
            publish_output_peak(shared, 0.0);
            publish_stem_peaks(shared, [0.0; STEM_COUNT]);
            return;
        }
        let Some(audio) = shared.audio.load_full() else {
            self.seek_transition.clear_output();
            publish_output_peak(shared, 0.0);
            publish_stem_peaks(shared, [0.0; STEM_COUNT]);
            return;
        };
        let generation = shared.position_generation.load(Ordering::Acquire);
        let audio_pointer = Arc::as_ptr(&audio) as usize;
        let requested_position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
        let source_step = audio.sample_rate as f64 / self.output_rate as f64;
        let loop_a = shared.loop_a.load(Ordering::Acquire);
        let loop_b = shared.loop_b.load(Ordering::Acquire);
        let valid_loop = loop_a != NO_LOOP && loop_b != NO_LOOP && loop_b > loop_a;
        let full_track_training = !valid_loop && shared.trainer_enabled.load(Ordering::Acquire);
        let restart_at_end = EndBehavior::from_code(shared.end_behavior.load(Ordering::Acquire))
            == EndBehavior::Restart;
        let repeat_full_track = full_track_training || (!valid_loop && restart_at_end);
        let target_rate = f64::from_bits(shared.playback_rate_bits.load(Ordering::Acquire));
        let rate_settled = (target_rate - rate).abs() < 0.000_001;
        let needs_reset = !self.dsp_active
            || generation != self.last_generation
            || audio_pointer != self.last_audio
            || (rate_settled && (rate - self.last_reset_rate).abs() >= 0.000_1);
        let pitch_changed = (pitch - self.last_pitch).abs() >= 0.000_1;
        if needs_reset {
            self.reset_at(
                shared,
                &audio,
                requested_position,
                source_step,
                rate,
                loop_a,
                loop_b,
                valid_loop,
                repeat_full_track,
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
        let mut stem_peaks = [0.0_f32; STEM_COUNT];
        let beat_timeline = shared.beat_timeline.load_full();
        let mut reached_end = false;
        'chunks: for output_chunk in output.chunks_mut(MAX_DSP_OUTPUT_FRAMES * self.channels) {
            let output_frames = output_chunk.len() / self.channels;
            let exact_input = output_frames as f64 * rate + self.rate_remainder;
            let input_frames = (exact_input.floor() as usize).min(MAX_DSP_INPUT_FRAMES);
            self.rate_remainder = exact_input - input_frames as f64;
            for frame in 0..input_frames {
                if valid_loop {
                    self.input_position = metronome_loop_position(
                        self.input_position,
                        audio.sample_rate,
                        loop_a,
                        loop_b,
                        true,
                    );
                }
                if self.input_position >= audio.frames as f64 {
                    if repeat_full_track {
                        self.input_position %= audio.frames as f64;
                    } else {
                        for channel in 0..self.channels {
                            self.input[frame * self.channels + channel] = 0.0;
                        }
                        self.input_position += source_step;
                        continue;
                    }
                }
                let crossfade = loop_crossfade_at(
                    self.input_position,
                    audio.sample_rate,
                    if repeat_full_track { 0 } else { loop_a },
                    if repeat_full_track {
                        audio.frames as u64
                    } else {
                        loop_b
                    },
                    valid_loop || repeat_full_track,
                );
                for channel in 0..self.channels {
                    // Select/mix the six stems first, crossfade that final mix
                    // once at the loop boundary, then feed the result through
                    // Signalsmith once for combined tempo and pitch changes.
                    self.input[frame * self.channels + channel] = playback_sample(
                        shared,
                        &audio,
                        self.input_position,
                        channel,
                        crossfade,
                        stem_gains,
                        Some(&mut stem_peaks),
                    );
                }
                self.input_position += source_step;
            }
            let input_samples = input_frames * self.channels;
            let output_samples = output_frames * self.channels;
            self.stretch.process(
                &self.input[..input_samples],
                &mut self.processed[..output_samples],
            );
            for (frame_index, frame) in output_chunk.chunks_mut(self.channels).enumerate() {
                if valid_loop {
                    process_loop_boundary(
                        shared,
                        &mut self.output_position,
                        loop_a,
                        loop_b,
                        audio.sample_rate,
                    );
                }
                if self.output_position >= audio.frames as f64 {
                    if repeat_full_track {
                        if full_track_training {
                            handle_loop_trainer(shared);
                        }
                        self.output_position %= audio.frames as f64;
                    } else {
                        signal_advance_at_end(shared);
                        shared.playing.store(false, Ordering::Release);
                        reached_end = true;
                        break 'chunks;
                    }
                }
                let seek_blend = self.seek_transition.begin_frame(generation);
                let metronome_position = self.output_position;
                let metronome_crossfade = loop_crossfade_at(
                    metronome_position,
                    audio.sample_rate,
                    loop_a,
                    loop_b,
                    valid_loop,
                );
                let click = metronome_sample_with_crossfade(
                    metronome_position,
                    audio.sample_rate,
                    rate,
                    shared,
                    beat_timeline.as_deref(),
                    metronome_crossfade,
                );
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
                self.output_position += source_step * rate;
            }
        }
        if generation == shared.position_generation.load(Ordering::Acquire) {
            shared
                .position_bits
                .store(self.output_position.to_bits(), Ordering::Release);
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
        publish_stem_peaks(shared, stem_peaks);
        if reached_end {
            self.dsp_active = false;
        }
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
        repeat_full_track: bool,
        stem_gains: &StemChannelGains,
    ) {
        self.stretch.reset();
        self.rate_remainder = 0.0;
        let lookahead = self.stretch.input_latency()
            + (rate * self.stretch.output_latency() as f64).ceil() as usize;
        let frames = lookahead.clamp(1, MAX_DSP_PREROLL_FRAMES);
        let mut preroll_position = position;
        for frame in 0..frames {
            if valid_loop {
                preroll_position = metronome_loop_position(
                    preroll_position,
                    audio.sample_rate,
                    loop_a,
                    loop_b,
                    true,
                );
            } else if repeat_full_track && preroll_position >= audio.frames as f64 {
                preroll_position %= audio.frames as f64;
            }
            for channel in 0..self.channels {
                self.preroll[frame * self.channels + channel] =
                    if preroll_position < audio.frames as f64 {
                        let crossfade = loop_crossfade_at(
                            preroll_position,
                            audio.sample_rate,
                            if repeat_full_track { 0 } else { loop_a },
                            if repeat_full_track {
                                audio.frames as u64
                            } else {
                                loop_b
                            },
                            valid_loop || repeat_full_track,
                        );
                        playback_sample(
                            shared,
                            audio,
                            preroll_position,
                            channel,
                            crossfade,
                            stem_gains,
                            None,
                        )
                    } else {
                        0.0
                    };
            }
            preroll_position += source_step;
        }
        self.stretch
            .seek(&self.preroll[..frames * self.channels], rate);
        self.input_position = preroll_position;
        self.output_position = position;
        self.last_reset_rate = rate;
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

fn process_loop_boundary(
    shared: &SharedState,
    position: &mut f64,
    loop_a: u64,
    loop_b: u64,
    sample_rate: u32,
) {
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
    *position = loop_resume_position(*position, sample_rate, loop_a, loop_b);
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
    stem_gains: &StemChannelGains,
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
        publish_stem_peaks(shared, [0.0; STEM_COUNT]);
        return;
    }
    let Some(audio) = shared.audio.load_full() else {
        if let Some(transition) = seek_transition.as_deref_mut() {
            transition.clear_output();
        }
        publish_output_peak(shared, 0.0);
        publish_stem_peaks(shared, [0.0; STEM_COUNT]);
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
    let mut stem_peaks = [0.0_f32; STEM_COUNT];
    let beat_timeline = shared.beat_timeline.load_full();
    for frame in output.chunks_mut(output_channels) {
        let seek_blend = seek_transition
            .as_deref_mut()
            .map_or(1.0, |transition| transition.begin_frame(generation));
        if valid_loop {
            process_loop_boundary(shared, &mut position, loop_a, loop_b, audio.sample_rate);
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
        let crossfade = loop_crossfade_at(
            position,
            audio.sample_rate,
            if repeat_full_track { 0 } else { loop_a },
            if repeat_full_track {
                audio.frames as u64
            } else {
                loop_b
            },
            valid_loop || repeat_full_track,
        );
        let metronome_crossfade = if valid_loop { crossfade } else { None };
        let click = metronome_sample_with_crossfade(
            position,
            audio.sample_rate,
            1.0,
            shared,
            beat_timeline.as_deref(),
            metronome_crossfade,
        );
        for (channel, sample) in frame.iter_mut().enumerate() {
            let value = playback_sample(
                shared,
                &audio,
                position,
                channel,
                crossfade,
                stem_gains,
                Some(&mut stem_peaks),
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
    publish_stem_peaks(shared, stem_peaks);
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

fn publish_stem_peaks(shared: &SharedState, block_peaks: [f32; STEM_COUNT]) {
    for (peak, block_peak) in shared.stem_peak_bits.iter().zip(block_peaks) {
        let previous = f32::from_bits(peak.load(Ordering::Relaxed));
        peak.store(block_peak.max(previous * 0.82).to_bits(), Ordering::Relaxed);
    }
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

fn target_stem_gains(shared: &SharedState) -> StemChannelGains {
    let any_solo = shared
        .stem_soloed
        .iter()
        .any(|value| value.load(Ordering::Relaxed));
    std::array::from_fn(|index| {
        let muted = shared.stem_muted[index].load(Ordering::Relaxed);
        let soloed = shared.stem_soloed[index].load(Ordering::Relaxed);
        if muted || (any_solo && !soloed) {
            [0.0; 2]
        } else {
            let gain = f32::from_bits(shared.stem_gain_bits[index].load(Ordering::Relaxed));
            let pan = f32::from_bits(shared.stem_pan_bits[index].load(Ordering::Relaxed));
            [gain * (1.0 - pan.max(0.0)), gain * (1.0 + pan.min(0.0))]
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
    timeline: Option<&BeatTimeline>,
) -> f32 {
    if !shared.metronome_enabled.load(Ordering::Relaxed) {
        return 0.0;
    }
    let Some(timeline) = timeline else {
        return 0.0;
    };
    let seconds = source_position / source_rate as f64;
    let next_index = timeline
        .points
        .partition_point(|point| point.seconds <= seconds);
    let Some(point) = next_index
        .checked_sub(1)
        .and_then(|index| timeline.points.get(index))
    else {
        return 0.0;
    };
    let source_elapsed = seconds - point.seconds;
    let output_elapsed = source_elapsed / playback_rate.max(0.01);
    let sound = MetronomeSound::from_code(shared.metronome_sound.load(Ordering::Relaxed));
    let oscillator = synthesize_metronome_sound(sound, output_elapsed, point.downbeat);
    let volume = f32::from_bits(shared.metronome_volume_bits.load(Ordering::Relaxed));
    oscillator * volume
}

fn metronome_sample_with_crossfade(
    source_position: f64,
    source_rate: u32,
    playback_rate: f64,
    shared: &SharedState,
    timeline: Option<&BeatTimeline>,
    crossfade: Option<LoopCrossfade>,
) -> f32 {
    let primary = metronome_sample(
        source_position,
        source_rate,
        playback_rate,
        shared,
        timeline,
    );
    let Some(crossfade) = crossfade else {
        return primary;
    };
    let wrapped = metronome_sample(
        crossfade.wrapped_position,
        source_rate,
        playback_rate,
        shared,
        timeline,
    );
    primary * (1.0 - crossfade.mix) + wrapped * crossfade.mix
}

fn synthesize_metronome_sound(sound: MetronomeSound, elapsed: f64, downbeat: bool) -> f32 {
    let tau = std::f64::consts::TAU;
    match sound {
        MetronomeSound::Electronic => {
            const DURATION: f64 = 0.035;
            if elapsed >= DURATION {
                return 0.0;
            }
            let frequency = if downbeat { 1_760.0 } else { 1_180.0 };
            let envelope = (1.0 - elapsed / DURATION).powi(3) as f32;
            let oscillator = (tau * frequency * elapsed).sin() as f32;
            oscillator * envelope * if downbeat { 0.42 } else { 0.28 }
        }
        MetronomeSound::Woodblock => {
            const DURATION: f64 = 0.055;
            if elapsed >= DURATION {
                return 0.0;
            }
            let fundamental = if downbeat { 1_050.0 } else { 760.0 };
            let envelope = (1.0 - elapsed / DURATION).powi(4) as f32;
            let modes = (tau * fundamental * elapsed).sin()
                + 0.58 * (tau * fundamental * 1.47 * elapsed).sin()
                + 0.24 * (tau * fundamental * 2.09 * elapsed).sin();
            modes as f32 * envelope * if downbeat { 0.34 } else { 0.27 }
        }
        MetronomeSound::Metallic => {
            const DURATION: f64 = 0.075;
            if elapsed >= DURATION {
                return 0.0;
            }
            let base = if downbeat { 690.0 } else { 510.0 };
            let envelope = (1.0 - elapsed / DURATION).powi(3) as f32;
            let partials = (tau * base * elapsed).sin()
                + 0.72 * (tau * base * 1.53 * elapsed).sin()
                + 0.38 * (tau * base * 2.37 * elapsed).sin();
            partials as f32 * envelope * if downbeat { 0.30 } else { 0.23 }
        }
    }
}

#[derive(Clone, Copy)]
struct LoopCrossfade {
    wrapped_position: f64,
    mix: f32,
}

fn loop_crossfade_at(
    position: f64,
    sample_rate: u32,
    loop_a: u64,
    loop_b: u64,
    valid_loop: bool,
) -> Option<LoopCrossfade> {
    if !valid_loop {
        return None;
    }
    let crossfade = loop_crossfade_frames(sample_rate, loop_a, loop_b);
    let remaining = loop_b as f64 - position;
    if remaining <= 0.0 || remaining >= crossfade as f64 {
        return None;
    }
    Some(LoopCrossfade {
        wrapped_position: loop_a as f64 + (crossfade as f64 - remaining),
        mix: 1.0 - remaining as f32 / crossfade as f32,
    })
}

fn loop_crossfade_frames(sample_rate: u32, loop_a: u64, loop_b: u64) -> u64 {
    let loop_length = loop_b.saturating_sub(loop_a);
    ((sample_rate as f64 * CROSSFADE_SECONDS) as u64)
        .min(loop_length / 2)
        .max(1)
}

fn loop_resume_position(position: f64, sample_rate: u32, loop_a: u64, loop_b: u64) -> f64 {
    let overshoot = position - loop_b as f64;
    let resumed =
        loop_a as f64 + loop_crossfade_frames(sample_rate, loop_a, loop_b) as f64 + overshoot;
    wrap_loop_position(resumed, loop_a, loop_b, true)
}

fn metronome_loop_position(
    position: f64,
    sample_rate: u32,
    loop_a: u64,
    loop_b: u64,
    valid_loop: bool,
) -> f64 {
    if valid_loop && position >= loop_b as f64 {
        loop_resume_position(position, sample_rate, loop_a, loop_b)
    } else {
        position
    }
}

#[allow(clippy::too_many_arguments)]
fn playback_sample(
    shared: &SharedState,
    audio: &DecodedAudio,
    position: f64,
    output_channel: usize,
    crossfade: Option<LoopCrossfade>,
    stem_gains: &StemChannelGains,
    stem_peaks: Option<&mut [f32; STEM_COUNT]>,
) -> f32 {
    let stems = shared
        .stems_enabled
        .load(Ordering::Relaxed)
        .then(|| shared.stems.load_full())
        .flatten();
    mixed_sample_with_loop_crossfade(
        audio,
        stems.as_deref(),
        position,
        output_channel,
        crossfade,
        stem_gains,
        stem_peaks,
    )
}

fn mixed_sample_with_loop_crossfade(
    audio: &DecodedAudio,
    stems: Option<&StemSet>,
    position: f64,
    output_channel: usize,
    crossfade: Option<LoopCrossfade>,
    stem_gains: &StemChannelGains,
    mut stem_peaks: Option<&mut [f32; STEM_COUNT]>,
) -> f32 {
    let primary = mixed_sample_at(
        audio,
        stems,
        position,
        output_channel,
        stem_gains,
        stem_peaks.as_deref_mut(),
    );
    let Some(crossfade) = crossfade else {
        return primary;
    };
    let wrapped = mixed_sample_at(
        audio,
        stems,
        crossfade.wrapped_position,
        output_channel,
        stem_gains,
        stem_peaks,
    );
    primary * (1.0 - crossfade.mix) + wrapped * crossfade.mix
}

fn mixed_sample_at(
    audio: &DecodedAudio,
    stems: Option<&StemSet>,
    position: f64,
    output_channel: usize,
    stem_gains: &StemChannelGains,
    mut stem_peaks: Option<&mut [f32; STEM_COUNT]>,
) -> f32 {
    let Some(stems) = stems else {
        return interpolated_sample(audio, position, output_channel);
    };
    let channel = output_channel.min(1);
    let mut mix = 0.0;
    for (index, stem) in stems.stems.iter().enumerate() {
        let gain = stem_gains[index][channel];
        if gain <= 0.0 {
            continue;
        }
        let sample = interpolated_sample(stem, position, output_channel) * gain;
        if let Some(peaks) = stem_peaks.as_deref_mut() {
            peaks[index] = peaks[index].max(sample.abs());
        }
        mix += sample;
    }
    mix
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

    fn original_loop_sample(audio: &DecodedAudio, position: f64, loop_a: u64, loop_b: u64) -> f32 {
        mixed_sample_with_loop_crossfade(
            audio,
            None,
            position,
            0,
            loop_crossfade_at(position, audio.sample_rate, loop_a, loop_b, true),
            &[[1.0; 2]; STEM_COUNT],
            None,
        )
    }

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
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(true),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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
        let values: Vec<f32> = (10..20)
            .map(|position| original_loop_sample(&audio, position as f64, 0, 20))
            .collect();

        assert_eq!(values[0], 1.0);
        assert!(values
            .windows(2)
            .all(|pair| (pair[1] - pair[0]).abs() <= 0.5));
        assert!(values.last().copied().unwrap() < 0.0);
    }

    #[test]
    fn crossfade_is_applied_to_the_final_stem_mix() {
        let sample_rate = 1_000;
        let frames = 30;
        let original = DecodedAudio {
            samples: vec![0.0; frames],
            channels: 1,
            sample_rate,
            frames,
        };
        let discontinuous = Arc::new(DecodedAudio {
            samples: (0..frames)
                .map(|frame| if frame < 10 { -1.0 } else { 1.0 })
                .collect(),
            channels: 1,
            sample_rate,
            frames,
        });
        let constant = Arc::new(DecodedAudio {
            samples: vec![0.25; frames],
            channels: 1,
            sample_rate,
            frames,
        });
        let silent = Arc::new(DecodedAudio {
            samples: vec![0.0; frames],
            channels: 1,
            sample_rate,
            frames,
        });
        let stems = StemSet {
            stems: [
                discontinuous,
                constant,
                Arc::clone(&silent),
                Arc::clone(&silent),
                Arc::clone(&silent),
                silent,
            ],
        };
        let gains = [[1.0; 2]; STEM_COUNT];
        let position = 15.0;
        let crossfade = loop_crossfade_at(position, sample_rate, 0, 20, true);

        let primary = mixed_sample_at(&original, Some(&stems), position, 0, &gains, None);
        let wrapped = mixed_sample_at(
            &original,
            Some(&stems),
            crossfade.unwrap().wrapped_position,
            0,
            &gains,
            None,
        );
        let result = mixed_sample_with_loop_crossfade(
            &original,
            Some(&stems),
            position,
            0,
            crossfade,
            &gains,
            None,
        );

        assert_eq!(primary, 1.25);
        assert_eq!(wrapped, -0.75);
        assert!((result - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn loop_resumes_after_the_head_already_used_by_the_crossfade() {
        let sample_rate = 48_000;
        let loop_a = 0_u64;
        let loop_b = 1_440_u64;
        let audio = DecodedAudio {
            samples: (0..loop_b)
                .map(|frame| if frame < 720 { -1.0 } else { 1.0 })
                .collect(),
            channels: 1,
            sample_rate,
            frames: loop_b as usize,
        };
        let before = original_loop_sample(&audio, loop_b as f64 - 1.0, loop_a, loop_b);
        let resumed = loop_resume_position(loop_b as f64, sample_rate, loop_a, loop_b);
        let after = interpolated_sample(&audio, resumed, 0);

        assert_eq!(resumed, 480.0);
        assert!((after - before).abs() < 0.01);
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
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(true),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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
    fn loop_transport_respects_bounds_and_training_activation_configures_a_full_loop() {
        let shared = Arc::new(SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples: vec![0.0; 100],
                channels: 1,
                sample_rate: 1_000,
                frames: 100,
            }))),
            stems: ArcSwapOption::empty(),
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(false),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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

        assert!(!engine.seek(0.015));
        assert_eq!(engine.shared.loop_a.load(Ordering::Acquire), 10);
        assert_eq!(engine.shared.loop_b.load(Ordering::Acquire), 20);

        engine.shared.trainer_enabled.store(true, Ordering::Release);
        assert!(engine.seek(0.02));
        assert_eq!(engine.shared.loop_a.load(Ordering::Acquire), NO_LOOP);
        assert_eq!(engine.shared.loop_b.load(Ordering::Acquire), NO_LOOP);
        assert!(!engine.shared.trainer_enabled.load(Ordering::Acquire));
        assert_eq!(
            f64::from_bits(engine.shared.position_bits.load(Ordering::Acquire)),
            20.0
        );

        engine.set_loop_trainer(LoopTrainerSettings {
            enabled: true,
            start_rate: 0.5,
            repetitions: 1,
            increment: 0.05,
            target_rate: 1.0,
            loop_a_seconds: None,
            loop_b_seconds: None,
        });
        assert!(engine.shared.trainer_enabled.load(Ordering::Acquire));
        assert_eq!(engine.shared.loop_a.load(Ordering::Acquire), 0);
        assert_eq!(engine.shared.loop_b.load(Ordering::Acquire), 100);
        assert_eq!(
            f64::from_bits(engine.shared.playback_rate_bits.load(Ordering::Acquire)),
            0.5
        );
        handle_loop_trainer(&engine.shared);
        assert_eq!(
            f64::from_bits(engine.shared.playback_rate_bits.load(Ordering::Acquire)),
            0.55
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
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(true),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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
    fn stretched_renderer_keeps_audible_audio_and_public_clock_aligned() {
        let frames = 48_000;
        let mut samples = vec![0.0; frames];
        samples[24_000] = 1.0;
        let shared = SharedState {
            audio: ArcSwapOption::from(Some(Arc::new(DecodedAudio {
                samples,
                channels: 1,
                sample_rate: 48_000,
                frames,
            }))),
            stems: ArcSwapOption::empty(),
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            playing: AtomicBool::new(true),
            position_bits: AtomicU64::new(0_f64.to_bits()),
            position_generation: AtomicU64::new(0),
            loop_a: AtomicU64::new(NO_LOOP),
            loop_b: AtomicU64::new(NO_LOOP),
            loop_cycle_armed: AtomicBool::new(false),
            loop_waiting_for_a: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1_f32.to_bits()),
            playback_rate_bits: AtomicU64::new(0.5_f64.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0_f32.to_bits()),
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0.55_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(false),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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
        let mut output = vec![0_f32; 50_000];

        renderer.render(&mut output, &shared);

        let position = f64::from_bits(shared.position_bits.load(Ordering::Acquire));
        let peak = output
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
            .map(|(index, _)| index)
            .unwrap();
        assert!(
            (47_984..=48_016).contains(&peak),
            "impulse rendered at {peak}"
        );
        assert!((position - 25_000.0).abs() < 0.001);
        assert!((renderer.input_position - position - 4_320.0).abs() < 0.001);
        assert_eq!(renderer.smoothed_rate, 0.5);
        assert!(output.iter().all(|sample| sample.is_finite()));

        shared.audio.store(Some(Arc::new(DecodedAudio {
            samples: vec![0.0; frames],
            channels: 1,
            sample_rate: 48_000,
            frames,
        })));
        shared
            .position_bits
            .store(0_f64.to_bits(), Ordering::Release);
        shared.position_generation.fetch_add(1, Ordering::AcqRel);
        shared
            .playback_rate_bits
            .store(1.25_f64.to_bits(), Ordering::Release);
        shared
            .pitch_semitones_bits
            .store((-2_f32).to_bits(), Ordering::Release);
        let mut next_track_output = [0_f32; 512];

        renderer.render(&mut next_track_output, &shared);

        assert_eq!(renderer.smoothed_rate, 1.25);
        assert_eq!(renderer.smoothed_pitch, -2.0);
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
            stems_enabled: AtomicBool::new(true),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(false),
            metronome_volume_bits: AtomicU32::new(0_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(false),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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
        shared.stem_pan_bits[0].store(1_f32.to_bits(), Ordering::Relaxed);
        let panned = target_stem_gains(&shared);
        assert_eq!(panned[0], [0.0, 1.0]);
        shared.stem_pan_bits[0].store(0_f32.to_bits(), Ordering::Relaxed);
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
        assert!(f32::from_bits(shared.stem_peak_bits[0].load(Ordering::Relaxed)) > 0.5);

        shared.stems_enabled.store(false, Ordering::Release);
        shared
            .position_bits
            .store(0_f64.to_bits(), Ordering::Release);
        shared.position_generation.fetch_add(1, Ordering::AcqRel);
        shared.playing.store(true, Ordering::Release);
        let mut bypassed = vec![0_f32; 4_800];
        RealtimeRenderer::new(1, sample_rate).render(&mut bypassed, &shared);
        assert!(bypassed.iter().all(|sample| sample.abs() < 0.000_1));
    }

    #[test]
    fn metronome_is_silent_between_beats_and_clicks_on_the_grid() {
        let shared = SharedState {
            audio: ArcSwapOption::empty(),
            stems: ArcSwapOption::empty(),
            stems_enabled: AtomicBool::new(false),
            stem_gain_bits: std::array::from_fn(|_| AtomicU32::new(1_f32.to_bits())),
            stem_pan_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
            stem_muted: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            stem_peak_bits: std::array::from_fn(|_| AtomicU32::new(0_f32.to_bits())),
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
            beat_timeline: ArcSwapOption::empty(),
            metronome_enabled: AtomicBool::new(true),
            metronome_volume_bits: AtomicU32::new(1_f32.to_bits()),
            metronome_sound: AtomicU32::new(MetronomeSound::Electronic.code()),
            trainer_enabled: AtomicBool::new(false),
            trainer_start_bits: AtomicU64::new(0.5_f64.to_bits()),
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

        let timeline = BeatTimeline {
            points: vec![
                BeatPoint {
                    seconds: 0.0,
                    downbeat: true,
                },
                BeatPoint {
                    seconds: 0.51,
                    downbeat: false,
                },
                BeatPoint {
                    seconds: 1.03,
                    downbeat: false,
                },
            ],
        };
        let on_beat = metronome_sample(0.0, 48_000, 1.0, &shared, Some(&timeline));
        let just_after_beat = metronome_sample(240.0, 48_000, 1.0, &shared, Some(&timeline));
        let between_beats = metronome_sample(12_000.0, 48_000, 1.0, &shared, Some(&timeline));

        assert_eq!(on_beat, 0.0);
        assert_ne!(just_after_beat, 0.0);
        assert_eq!(between_beats, 0.0);

        assert_eq!(metronome_sample(24_000.0, 48_000, 1.0, &shared, None), 0.0);
        assert_ne!(
            metronome_sample(49_440.0 + 240.0, 48_000, 1.0, &shared, Some(&timeline)),
            0.0
        );
        assert_eq!(
            metronome_sample(
                4_800.0,
                48_000,
                1.0,
                &shared,
                Some(&BeatTimeline {
                    points: vec![BeatPoint {
                        seconds: 0.5,
                        downbeat: true,
                    }],
                }),
            ),
            0.0,
            "the metronome must remain silent before the first detected beat"
        );

        let normal_speed = metronome_sample(240.0, 48_000, 1.0, &shared, Some(&timeline));
        let half_speed = metronome_sample(120.0, 48_000, 0.5, &shared, Some(&timeline));
        assert!((normal_speed - half_speed).abs() < f32::EPSILON);

        let loop_timeline = BeatTimeline {
            points: vec![BeatPoint {
                seconds: 0.5,
                downbeat: true,
            }],
        };
        let loop_a = 24_000;
        let loop_b = 48_000;
        let position_before_b = loop_b as f64 - 288.0;
        let crossfade = loop_crossfade_at(position_before_b, 48_000, loop_a, loop_b, true);
        assert_eq!(
            metronome_sample(
                position_before_b,
                48_000,
                1.0,
                &shared,
                Some(&loop_timeline),
            ),
            0.0,
            "the outgoing timeline has no beat near B"
        );
        assert_ne!(
            metronome_sample_with_crossfade(
                position_before_b,
                48_000,
                1.0,
                &shared,
                Some(&loop_timeline),
                crossfade,
            ),
            0.0,
            "the Beat This! click at A must follow the loop-head crossfade"
        );
        assert_eq!(
            metronome_loop_position(loop_b as f64, 48_000, loop_a, loop_b, true),
            loop_a as f64 + loop_crossfade_frames(48_000, loop_a, loop_b) as f64,
            "direct and stretched playback must resume the metronome at the same source phase"
        );
    }

    #[test]
    fn beat_timeline_rejects_untrusted_or_inconsistent_positions() {
        assert!(BeatTimeline::from_detected(&[0.5, 1.0], &[0.5]).is_ok());
        assert!(BeatTimeline::from_detected(&[1.0, 0.5], &[]).is_err());
        assert!(BeatTimeline::from_detected(&[0.5, f64::NAN], &[]).is_err());
        assert!(BeatTimeline::from_detected(&[0.5], &[0.6]).is_err());
        assert!(BeatTimeline::from_detected(&[MAX_BEAT_POSITION_SECONDS + 1.0], &[]).is_err());
    }

    #[test]
    fn metronome_timbres_are_distinct_bounded_bursts() {
        let elapsed = 0.005;
        let electronic = synthesize_metronome_sound(MetronomeSound::Electronic, elapsed, false);
        let woodblock = synthesize_metronome_sound(MetronomeSound::Woodblock, elapsed, false);
        let metallic = synthesize_metronome_sound(MetronomeSound::Metallic, elapsed, false);

        assert!((electronic - woodblock).abs() > 0.01);
        assert!((woodblock - metallic).abs() > 0.01);
        for sample in [electronic, woodblock, metallic] {
            assert!(sample.is_finite());
            assert!(sample.abs() <= 1.0);
        }
        for sound in [
            MetronomeSound::Electronic,
            MetronomeSound::Woodblock,
            MetronomeSound::Metallic,
        ] {
            assert_eq!(synthesize_metronome_sound(sound, 0.1, false), 0.0);
        }
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
    fn signalsmith_preroll_compensates_latency_at_practice_rates() {
        let sample_rate = 48_000_u32;
        for rate in [1.0_f64, 0.75, 0.5] {
            let mut stretch = Stretch::preset_default(1, sample_rate);
            let input_latency = stretch.input_latency();
            let output_latency = stretch.output_latency();
            let lookahead = input_latency + (rate * output_latency as f64).ceil() as usize;
            let impulse_source = 24_000_usize;
            let preroll: Vec<f32> = (0..lookahead)
                .map(|frame| if frame == impulse_source { 1.0 } else { 0.0 })
                .collect();
            stretch.seek(preroll, rate);
            let mut source_position = lookahead;
            let mut rendered = Vec::new();
            while source_position < 48_000 {
                let output_frames = 1_024_usize;
                let input_frames = (output_frames as f64 * rate) as usize;
                let mut input = vec![0.0_f32; input_frames];
                if impulse_source >= source_position
                    && impulse_source < source_position + input_frames
                {
                    input[impulse_source - source_position] = 1.0;
                }
                let mut output = vec![0.0_f32; output_frames];
                stretch.process(&input, &mut output);
                rendered.extend(output);
                source_position += input_frames;
            }
            let peak = rendered
                .iter()
                .enumerate()
                .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
                .map(|(index, _)| index)
                .unwrap();
            let expected = impulse_source as f64 / rate;
            assert!(
                (peak as f64 - expected).abs() <= 16.0,
                "rate {rate}: expected impulse near {expected}, rendered at {peak}"
            );
        }
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

    #[test]
    fn failed_decode_releases_the_in_flight_cache_entry() {
        let path = PathBuf::from("unreadable.mp3");
        let mut cache = DecodeCache::default();
        cache.loading.insert(path.clone());

        let result = finish_decode_load(
            &mut cache,
            &path,
            0,
            None,
            Err(AppError::AudioEngine("decode failed".to_owned())),
        );

        assert!(result.is_err());
        assert!(!cache.loading.contains(&path));
        assert!(cache.entries.is_empty());
    }
}
