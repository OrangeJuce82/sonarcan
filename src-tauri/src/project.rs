use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{audio, error::AppError};

pub const PROJECT_FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "project.json";

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

pub fn create_project(parent: &Path, name: &str) -> Result<ProjectSummary, AppError> {
    let manifest = ProjectManifest::create(name)?;
    let package_name = format!("{}.sac", sanitize_package_name(name));
    let package_path = parent.join(package_name);
    for directory in ["Audio", "Stems", "Analysis", "Chords", "Cache"] {
        let path = package_path.join(directory);
        fs::create_dir_all(&path).map_err(|error| AppError::io(path, error))?;
    }
    save(&package_path, &manifest)?;
    Ok(summary(package_path, manifest))
}

pub fn open_project(package_path: &Path) -> Result<ProjectSummary, AppError> {
    if package_path.extension().and_then(|value| value.to_str()) != Some("sac")
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
    for source_path in source_paths {
        if !source_path.is_file() {
            return Err(AppError::MissingSource(source_path.clone()));
        }
        let format = AudioFormat::from_path(source_path)?;
        let canonical = source_path
            .canonicalize()
            .map_err(|error| AppError::io(source_path, error))?;
        if manifest.tracks.iter().any(|track| {
            track.source_path == canonical
                || track.original_source_path.as_ref() == Some(&canonical)
        }) {
            continue;
        }
        let metadata = fs::metadata(&canonical).map_err(|error| AppError::io(&canonical, error))?;
        let audio_metadata = audio::probe(&canonical)?;
        let title = canonical
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        let track_id = Uuid::new_v4();
        let file_name = canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("audio");
        let imported_path = package_path
            .join("Audio")
            .join(format!("{track_id}-{file_name}"));
        fs::copy(&canonical, &imported_path)
            .map_err(|error| AppError::io(&imported_path, error))?;
        manifest.tracks.push(Track {
            id: track_id,
            title,
            source_path: imported_path,
            original_source_path: Some(canonical),
            format,
            file_size_bytes: metadata.len(),
            duration_seconds: audio_metadata.duration_seconds,
            sample_rate: audio_metadata.sample_rate,
            channels: audio_metadata.channels,
        });
    }
    manifest.updated_at = Utc::now();
    save(package_path, &manifest)?;
    Ok(summary(package_path.to_path_buf(), manifest))
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

pub fn save_as(
    source_package: &Path,
    parent_directory: &Path,
    name: &str,
) -> Result<ProjectSummary, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::EmptyProjectName);
    }
    let mut manifest = load(source_package)?;
    let destination = parent_directory.join(format!("{}.sac", sanitize_package_name(name)));
    if destination.exists() {
        return Err(AppError::ProjectAlreadyExists(destination));
    }
    copy_directory(source_package, &destination)?;
    manifest.name = name.to_owned();
    manifest.id = Uuid::new_v4();
    manifest.created_at = Utc::now();
    manifest.updated_at = manifest.created_at;
    for track in &mut manifest.tracks {
        if let Ok(relative) = track.source_path.strip_prefix(source_package) {
            track.source_path = destination.join(relative);
        }
    }
    save(&destination, &manifest)?;
    Ok(summary(destination, manifest))
}

fn load(package_path: &Path) -> Result<ProjectManifest, AppError> {
    let path = package_path.join(MANIFEST_NAME);
    let bytes = fs::read(&path).map_err(|error| AppError::io(&path, error))?;
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
    manifest
        .tracks
        .into_iter()
        .find(|track| track.id == track_id)
        .map(|track| track.source_path)
        .ok_or(AppError::TrackNotFound(track_id))
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
    ProjectSummary {
        name: manifest.name,
        package_path,
        format_version: manifest.format_version,
        track_count: manifest.tracks.len(),
        tracks: manifest.tracks,
    }
}

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
    fs::create_dir_all(destination).map_err(|error| AppError::io(destination, error))?;
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
    fn imports_audio_metadata_and_skips_duplicate_sources() {
        let temp = tempfile::tempdir().unwrap();
        let project = create_project(temp.path(), "Import Test").unwrap();
        let wave_path = temp.path().join("tone.wav");
        fs::write(&wave_path, minimal_pcm_wave()).unwrap();

        let imported = import_audio(
            &project.package_path,
            &[wave_path.clone(), wave_path.clone()],
        )
        .unwrap();

        assert_eq!(imported.track_count, 1);
        assert_eq!(imported.tracks[0].sample_rate, Some(8_000));
        assert_eq!(imported.tracks[0].channels, Some(1));
        assert!(imported.tracks[0]
            .source_path
            .starts_with(project.package_path.join("Audio")));
        assert!(imported.tracks[0].source_path.is_file());
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

        let copy = save_as(&project.package_path, temp.path(), "Practice Copy").unwrap();
        assert_eq!(copy.name, "Practice Copy");
        assert_ne!(copy.package_path, project.package_path);
        assert!(copy.tracks[0].source_path.is_file());
        assert!(copy.tracks[0].source_path.starts_with(&copy.package_path));
    }

    fn minimal_pcm_wave() -> Vec<u8> {
        let samples = [0_i16; 8];
        let data_size = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}
