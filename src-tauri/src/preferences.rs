use std::{fs, path::PathBuf, sync::Mutex};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{audio_engine::MetronomeSound, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct UserPreferences {
    pub theme: Theme,
    pub language: String,
    pub time_display: TimeDisplay,
    pub toast_duration_seconds: u32,
    pub concurrent_downloads: usize,
    pub youtube_auto_select_best_match: bool,
    pub conversion_format: ConversionFormat,
    pub sample_rate: SampleRatePreference,
    pub channels: ChannelPreference,
    pub mp3_quality: Mp3Quality,
    pub master_volume: f32,
    pub music_volume: f32,
    pub loudness_normalization: bool,
    pub metronome_volume: f32,
    pub metronome_sound: MetronomeSound,
    pub beat_this_dbn: bool,
    pub default_playback_rate: f64,
    pub default_pitch_semitones: f64,
    pub loop_load_position: LoopLoadPosition,
    pub loop_snap_enabled: bool,
    pub navigation_mode: NavigationMode,
    pub navigation_time_seconds: u32,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NavigationMode {
    Time,
    Beat,
    Chord,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: "en".into(),
            time_display: TimeDisplay::Simple,
            toast_duration_seconds: 3,
            concurrent_downloads: 3,
            youtube_auto_select_best_match: true,
            conversion_format: ConversionFormat::Mp3,
            sample_rate: SampleRatePreference::Preserve,
            channels: ChannelPreference::Stereo,
            mp3_quality: Mp3Quality::VbrHigh,
            master_volume: 1.0,
            music_volume: 1.0,
            loudness_normalization: true,
            metronome_volume: 0.55,
            metronome_sound: MetronomeSound::Electronic,
            beat_this_dbn: true,
            default_playback_rate: 1.0,
            default_pitch_semitones: 0.0,
            loop_load_position: LoopLoadPosition::Beginning,
            loop_snap_enabled: true,
            navigation_mode: NavigationMode::Time,
            navigation_time_seconds: 10,
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
    value.master_volume = value.master_volume.clamp(0.0, 2.0);
    value.music_volume = value.music_volume.clamp(0.0, 1.0);
    value.metronome_volume = value.metronome_volume.clamp(0.0, 1.0);
    value.default_playback_rate = value.default_playback_rate.clamp(0.5, 2.0);
    value.default_pitch_semitones = value.default_pitch_semitones.clamp(-12.0, 12.0);
    value.navigation_time_seconds = value.navigation_time_seconds.clamp(1, 60);
    value.default_trainer_start_rate = value.default_trainer_start_rate.clamp(0.5, 1.99);
    value.default_trainer_target_rate = value
        .default_trainer_target_rate
        .clamp(value.default_trainer_start_rate + 0.01, 2.0);
    value.default_trainer_repetitions = value.default_trainer_repetitions.clamp(1, 99);
    value.default_trainer_increment = value.default_trainer_increment.clamp(0.01, 0.25);
    const SUPPORTED_LANGUAGES: &[&str] = &[
        "en", "fr", "es", "de", "pt", "it", "zh", "ja", "ko", "ar", "hi", "id",
    ];
    if !SUPPORTED_LANGUAGES.contains(&value.language.as_str()) {
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
        assert!(preferences.youtube_auto_select_best_match);
        assert_eq!(preferences.time_display, TimeDisplay::Simple);
        assert_eq!(preferences.default_trainer_start_rate, 0.5);
        assert_eq!(preferences.default_trainer_target_rate, 1.0);
        assert_eq!(preferences.default_trainer_increment, 0.05);
        assert_eq!(preferences.default_trainer_repetitions, 1);
        assert_eq!(preferences.loop_load_position, LoopLoadPosition::Beginning);
        assert!(preferences.loop_snap_enabled);
        assert_eq!(preferences.navigation_mode, NavigationMode::Time);
        assert_eq!(preferences.navigation_time_seconds, 10);
        assert_eq!(preferences.metronome_sound, MetronomeSound::Electronic);
        assert!(preferences.beat_this_dbn);
        assert_eq!(preferences.master_volume, 1.0);
        assert!(preferences.loudness_normalization);
    }

    #[test]
    fn older_preferences_default_to_simplified_time() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("timeDisplay");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert_eq!(preferences.time_display, TimeDisplay::Simple);
    }

    #[test]
    fn older_preferences_default_to_the_electronic_metronome() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("metronomeSound");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert_eq!(preferences.metronome_sound, MetronomeSound::Electronic);
    }

    #[test]
    fn older_preferences_default_to_full_music_volume() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("musicVolume");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert_eq!(preferences.music_volume, 1.0);
    }

    #[test]
    fn older_preferences_enable_loudness_normalization() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored
            .as_object_mut()
            .unwrap()
            .remove("loudnessNormalization");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert!(preferences.loudness_normalization);
    }

    #[test]
    fn older_preferences_enable_loop_snap_by_default() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("loopSnapEnabled");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert!(preferences.loop_snap_enabled);
    }

    #[test]
    fn older_preferences_auto_select_the_best_youtube_match() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored
            .as_object_mut()
            .unwrap()
            .remove("youtubeAutoSelectBestMatch");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert!(preferences.youtube_auto_select_best_match);
    }

    #[test]
    fn older_preferences_use_ten_second_time_navigation() {
        let mut stored = serde_json::to_value(UserPreferences::default()).unwrap();
        stored.as_object_mut().unwrap().remove("navigationMode");
        stored
            .as_object_mut()
            .unwrap()
            .remove("navigationTimeSeconds");

        let preferences: UserPreferences = serde_json::from_value(stored).unwrap();

        assert_eq!(preferences.navigation_mode, NavigationMode::Time);
        assert_eq!(preferences.navigation_time_seconds, 10);
    }

    #[test]
    fn navigation_time_is_kept_within_the_supported_range() {
        let mut too_short = UserPreferences {
            navigation_time_seconds: 0,
            ..UserPreferences::default()
        };
        validate(&mut too_short);
        assert_eq!(too_short.navigation_time_seconds, 1);

        let mut too_long = UserPreferences {
            navigation_time_seconds: 120,
            ..UserPreferences::default()
        };
        validate(&mut too_long);
        assert_eq!(too_long.navigation_time_seconds, 60);
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

    #[test]
    fn supported_languages_are_preserved_and_unknown_languages_fall_back() {
        for language in [
            "en", "fr", "es", "de", "pt", "it", "zh", "ja", "ko", "ar", "hi", "id",
        ] {
            let mut preferences = UserPreferences {
                language: language.into(),
                ..UserPreferences::default()
            };
            validate(&mut preferences);
            assert_eq!(preferences.language, language);
        }
        let mut preferences = UserPreferences {
            language: "xx".into(),
            ..UserPreferences::default()
        };
        validate(&mut preferences);
        assert_eq!(preferences.language, "en");
    }
}
