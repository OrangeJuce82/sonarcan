use std::{fs, path::PathBuf, sync::Mutex};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct UserPreferences {
    pub theme: Theme,
    pub language: String,
    pub time_display: TimeDisplay,
    pub toast_duration_seconds: u32,
    pub concurrent_downloads: usize,
    pub conversion_format: ConversionFormat,
    pub sample_rate: SampleRatePreference,
    pub channels: ChannelPreference,
    pub mp3_quality: Mp3Quality,
    pub master_volume: f32,
    pub metronome_volume: f32,
    pub default_playback_rate: f64,
    pub default_pitch_semitones: f64,
    pub loop_load_position: LoopLoadPosition,
    pub default_trainer_start_rate: f64,
    pub default_trainer_repetitions: u32,
    pub default_trainer_increment: f64,
    pub default_trainer_target_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Dark,
    Light,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimeDisplay {
    Simple,
    Precise,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversionFormat {
    Keep,
    Mp3,
    Wav,
    Flac,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SampleRatePreference {
    Preserve,
    Hz44100,
    Hz48000,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelPreference {
    Preserve,
    Stereo,
    Mono,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mp3Quality {
    VbrHigh,
    Kbps320,
    Kbps256,
    Kbps192,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoopLoadPosition {
    Beginning,
    LoopStart,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: "en".into(),
            time_display: TimeDisplay::Simple,
            toast_duration_seconds: 3,
            concurrent_downloads: 3,
            conversion_format: ConversionFormat::Mp3,
            sample_rate: SampleRatePreference::Preserve,
            channels: ChannelPreference::Stereo,
            mp3_quality: Mp3Quality::VbrHigh,
            master_volume: 0.8,
            metronome_volume: 0.55,
            default_playback_rate: 1.0,
            default_pitch_semitones: 0.0,
            loop_load_position: LoopLoadPosition::Beginning,
            default_trainer_start_rate: 0.5,
            default_trainer_repetitions: 1,
            default_trainer_increment: 0.05,
            default_trainer_target_rate: 1.0,
        }
    }
}

pub struct PreferencesStore(Mutex<UserPreferences>);

impl PreferencesStore {
    pub fn load() -> Self {
        let preferences = preference_path()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self(Mutex::new(preferences))
    }

    pub fn get(&self) -> UserPreferences {
        self.0.lock().map(|value| value.clone()).unwrap_or_default()
    }

    pub fn save(&self, mut value: UserPreferences) -> Result<UserPreferences, AppError> {
        validate(&mut value);
        let path = preference_path().ok_or_else(|| {
            AppError::BackgroundTask("preferences directory is unavailable".into())
        })?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&value)?)
            .map_err(|error| AppError::io(&temporary, error))?;
        fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
        *self
            .0
            .lock()
            .map_err(|_| AppError::BackgroundTask("preferences state is unavailable".into()))? =
            value.clone();
        Ok(value)
    }
}

fn preference_path() -> Option<PathBuf> {
    ProjectDirs::from("music", "SonArcan", "SonArcan")
        .map(|dirs| dirs.config_dir().join("preferences.json"))
}
fn validate(value: &mut UserPreferences) {
    value.toast_duration_seconds = value.toast_duration_seconds.clamp(1, 10);
    value.concurrent_downloads = value.concurrent_downloads.clamp(1, 8);
    value.master_volume = value.master_volume.clamp(0.0, 1.0);
    value.metronome_volume = value.metronome_volume.clamp(0.0, 1.0);
    value.default_playback_rate = value.default_playback_rate.clamp(0.5, 2.0);
    value.default_pitch_semitones = value.default_pitch_semitones.clamp(-12.0, 12.0);
    value.default_trainer_start_rate = value.default_trainer_start_rate.clamp(0.5, 1.99);
    value.default_trainer_target_rate = value
        .default_trainer_target_rate
        .clamp(value.default_trainer_start_rate + 0.01, 2.0);
    value.default_trainer_repetitions = value.default_trainer_repetitions.clamp(1, 99);
    value.default_trainer_increment = value.default_trainer_increment.clamp(0.01, 0.25);
    if value.language != "fr" {
        value.language = "en".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn training_defaults_match_the_product_contract() {
        let preferences = UserPreferences::default();
        assert_eq!(preferences.toast_duration_seconds, 3);
        assert_eq!(preferences.time_display, TimeDisplay::Simple);
        assert_eq!(preferences.default_trainer_start_rate, 0.5);
        assert_eq!(preferences.default_trainer_target_rate, 1.0);
        assert_eq!(preferences.default_trainer_increment, 0.05);
        assert_eq!(preferences.default_trainer_repetitions, 1);
        assert_eq!(preferences.loop_load_position, LoopLoadPosition::Beginning);
    }

    #[test]
    fn older_preferences_default_to_simplified_time() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("timeDisplay");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert_eq!(preferences.time_display, TimeDisplay::Simple);
    }

    #[test]
    fn toast_duration_is_kept_within_the_supported_range() {
        let mut too_short = UserPreferences {
            toast_duration_seconds: 0,
            ..UserPreferences::default()
        };
        validate(&mut too_short);
        assert_eq!(too_short.toast_duration_seconds, 1);

        let mut too_long = UserPreferences {
            toast_duration_seconds: 30,
            ..UserPreferences::default()
        };
        validate(&mut too_long);
        assert_eq!(too_long.toast_duration_seconds, 10);
    }

    #[test]
    fn training_preferences_always_keep_the_end_above_the_start() {
        let mut preferences = UserPreferences {
            default_trainer_start_rate: 1.5,
            default_trainer_target_rate: 1.0,
            ..UserPreferences::default()
        };
        validate(&mut preferences);
        assert_eq!(preferences.default_trainer_start_rate, 1.5);
        assert_eq!(preferences.default_trainer_target_rate, 1.51);
    }
}
