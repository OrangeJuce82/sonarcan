use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    audio, audio_fingerprint,
    error::AppError,
    stem_contract::{STEM_COUNT, STEM_NAMES},
};

pub const PROJECT_FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "project.json";
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Wav,
    Mp3,
    Flac,
}

impl AudioFormat {
    pub fn from_path(path: &Path) -> Result<Self, AppError> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("wav") => Ok(Self::Wav),
            Some("mp3") => Ok(Self::Mp3),
            Some("flac") => Ok(Self::Flac),
            _ => Err(AppError::UnsupportedAudioFormat(path.to_path_buf())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Uuid,
    pub title: String,
    pub source_path: PathBuf,
    #[serde(default)]
    pub original_source_path: Option<PathBuf>,
    pub format: AudioFormat,
    pub file_size_bytes: u64,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    #[serde(default)]
    pub practice: PracticeState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PracticeState {
    pub position_seconds: f64,
    pub playback_rate: f64,
    #[serde(default)]
    pub pitch_semitones: f64,
    #[serde(default = "legacy_master_volume", skip_serializing)]
    pub volume: f64,
    #[serde(default = "legacy_loop_enabled")]
    pub loop_enabled: bool,
    pub loop_a_seconds: Option<f64>,
    pub loop_b_seconds: Option<f64>,
    #[serde(default)]
    pub metronome_enabled: bool,
    #[serde(default = "default_metronome_volume", skip_serializing)]
    pub metronome_volume: f64,
    #[serde(default)]
    pub trainer_enabled: bool,
    pub trainer_start_rate: f64,
    #[serde(default = "default_trainer_repetitions")]
    pub trainer_repetitions: u32,
    #[serde(default = "default_trainer_increment")]
    pub trainer_increment: f64,
    #[serde(default = "default_trainer_target_rate")]
    pub trainer_target_rate: f64,
    pub stems_enabled: bool,
    pub stem_mix: [StemMixState; STEM_COUNT],
    pub stem_names: [String; STEM_COUNT],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StemMixState {
    pub gain: f64,
    pub pan: f64,
    pub muted: bool,
    pub soloed: bool,
}
impl Default for StemMixState {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }
    }
}
fn default_stem_mix() -> [StemMixState; STEM_COUNT] {
    std::array::from_fn(|_| StemMixState::default())
}

fn default_stem_names() -> [String; STEM_COUNT] {
    STEM_NAMES.map(str::to_owned)
}

const fn legacy_master_volume() -> f64 {
    0.8
}

impl Default for PracticeState {
    fn default() -> Self {
        Self {
            position_seconds: 0.0,
            playback_rate: 1.0,
            pitch_semitones: 0.0,
            volume: 0.8,
            loop_enabled: false,
            loop_a_seconds: None,
            loop_b_seconds: None,
            metronome_enabled: false,
            metronome_volume: default_metronome_volume(),
            trainer_enabled: false,
            trainer_start_rate: default_trainer_start_rate(),
            trainer_repetitions: default_trainer_repetitions(),
            trainer_increment: default_trainer_increment(),
            trainer_target_rate: default_trainer_target_rate(),
            stems_enabled: false,
            stem_mix: default_stem_mix(),
            stem_names: default_stem_names(),
        }
    }
}

// Projects written before loop toggling was introduced had an active loop whenever
// A and B were present. Defaulting missing fields to true preserves that behaviour.
const fn legacy_loop_enabled() -> bool {
    true
}

const fn default_metronome_volume() -> f64 {
    0.55
}

const fn default_trainer_repetitions() -> u32 {
    1
}

const fn default_trainer_start_rate() -> f64 {
    0.5
}

const fn default_trainer_increment() -> f64 {
    0.05
}

const fn default_trainer_target_rate() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub format_version: u32,
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub package_path: PathBuf,
    pub temporary: bool,
    pub format_version: u32,
    pub track_count: usize,
    pub tracks: Vec<Track>,
}

impl ProjectManifest {
    pub fn create(name: &str) -> Result<Self, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::EmptyProjectName);
        }
        let now = Utc::now();
        Ok(Self {
            format_version: PROJECT_FORMAT_VERSION,
            id: Uuid::new_v4(),
            name: name.to_owned(),
            created_at: now,
            updated_at: now,
            tracks: Vec::new(),
        })
    }
}

#[cfg(test)]
pub fn create_project(parent: &Path, name: &str) -> Result<ProjectSummary, AppError> {
    let manifest = ProjectManifest::create(name)?;
    let package_name = format!("{}.sac", sanitize_package_name(name));
    let package_path = parent.join(package_name);
    create_project_package(&package_path, manifest)
}

pub fn create_project_at(selected_path: &Path) -> Result<ProjectSummary, AppError> {
    let package_path = if selected_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("sac"))
    {
        selected_path.to_path_buf()
    } else {
        selected_path.with_extension("sac")
    };
    let name = package_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(AppError::EmptyProjectName)?;
    let manifest = ProjectManifest::create(name)?;
    if package_path.exists() {
        return Err(AppError::ProjectAlreadyExists(package_path));
    }
    create_project_package(&package_path, manifest)
}

pub fn create_temporary_project() -> Result<ProjectSummary, AppError> {
    let package_path = std::env::temp_dir().join(format!("sonarcan-{}.sac", Uuid::new_v4()));
    create_project_package(&package_path, ProjectManifest::create("New Project")?)
}

fn create_project_package(
    package_path: &Path,
    manifest: ProjectManifest,
) -> Result<ProjectSummary, AppError> {
    match fs::create_dir(package_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AppError::ProjectAlreadyExists(package_path.to_path_buf()));
        }
        Err(error) => return Err(AppError::io(package_path, error)),
    }
    for directory in ["Audio", "Stems", "Analysis", "Chords", "Cache"] {
        let path = package_path.join(directory);
        fs::create_dir(&path).map_err(|error| AppError::io(path, error))?;
    }
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn open_project(package_path: &Path) -> Result<ProjectSummary, AppError> {
    if !package_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("sac"))
        || !package_path.is_dir()
    {
        return Err(AppError::InvalidProjectPackage(package_path.to_path_buf()));
    }
    let manifest = load(package_path)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn import_audio(
    package_path: &Path,
    source_paths: &[PathBuf],
) -> Result<ProjectSummary, AppError> {
    let mut manifest = load(package_path)?;
    fingerprint_cache_directory(package_path)?;
    let mut existing_fingerprints = Vec::with_capacity(manifest.tracks.len());
    for track in &manifest.tracks {
        let cache_path = fingerprint_cache_path(package_path, track.id)?;
        let fingerprint = match audio_fingerprint::load(&cache_path)? {
            Some(fingerprint) => fingerprint,
            None => {
                let media_path = validated_media_path(package_path, &track.source_path)?;
                let fingerprint = audio_fingerprint::calculate(&media_path)?;
                audio_fingerprint::save(&cache_path, &fingerprint)?;
                fingerprint
            }
        };
        existing_fingerprints.push((track.id, fingerprint));
    }

    let mut prepared = Vec::with_capacity(source_paths.len());
    for source_path in source_paths {
        if !source_path.is_file() {
            return Err(AppError::MissingSource(source_path.clone()));
        }
        let format = AudioFormat::from_path(source_path)?;
        let canonical = source_path
            .canonicalize()
            .map_err(|error| AppError::io(source_path, error))?;
        if let Some(track) = manifest.tracks.iter().find(|track| {
            track.source_path == canonical
                || track.original_source_path.as_ref() == Some(&canonical)
        }) {
            return Err(AppError::DuplicateAudio {
                incoming: canonical,
                existing_title: track.title.clone(),
            });
        }
        let metadata = fs::metadata(&canonical).map_err(|error| AppError::io(&canonical, error))?;
        let audio_metadata = audio::probe(&canonical)?;
        let title = canonical
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        let fingerprint = audio_fingerprint::calculate(&canonical)?;
        if let Some(track) = manifest.tracks.iter().find(|track| {
            existing_fingerprints
                .iter()
                .find(|(track_id, _)| *track_id == track.id)
                .is_some_and(|(_, existing)| {
                    audio_fingerprint::are_duplicates(
                        &fingerprint,
                        audio_metadata.duration_seconds,
                        existing,
                        track.duration_seconds,
                    )
                })
        }) {
            return Err(AppError::DuplicateAudio {
                incoming: canonical,
                existing_title: track.title.clone(),
            });
        }
        if let Some(existing) = prepared.iter().find(|existing: &&PreparedImport| {
            existing.canonical == canonical
                || audio_fingerprint::are_duplicates(
                    &fingerprint,
                    audio_metadata.duration_seconds,
                    &existing.fingerprint,
                    existing.audio_metadata.duration_seconds,
                )
        }) {
            return Err(AppError::DuplicateAudio {
                incoming: canonical,
                existing_title: existing.title.clone(),
            });
        }
        prepared.push(PreparedImport {
            canonical,
            format,
            file_size_bytes: metadata.len(),
            audio_metadata,
            title,
            fingerprint,
        });
    }

    for prepared in prepared {
        let track_id = Uuid::new_v4();
        let file_name = prepared
            .canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("audio");
        let imported_path = package_path
            .join("Audio")
            .join(format!("{track_id}-{file_name}"));
        fs::copy(&prepared.canonical, &imported_path)
            .map_err(|error| AppError::io(&imported_path, error))?;
        audio_fingerprint::save(
            &fingerprint_cache_path(package_path, track_id)?,
            &prepared.fingerprint,
        )?;
        manifest.tracks.push(Track {
            id: track_id,
            title: prepared.title,
            source_path: imported_path,
            original_source_path: Some(prepared.canonical),
            format: prepared.format,
            file_size_bytes: prepared.file_size_bytes,
            duration_seconds: prepared.audio_metadata.duration_seconds,
            sample_rate: prepared.audio_metadata.sample_rate,
            channels: prepared.audio_metadata.channels,
            practice: PracticeState::default(),
        });
    }
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

struct PreparedImport {
    canonical: PathBuf,
    format: AudioFormat,
    file_size_bytes: u64,
    audio_metadata: audio::AudioMetadata,
    title: String,
    fingerprint: audio_fingerprint::AudioFingerprint,
}

fn fingerprint_cache_path(package_path: &Path, track_id: Uuid) -> Result<PathBuf, AppError> {
    let directory = fingerprint_cache_directory(package_path)?;
    let path = directory.join(format!("{track_id}.json"));
    if path.exists() {
        let canonical_path = path
            .canonicalize()
            .map_err(|error| AppError::io(&path, error))?;
        if !canonical_path.starts_with(&directory) {
            return Err(AppError::AnalysisCacheOutsideProject(path));
        }
    }
    Ok(path)
}

fn fingerprint_cache_directory(package_path: &Path) -> Result<PathBuf, AppError> {
    let canonical_package = package_path
        .canonicalize()
        .map_err(|error| AppError::io(package_path, error))?;
    let analysis_directory = package_path.join("Analysis");
    let canonical_analysis = analysis_directory
        .canonicalize()
        .map_err(|error| AppError::io(&analysis_directory, error))?;
    if !canonical_analysis.starts_with(&canonical_package) {
        return Err(AppError::AnalysisCacheOutsideProject(analysis_directory));
    }
    let directory = analysis_directory.join("fingerprints");
    if directory.exists() {
        let canonical_directory = directory
            .canonicalize()
            .map_err(|error| AppError::io(&directory, error))?;
        if !canonical_directory.starts_with(&canonical_analysis) {
            return Err(AppError::AnalysisCacheOutsideProject(directory));
        }
        Ok(canonical_directory)
    } else {
        fs::create_dir(&directory).map_err(|error| AppError::io(&directory, error))?;
        directory
            .canonicalize()
            .map_err(|error| AppError::io(&directory, error))
    }
}

pub fn rename_project(package_path: &Path, name: &str) -> Result<ProjectSummary, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::EmptyProjectName);
    }
    let mut manifest = load(package_path)?;
    manifest.name = name.to_owned();
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn rename_track(
    package_path: &Path,
    track_id: Uuid,
    name: &str,
) -> Result<ProjectSummary, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::EmptyTrackName);
    }
    let mut manifest = load(package_path)?;
    let track = manifest
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
        .ok_or(AppError::TrackNotFound(track_id))?;
    track.title = name.to_owned();
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn reorder_track(
    package_path: &Path,
    track_id: Uuid,
    new_index: usize,
) -> Result<ProjectSummary, AppError> {
    let mut manifest = load(package_path)?;
    let old_index = manifest
        .tracks
        .iter()
        .position(|track| track.id == track_id)
        .ok_or(AppError::TrackNotFound(track_id))?;
    let track = manifest.tracks.remove(old_index);
    let destination = new_index.min(manifest.tracks.len());
    manifest.tracks.insert(destination, track);
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn delete_track(package_path: &Path, track_id: Uuid) -> Result<ProjectSummary, AppError> {
    let mut manifest = load(package_path)?;
    let index = manifest
        .tracks
        .iter()
        .position(|track| track.id == track_id)
        .ok_or(AppError::TrackNotFound(track_id))?;
    let media_path = validated_media_path(package_path, &manifest.tracks[index].source_path)?;
    let fingerprint_path = fingerprint_cache_path(package_path, track_id)?;
    manifest.tracks.remove(index);
    if media_path.is_file() {
        fs::remove_file(&media_path).map_err(|error| AppError::io(&media_path, error))?;
    }
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;

    for cache_path in [
        package_path
            .join("Analysis")
            .join("waveform")
            .join(format!("{track_id}.json")),
        package_path
            .join("Analysis")
            .join("tempo")
            .join(format!("{track_id}.json")),
        fingerprint_path,
        media_path
            .file_name()
            .map(|name| {
                package_path
                    .join("Cache")
                    .join("decoded")
                    .join(format!("{}.pcm", name.to_string_lossy()))
            })
            .unwrap_or_default(),
    ] {
        let _ = fs::remove_file(cache_path);
    }
    let _ = fs::remove_dir_all(package_path.join("Stems").join(track_id.to_string()));
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn export_playlist(
    package_path: &Path,
    destination: &Path,
    format: &str,
) -> Result<(), AppError> {
    let manifest = load(package_path)?;
    let contents = match format {
        "json" => {
            let tracks = manifest
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    serde_json::json!({
                        "position": index + 1,
                        "title": track.title,
                        "durationSeconds": track.duration_seconds,
                    })
                })
                .collect::<Vec<_>>();
            serde_json::to_string_pretty(&serde_json::json!({
                "project": manifest.name,
                "tracks": tracks,
            }))?
        }
        "markdown" => {
            let mut text = format!(
                "# {}\n\n| # | Song | Duration | Notes |\n|---:|---|---:|---|\n",
                manifest.name
            );
            for (index, track) in manifest.tracks.iter().enumerate() {
                let title = track.title.replace('|', "\\|").replace('\n', " ");
                let duration = track
                    .duration_seconds
                    .map(format_duration)
                    .unwrap_or_default();
                text.push_str(&format!(
                    "| {} | {} | {} |  |\n",
                    index + 1,
                    title,
                    duration
                ));
            }
            text
        }
        _ => {
            return Err(AppError::BackgroundTask(
                "unsupported playlist export format".into(),
            ))
        }
    };
    fs::write(destination, contents).map_err(|error| AppError::io(destination, error))
}

fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

pub fn update_practice_state(
    package_path: &Path,
    track_id: Uuid,
    state: PracticeState,
) -> Result<ProjectSummary, AppError> {
    validate_practice_state(&state)?;
    let mut manifest = load(package_path)?;
    let track = manifest
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
        .ok_or(AppError::TrackNotFound(track_id))?;
    track.practice = state;
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
}

pub fn save_as_to(
    source_package: &Path,
    selected_destination: &Path,
) -> Result<ProjectSummary, AppError> {
    let destination = if selected_destination
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("sac"))
    {
        selected_destination.to_path_buf()
    } else {
        selected_destination.with_extension("sac")
    };
    if destination.exists() {
        return Err(AppError::ProjectAlreadyExists(destination));
    }
    let source_package = source_package
        .canonicalize()
        .map_err(|error| AppError::io(source_package, error))?;
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::InvalidProjectDestination(destination.clone()))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| AppError::io(parent, error))?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| AppError::InvalidProjectDestination(destination.clone()))?;
    let destination = canonical_parent.join(file_name);
    if destination.starts_with(&source_package) {
        return Err(AppError::InvalidProjectDestination(destination));
    }
    let name = destination
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::InvalidProjectDestination(destination.clone()))?;
    let mut manifest = load(&source_package)?;
    let relative_media_paths = manifest
        .tracks
        .iter()
        .map(|track| {
            validated_media_path(&source_package, &track.source_path).and_then(|path| {
                path.strip_prefix(&source_package)
                    .map(Path::to_path_buf)
                    .map_err(|_| AppError::TrackMediaOutsideProject(path))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    copy_directory(&source_package, &destination)?;
    manifest.name = name.to_owned();
    manifest.id = Uuid::new_v4();
    manifest.created_at = Utc::now();
    manifest.updated_at = manifest.created_at;
    for (track, relative) in manifest.tracks.iter_mut().zip(relative_media_paths) {
        track.source_path = destination.join(relative);
    }
    save(&destination, &manifest)?;
    Ok(summary(destination, manifest))
}

fn load(package_path: &Path) -> Result<ProjectManifest, AppError> {
    let path = package_path.join(MANIFEST_NAME);
    let file = fs::File::open(&path).map_err(|error| AppError::io(&path, error))?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::io(&path, error))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(AppError::ProjectManifestTooLarge(path));
    }
    let manifest: ProjectManifest =
        serde_json::from_slice(&bytes).map_err(|source| AppError::InvalidProjectMetadata {
            path: path.clone(),
            source,
        })?;
    if manifest.format_version != PROJECT_FORMAT_VERSION {
        return Err(AppError::UnsupportedProjectVersion {
            found: manifest.format_version,
            supported: PROJECT_FORMAT_VERSION,
        });
    }
    Ok(manifest)
}

pub fn track_media_path(package_path: &Path, track_id: Uuid) -> Result<PathBuf, AppError> {
    let manifest = load(package_path)?;
    let source_path = manifest
        .tracks
        .into_iter()
        .find(|track| track.id == track_id)
        .map(|track| track.source_path)
        .ok_or(AppError::TrackNotFound(track_id))?;
    validated_media_path(package_path, &source_path)
}

fn validated_media_path(package_path: &Path, source_path: &Path) -> Result<PathBuf, AppError> {
    let audio_directory = package_path
        .join("Audio")
        .canonicalize()
        .map_err(|error| AppError::io(package_path.join("Audio"), error))?;
    let media_path = source_path
        .canonicalize()
        .map_err(|error| AppError::io(source_path, error))?;
    if !media_path.starts_with(&audio_directory) || !media_path.is_file() {
        return Err(AppError::TrackMediaOutsideProject(
            source_path.to_path_buf(),
        ));
    }
    Ok(media_path)
}

fn save(package_path: &Path, manifest: &ProjectManifest) -> Result<(), AppError> {
    let target = package_path.join(MANIFEST_NAME);
    let temporary = package_path.join("project.json.tmp");
    let contents = serde_json::to_vec_pretty(manifest)?;
    fs::write(&temporary, contents).map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &target).map_err(|error| AppError::io(&target, error))?;
    Ok(())
}

fn summary(package_path: PathBuf, manifest: ProjectManifest) -> ProjectSummary {
    let temporary = is_temporary_project_path(&package_path);
    ProjectSummary {
        name: manifest.name,
        package_path,
        temporary,
        format_version: manifest.format_version,
        track_count: manifest.tracks.len(),
        tracks: manifest.tracks,
    }
}

pub fn is_temporary_project_path(path: &Path) -> bool {
    let temporary_directory = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    let project_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    project_path.starts_with(temporary_directory)
}

#[cfg(test)]
fn sanitize_package_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect();
    sanitized.trim().replace(' ', "-")
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), AppError> {
    fs::create_dir(destination).map_err(|error| AppError::io(destination, error))?;
    let entries = fs::read_dir(source).map_err(|error| AppError::io(source, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| AppError::io(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| AppError::io(&source_path, error))?;
        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| AppError::io(&destination_path, error))?;
        }
    }
    Ok(())
}

fn validate_practice_state(state: &PracticeState) -> Result<(), AppError> {
    let finite = [
        state.position_seconds,
        state.playback_rate,
        state.pitch_semitones,
        state.volume,
        state.metronome_volume,
        state.trainer_start_rate,
        state.trainer_increment,
        state.trainer_target_rate,
    ]
    .into_iter()
    .all(f64::is_finite)
        && optional_f64_is_finite(state.loop_a_seconds)
        && optional_f64_is_finite(state.loop_b_seconds);
    if !finite
        || state.position_seconds < 0.0
        || !(0.5..=2.0).contains(&state.playback_rate)
        || !(-12.0..=12.0).contains(&state.pitch_semitones)
        || !(0.0..=1.0).contains(&state.volume)
        || !(0.0..=1.0).contains(&state.metronome_volume)
        || !(1..=99).contains(&state.trainer_repetitions)
        || !(0.5..2.0).contains(&state.trainer_start_rate)
        || !(0.01..=0.25).contains(&state.trainer_increment)
        || !(0.5..=2.0).contains(&state.trainer_target_rate)
        || state.trainer_start_rate >= state.trainer_target_rate
        || state.stem_mix.iter().any(|stem| {
            !stem.gain.is_finite()
                || !(0.0..=2.0).contains(&stem.gain)
                || !stem.pan.is_finite()
                || !(-1.0..=1.0).contains(&stem.pan)
        })
        || state.stem_names.iter().any(|name| {
            let name = name.trim();
            name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control)
        })
    {
        return Err(AppError::InvalidPracticeState(
            "values are outside the supported range".to_owned(),
        ));
    }
    if let (Some(a), Some(b)) = (state.loop_a_seconds, state.loop_b_seconds) {
        if a < 0.0 || b <= a {
            return Err(AppError::InvalidPracticeState(
                "loop B must be greater than loop A".to_owned(),
            ));
        }
    }
    Ok(())
}

fn optional_f64_is_finite(value: Option<f64>) -> bool {
    match value {
        Some(value) => value.is_finite(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reopens_a_project_package() {
        let temp = tempfile::tempdir().unwrap();
        let created = create_project(temp.path(), "Test Band").unwrap();
        assert_eq!(created.package_path.file_name().unwrap(), "Test-Band.sac");
        let reopened = open_project(&created.package_path).unwrap();
        assert_eq!(reopened.name, "Test Band");
        assert_eq!(reopened.format_version, PROJECT_FORMAT_VERSION);
    }

    #[test]
    fn creates_project_structure_from_a_selected_file_path() {
        let temp = tempfile::tempdir().unwrap();
        let selected_path = temp.path().join("Rehearsal.sac");

        let created = create_project_at(&selected_path).unwrap();

        assert_eq!(created.package_path, selected_path);
        for directory in ["Audio", "Stems", "Analysis", "Chords", "Cache"] {
            assert!(selected_path.join(directory).is_dir());
        }
        assert!(selected_path.join("project.json").is_file());
    }

    #[test]
    fn creates_a_random_project_in_the_platform_temporary_directory() {
        let created = create_temporary_project().unwrap();
        let canonical_temporary = std::env::temp_dir().canonicalize().unwrap();
        let canonical_project = created.package_path.canonicalize().unwrap();

        assert!(created.temporary);
        assert!(canonical_project.starts_with(canonical_temporary));
        assert_eq!(
            created
                .package_path
                .extension()
                .and_then(|value| value.to_str()),
            Some("sac")
        );
        assert_eq!(created.name, "New Project");

        fs::remove_dir_all(created.package_path).unwrap();
    }

    #[test]
    fn accepts_only_v1_audio_formats() {
        assert_eq!(
            AudioFormat::from_path(Path::new("song.wav")).unwrap(),
            AudioFormat::Wav
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.MP3")).unwrap(),
            AudioFormat::Mp3
        );
        assert!(AudioFormat::from_path(Path::new("song.aac")).is_err());
    }

    #[test]
    fn rejects_an_empty_project_name() {
        assert!(matches!(
            ProjectManifest::create("  "),
            Err(AppError::EmptyProjectName)
        ));
    }

    #[test]
    fn imports_audio_metadata_and_rejects_a_duplicate_source() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Import Test").unwrap();
        let wave_path = temp.path().join("tone.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();

        let imported =
            import_audio(&project.package_path, std::slice::from_ref(&wave_path)).unwrap();
        let duplicate = import_audio(&project.package_path, std::slice::from_ref(&wave_path));

        assert_eq!(imported.track_count, 1);
        assert!(matches!(duplicate, Err(AppError::DuplicateAudio { .. })));
        assert_eq!(imported.tracks[0].sample_rate, Some(8_000));
        assert_eq!(imported.tracks[0].channels, Some(1));
        assert!(imported.tracks[0]
            .source_path
            .starts_with(project.package_path.join("Audio")));
        assert!(imported.tracks[0].source_path.is_file());
    }

    #[test]
    fn rejects_acoustically_equivalent_audio_with_lossy_sample_changes() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Fingerprint Test").unwrap();
        let original_path = temp.path().join("original.wav");
        let converted_path = temp.path().join("converted.wav");
        fs::write(&original_path, synthetic_music_wave(0)).unwrap();
        fs::write(&converted_path, synthetic_music_wave(48)).unwrap();

        import_audio(&project.package_path, std::slice::from_ref(&original_path)).unwrap();
        let duplicate = import_audio(&project.package_path, std::slice::from_ref(&converted_path));

        assert!(matches!(
            duplicate,
            Err(AppError::DuplicateAudio { existing_title, .. }) if existing_title == "original"
        ));
        assert_eq!(open_project(&project.package_path).unwrap().track_count, 1);
    }

    #[test]
    fn rejects_the_same_recording_after_wav_to_mp3_conversion() {
        if !std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Transcode Fingerprint Test").unwrap();
        let wave_path = temp.path().join("recording.wav");
        let mp3_path = temp.path().join("other-name.mp3");
        fs::write(&wave_path, synthetic_music_wave(0)).unwrap();
        let status = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&wave_path)
            .args(["-b:a", "192k"])
            .arg(&mp3_path)
            .status()
            .unwrap();
        assert!(status.success());

        import_audio(&project.package_path, std::slice::from_ref(&wave_path)).unwrap();
        let duplicate = import_audio(&project.package_path, std::slice::from_ref(&mp3_path));

        assert!(matches!(duplicate, Err(AppError::DuplicateAudio { .. })));
        assert_eq!(open_project(&project.package_path).unwrap().track_count, 1);
    }

    #[test]
    fn renames_projects_and_tracks_and_saves_a_copy() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Original").unwrap();
        let renamed = rename_project(&project.package_path, "Renamed").unwrap();
        assert_eq!(renamed.name, "Renamed");

        let wave_path = temp.path().join("tone.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();
        let imported = import_audio(&project.package_path, &[wave_path]).unwrap();
        let track_id = imported.tracks[0].id;
        let renamed_track = rename_track(&project.package_path, track_id, "Warm-up").unwrap();
        assert_eq!(renamed_track.tracks[0].title, "Warm-up");

        let copy = save_as_to(
            &project.package_path,
            &temp.path().join("Practice Copy.sac"),
        )
        .unwrap();
        assert_eq!(copy.name, "Practice Copy");
        assert_ne!(copy.package_path, project.package_path);
        assert!(copy.tracks[0].source_path.is_file());
        assert!(copy.tracks[0].source_path.starts_with(&copy.package_path));
    }

    #[test]
    fn refuses_to_save_a_project_inside_itself() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Recursive Copy").unwrap();
        let destination = project.package_path.join("nested.sac");

        assert!(matches!(
            save_as_to(&project.package_path, &destination),
            Err(AppError::InvalidProjectDestination(_))
        ));
        assert!(!destination.exists());
    }

    #[test]
    fn persists_playlist_reordering() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Order").unwrap();
        let first = temp.path().join("first.wav");
        let second = temp.path().join("second.wav");
        fs::write(&first, minimal_pcm_wave()).unwrap();
        fs::write(&second, minimal_pcm_wave()).unwrap();
        let imported = import_audio(&project.package_path, &[first, second]).unwrap();
        let second_id = imported.tracks[1].id;

        reorder_track(&project.package_path, second_id, 0).unwrap();
        let reopened = open_project(&project.package_path).unwrap();

        assert_eq!(reopened.tracks[0].id, second_id);
    }

    #[test]
    fn deletes_a_track_and_its_media() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Delete").unwrap();
        let first = temp.path().join("first.wav");
        let second = temp.path().join("second.wav");
        fs::write(&first, minimal_pcm_wave()).unwrap();
        fs::write(&second, minimal_pcm_wave()).unwrap();
        let imported = import_audio(&project.package_path, &[first, second]).unwrap();
        let deleted_id = imported.tracks[0].id;
        let deleted_path = imported.tracks[0].source_path.clone();

        let updated = delete_track(&project.package_path, deleted_id).unwrap();

        assert_eq!(updated.track_count, 1);
        assert!(!deleted_path.exists());
        assert!(updated.tracks.iter().all(|track| track.id != deleted_id));
    }

    #[test]
    fn rejects_manifest_path_traversal_before_read_or_delete() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Untrusted Manifest").unwrap();
        let manifest_path = project.package_path.join(MANIFEST_NAME);
        let traversal_path = project
            .package_path
            .join("Audio")
            .join("..")
            .join(MANIFEST_NAME);
        let track_id = Uuid::new_v4();
        let mut manifest = load(&project.package_path).unwrap();
        manifest.tracks.push(Track {
            id: track_id,
            title: "Traversal".to_owned(),
            source_path: traversal_path.clone(),
            original_source_path: None,
            format: AudioFormat::Wav,
            file_size_bytes: manifest_path.metadata().unwrap().len(),
            duration_seconds: None,
            sample_rate: None,
            channels: None,
            practice: PracticeState::default(),
        });
        save(&project.package_path, &manifest).unwrap();

        assert!(matches!(
            track_media_path(&project.package_path, track_id),
            Err(AppError::TrackMediaOutsideProject(path)) if path == traversal_path
        ));
        assert!(matches!(
            delete_track(&project.package_path, track_id),
            Err(AppError::TrackMediaOutsideProject(path)) if path == traversal_path
        ));
        assert!(manifest_path.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_analysis_cache_symlink_outside_the_project() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Untrusted Cache").unwrap();
        let analysis_path = project.package_path.join("Analysis");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::remove_dir(&analysis_path).unwrap();
        symlink(&outside, &analysis_path).unwrap();
        let wave_path = temp.path().join("tone.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();

        let imported = import_audio(&project.package_path, &[wave_path]);

        assert!(matches!(
            imported,
            Err(AppError::AnalysisCacheOutsideProject(path)) if path == analysis_path
        ));
        assert!(outside.read_dir().unwrap().next().is_none());
        assert_eq!(open_project(&project.package_path).unwrap().track_count, 0);
    }

    #[test]
    fn rejects_oversized_project_manifests_before_parsing() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Oversized Manifest").unwrap();
        let manifest_path = project.package_path.join(MANIFEST_NAME);
        fs::write(&manifest_path, vec![b' '; MAX_MANIFEST_BYTES as usize + 1]).unwrap();

        assert!(matches!(
            open_project(&project.package_path),
            Err(AppError::ProjectManifestTooLarge(path)) if path == manifest_path
        ));
    }

    #[test]
    fn exports_a_portable_and_printable_playlist() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Stage Set").unwrap();
        let wave_path = temp.path().join("first.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();
        import_audio(&project.package_path, &[wave_path]).unwrap();

        let json_path = temp.path().join("setlist.json");
        let markdown_path = temp.path().join("setlist.md");
        export_playlist(&project.package_path, &json_path, "json").unwrap();
        export_playlist(&project.package_path, &markdown_path, "markdown").unwrap();

        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(json_path).unwrap()).unwrap();
        assert_eq!(json["project"], "Stage Set");
        assert_eq!(json["tracks"][0]["title"], "first");
        let markdown = fs::read_to_string(markdown_path).unwrap();
        assert!(markdown.contains("# Stage Set"));
        assert!(markdown.contains("| 1 | first |"));
    }

    #[test]
    fn persists_and_validates_track_practice_state() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Practice State").unwrap();
        let wave_path = temp.path().join("tone.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();
        let imported = import_audio(&project.package_path, &[wave_path]).unwrap();
        let track_id = imported.tracks[0].id;
        let state = PracticeState {
            position_seconds: 0.0005,
            playback_rate: 0.75,
            pitch_semitones: -2.0,
            volume: 0.6,
            loop_enabled: true,
            loop_a_seconds: Some(0.0002),
            loop_b_seconds: Some(0.0008),
            metronome_enabled: true,
            metronome_volume: 0.5,
            trainer_enabled: true,
            trainer_start_rate: 0.5,
            trainer_repetitions: 4,
            trainer_increment: 0.05,
            trainer_target_rate: 1.1,
            stems_enabled: true,
            stem_mix: default_stem_mix(),
            stem_names: default_stem_names(),
        };

        update_practice_state(&project.package_path, track_id, state.clone()).unwrap();
        let reopened = open_project(&project.package_path).unwrap();
        let expected = PracticeState {
            // Master and metronome volumes are global user preferences and are
            // deliberately excluded from track persistence.
            volume: legacy_master_volume(),
            metronome_volume: default_metronome_volume(),
            ..state.clone()
        };
        assert_eq!(reopened.tracks[0].practice, expected);

        let invalid = PracticeState {
            playback_rate: 3.0,
            ..state.clone()
        };
        assert!(matches!(
            update_practice_state(&project.package_path, track_id, invalid),
            Err(AppError::InvalidPracticeState(_))
        ));

        let invalid_training_range = PracticeState {
            trainer_start_rate: 1.2,
            trainer_target_rate: 1.0,
            ..state
        };
        assert!(matches!(
            update_practice_state(&project.package_path, track_id, invalid_training_range),
            Err(AppError::InvalidPracticeState(_))
        ));
    }

    #[test]
    fn rejects_the_obsolete_four_stem_contract_without_migration() {
        let mut value = serde_json::to_value(PracticeState::default()).unwrap();
        value["stemMix"] = serde_json::json!([
            StemMixState::default(),
            StemMixState::default(),
            StemMixState::default(),
            StemMixState::default()
        ]);
        assert!(serde_json::from_value::<PracticeState>(value).is_err());
    }

    #[test]
    fn legacy_beat_grid_fields_are_read_but_not_rewritten() {
        let mut value = serde_json::to_value(PracticeState::default()).unwrap();
        value["gridBpm"] = serde_json::json!(123.4);
        value["beatGridOffsetSeconds"] = serde_json::json!(1.25);

        let state = serde_json::from_value::<PracticeState>(value).unwrap();
        let rewritten = serde_json::to_value(state).unwrap();

        assert!(rewritten.get("gridBpm").is_none());
        assert!(rewritten.get("beatGridOffsetSeconds").is_none());
    }

    #[test]
    fn new_six_stem_mix_starts_centered_with_canonical_names() {
        let state = PracticeState::default();
        assert!(state.stem_mix.iter().all(|stem| stem.pan == 0.0));
        assert_eq!(state.stem_names, default_stem_names());
    }

    #[test]
    fn rejects_invalid_stem_pan_and_names() {
        let mut invalid_pan = PracticeState::default();
        invalid_pan.stem_mix[2].pan = 1.01;
        assert!(validate_practice_state(&invalid_pan).is_err());

        let mut invalid_name = PracticeState::default();
        invalid_name.stem_names[4] = "  ".into();
        assert!(validate_practice_state(&invalid_name).is_err());
    }

    fn minimal_pcm_wave() -> Vec<u8> {
        let samples = [0_i16; 8];
        pcm_wave(&samples, 8_000, 1)
    }

    fn synthetic_music_wave(quantization: i16) -> Vec<u8> {
        let sample_rate = 44_100_u32;
        let channels = 2_u16;
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
        pcm_wave(&samples, sample_rate, channels)
    }

    fn pcm_wave(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let data_size = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples.iter().copied() {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}
