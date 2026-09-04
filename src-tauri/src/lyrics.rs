use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::AppError, project};

const DOCUMENT_VERSION: u32 = 1;
const MAX_LYRICS_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LINES: usize = 10_000;
const MAX_WORDS: usize = 100_000;
const MAX_TEXT_CHARS: usize = 4_096;
const MAX_SEARCH_RESULTS_BYTES: u64 = 4 * 1024 * 1024;
const LRCLIB_BASE_URL: &str = "https://lrclib.net/api";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LyricsProvider {
    Local,
    Lrclib,
    Musixmatch,
    Lyricfind,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResult {
    pub id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration_seconds: f64,
    pub instrumental: bool,
    pub has_synced_lyrics: bool,
    pub has_plain_lyrics: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLyricsRecord {
    #[serde(flatten)]
    pub summary: LyricsSearchResult,
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    id: u64,
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: String,
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LyricsSyncLevel {
    None,
    Line,
    Word,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsExportFormat {
    Lrc,
    Markdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub text: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    #[serde(default)]
    pub words: Vec<LyricsWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDocument {
    pub version: u32,
    pub provider: LyricsProvider,
    pub provider_track_id: Option<String>,
    pub language: String,
    pub sync_level: LyricsSyncLevel,
    pub attribution: Option<String>,
    pub copyright: Option<String>,
    pub offset_ms: i32,
    pub lines: Vec<LyricsLine>,
}

pub fn load(package_path: &Path, track_id: Uuid) -> Result<Option<LyricsDocument>, AppError> {
    project::track_media_path(package_path, track_id)?;
    let path = lyrics_path(package_path, track_id, false)?;
    if !path.is_file() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|error| AppError::io(&path, error))?
        .take(MAX_LYRICS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::io(&path, error))?;
    if bytes.len() as u64 > MAX_LYRICS_BYTES {
        return Err(AppError::LyricsTooLarge(path));
    }
    let document: LyricsDocument =
        serde_json::from_slice(&bytes).map_err(|source| AppError::InvalidLyrics {
            path: path.clone(),
            source,
        })?;
    validate(&document)?;
    validate_duration(
        &document,
        project::track_duration_seconds(package_path, track_id)?,
    )?;
    Ok(Some(document))
}

pub fn save(
    package_path: &Path,
    track_id: Uuid,
    document: LyricsDocument,
) -> Result<LyricsDocument, AppError> {
    project::track_media_path(package_path, track_id)?;
    validate(&document)?;
    validate_duration(
        &document,
        project::track_duration_seconds(package_path, track_id)?,
    )?;
    let path = lyrics_path(package_path, track_id, true)?;
    let contents = serde_json::to_vec_pretty(&document)?;
    if contents.len() as u64 > MAX_LYRICS_BYTES {
        return Err(AppError::LyricsTooLarge(path));
    }
    let temporary = path.with_extension(format!("json.{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, contents).map_err(|error| AppError::io(&temporary, error))?;
    fs::rename(&temporary, &path).map_err(|error| AppError::io(&path, error))?;
    Ok(document)
}

pub fn remove(package_path: &Path, track_id: Uuid) -> Result<(), AppError> {
    project::track_media_path(package_path, track_id)?;
    let path = lyrics_path(package_path, track_id, false)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::io(path, error)),
    }
}

pub fn export(
    destination: &Path,
    title: &str,
    document: &LyricsDocument,
    format: LyricsExportFormat,
) -> Result<(), AppError> {
    validate(document)?;
    let title = title.replace(['\r', '\n'], " ");
    if title.trim().is_empty() || title.chars().count() > 512 {
        return Err(AppError::InvalidLyricsData(
            "the lyrics export title is invalid".into(),
        ));
    }
    let contents = match format {
        LyricsExportFormat::Lrc => export_lrc(document),
        LyricsExportFormat::Markdown => {
            let body = document
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("  \n");
            format!("# {}\n\n{body}\n", title.trim())
        }
    };
    fs::write(destination, contents).map_err(|error| AppError::io(destination, error))
}

fn export_lrc(document: &LyricsDocument) -> String {
    let mut output = String::new();
    if document.offset_ms != 0 {
        output.push_str(&format!("[offset:{}]\n", document.offset_ms));
    }
    for line in &document.lines {
        if let Some(start) = line.start_ms {
            output.push_str(&format!("[{}]", lrc_timestamp(start)));
            if line.words.is_empty() {
                output.push_str(&line.text);
            } else {
                for word in &line.words {
                    output.push_str(&format!("<{}>{}", lrc_timestamp(word.start_ms), word.text));
                }
            }
        } else {
            output.push_str(&line.text);
        }
        output.push('\n');
    }
    output
}

fn lrc_timestamp(milliseconds: u64) -> String {
    let minutes = milliseconds / 60_000;
    let seconds = (milliseconds % 60_000) / 1_000;
    let centiseconds = (milliseconds % 1_000) / 10;
    format!("{minutes:02}:{seconds:02}.{centiseconds:02}")
}

pub fn search_lrclib(query: &str) -> Result<Vec<LyricsSearchResult>, AppError> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 200 || query.chars().any(char::is_control) {
        return Err(AppError::InvalidLyricsData(
            "the lyrics search must contain between 1 and 200 characters".into(),
        ));
    }
    let response = lrclib_client()?
        .get(format!("{LRCLIB_BASE_URL}/search"))
        .query(&[("q", query)])
        .send()
        .map_err(lyrics_service_error)?;
    let bytes = checked_response_bytes(response, MAX_SEARCH_RESULTS_BYTES)?;
    let records: Vec<LrclibRecord> = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::LyricsService("LRCLIB returned an invalid response".into()))?;
    if records.len() > 20 {
        return Err(AppError::LyricsService(
            "LRCLIB returned too many search results".into(),
        ));
    }
    records
        .into_iter()
        .map(|record| record_summary(&record))
        .collect()
}

pub fn get_lrclib(id: u64) -> Result<RemoteLyricsRecord, AppError> {
    if id == 0 {
        return Err(AppError::InvalidLyricsData(
            "the LRCLIB record identifier is invalid".into(),
        ));
    }
    let response = lrclib_client()?
        .get(format!("{LRCLIB_BASE_URL}/get/{id}"))
        .send()
        .map_err(lyrics_service_error)?;
    let bytes = checked_response_bytes(response, MAX_LYRICS_BYTES)?;
    let mut record: LrclibRecord = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::LyricsService("LRCLIB returned an invalid response".into()))?;
    let summary = record_summary(&record)?;
    record.synced_lyrics = nonempty(record.synced_lyrics);
    record.plain_lyrics = nonempty(record.plain_lyrics);
    if !record.instrumental && record.synced_lyrics.is_none() && record.plain_lyrics.is_none() {
        return Err(AppError::LyricsService(
            "the LRCLIB result does not contain lyrics".into(),
        ));
    }
    Ok(RemoteLyricsRecord {
        summary,
        synced_lyrics: record.synced_lyrics,
        plain_lyrics: record.plain_lyrics,
    })
}

fn lrclib_client() -> Result<reqwest::blocking::Client, AppError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent(concat!(
            "SonArcan/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/OrangeJuce82/sonarcan)"
        ))
        .build()
        .map_err(lyrics_service_error)
}

fn checked_response_bytes(
    response: reqwest::blocking::Response,
    maximum: u64,
) -> Result<Vec<u8>, AppError> {
    let status = response.status();
    if !status.is_success() {
        let detail = if status.as_u16() == 429 {
            response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .map_or_else(
                    || "LRCLIB rate limit reached; try again later".into(),
                    |seconds| format!("LRCLIB rate limit reached; retry after {seconds} seconds"),
                )
        } else if status.as_u16() == 404 {
            "no lyrics were found for this track".into()
        } else {
            format!("LRCLIB returned HTTP status {}", status.as_u16())
        };
        return Err(AppError::LyricsService(detail));
    }
    if response
        .content_length()
        .is_some_and(|length| length > maximum)
    {
        return Err(AppError::LyricsService(
            "LRCLIB returned an oversized response".into(),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(lyrics_service_error)?;
    if bytes.len() as u64 > maximum {
        return Err(AppError::LyricsService(
            "LRCLIB returned an oversized response".into(),
        ));
    }
    Ok(bytes)
}

fn record_summary(record: &LrclibRecord) -> Result<LyricsSearchResult, AppError> {
    let valid = record.id > 0
        && text_valid(&record.track_name, 512)
        && record.artist_name.chars().count() <= 512
        && record.album_name.chars().count() <= 512
        && record.duration.is_finite()
        && (0.0..=86_400.0).contains(&record.duration);
    if !valid {
        return Err(AppError::LyricsService(
            "LRCLIB returned invalid track metadata".into(),
        ));
    }
    Ok(LyricsSearchResult {
        id: record.id,
        track_name: record.track_name.clone(),
        artist_name: record.artist_name.clone(),
        album_name: record.album_name.clone(),
        duration_seconds: record.duration,
        instrumental: record.instrumental,
        has_synced_lyrics: nonempty_ref(record.synced_lyrics.as_deref()).is_some(),
        has_plain_lyrics: nonempty_ref(record.plain_lyrics.as_deref()).is_some(),
    })
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn nonempty_ref(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

fn lyrics_service_error(error: impl std::fmt::Display) -> AppError {
    AppError::LyricsService(error.to_string())
}

fn lyrics_path(package_path: &Path, track_id: Uuid, create: bool) -> Result<PathBuf, AppError> {
    let canonical_package = package_path
        .canonicalize()
        .map_err(|error| AppError::io(package_path, error))?;
    let directory = canonical_package.join("Lyrics");
    if create {
        fs::create_dir_all(&directory).map_err(|error| AppError::io(&directory, error))?;
    }
    if !directory.exists() {
        return Ok(directory.join(format!("{track_id}.json")));
    }
    let canonical_directory = directory
        .canonicalize()
        .map_err(|error| AppError::io(&directory, error))?;
    if !canonical_directory.starts_with(&canonical_package) {
        return Err(AppError::LyricsOutsideProject(canonical_directory));
    }
    let path = canonical_directory.join(format!("{track_id}.json"));
    if path.exists() {
        let metadata = fs::symlink_metadata(&path).map_err(|error| AppError::io(&path, error))?;
        let canonical_path = path
            .canonicalize()
            .map_err(|error| AppError::io(&path, error))?;
        if metadata.file_type().is_symlink() || !canonical_path.starts_with(&canonical_directory) {
            return Err(AppError::LyricsOutsideProject(path));
        }
    }
    Ok(path)
}

fn validate(document: &LyricsDocument) -> Result<(), AppError> {
    let metadata_valid = document.version == DOCUMENT_VERSION
        && !document.language.trim().is_empty()
        && document.language.chars().count() <= 35
        && !document.language.chars().any(char::is_control)
        && document.offset_ms.unsigned_abs() <= 30_000
        && optional_text_valid(document.provider_track_id.as_deref(), 256)
        && optional_text_valid(document.attribution.as_deref(), 1_024)
        && optional_text_valid(document.copyright.as_deref(), 2_048);
    let word_count = document
        .lines
        .iter()
        .map(|line| line.words.len())
        .sum::<usize>();
    let mut previous_start = None;
    let lines_valid = document.lines.len() <= MAX_LINES
        && word_count <= MAX_WORDS
        && document.lines.iter().all(|line| {
            let start_ordered = match (previous_start, line.start_ms) {
                (Some(previous), Some(current)) => current >= previous,
                _ => true,
            };
            if line.start_ms.is_some() {
                previous_start = line.start_ms;
            }
            let mut previous_word_start = None;
            text_valid(&line.text, MAX_TEXT_CHARS)
                && start_ordered
                && valid_range(line.start_ms, line.end_ms)
                && line.words.iter().all(|word| {
                    let ordered = previous_word_start.map_or(true, |start| word.start_ms >= start);
                    previous_word_start = Some(word.start_ms);
                    text_valid(&word.text, 512)
                        && ordered
                        && word.end_ms.map_or(true, |end| end > word.start_ms)
                        && line.start_ms.map_or(true, |start| word.start_ms >= start)
                        && line.end_ms.map_or(true, |end| word.start_ms < end)
                })
        });
    if !metadata_valid || !lines_valid {
        return Err(AppError::InvalidLyricsData(
            "lyrics contain invalid or unsupported values".into(),
        ));
    }
    Ok(())
}

fn validate_duration(
    document: &LyricsDocument,
    duration_seconds: Option<f64>,
) -> Result<(), AppError> {
    let Some(duration_ms) = duration_seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value * 1_000.0)
    else {
        return Ok(());
    };
    let outside = |value: u64| value as f64 > duration_ms;
    let invalid = document.lines.iter().any(|line| {
        line.start_ms.is_some_and(outside)
            || line.end_ms.is_some_and(outside)
            || line
                .words
                .iter()
                .any(|word| outside(word.start_ms) || word.end_ms.is_some_and(outside))
    });
    if invalid {
        return Err(AppError::InvalidLyricsData(
            "a synchronized timestamp is outside the audio duration".into(),
        ));
    }
    Ok(())
}

fn valid_range(start: Option<u64>, end: Option<u64>) -> bool {
    match (start, end) {
        (Some(start), Some(end)) => end > start,
        (None, Some(_)) => false,
        _ => true,
    }
}

fn text_valid(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum
        && !value.chars().any(|character| character == '\0')
}

fn optional_text_valid(value: Option<&str>, maximum: usize) -> bool {
    value.map_or(true, |value| text_valid(value, maximum))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> LyricsDocument {
        LyricsDocument {
            version: 1,
            provider: LyricsProvider::Local,
            provider_track_id: None,
            language: "fr".into(),
            sync_level: LyricsSyncLevel::Line,
            attribution: None,
            copyright: None,
            offset_ms: 0,
            lines: vec![LyricsLine {
                text: "Une ligne".into(),
                start_ms: Some(1_000),
                end_ms: Some(2_000),
                words: Vec::new(),
            }],
        }
    }

    #[test]
    fn validates_a_bounded_document() {
        assert!(validate(&document()).is_ok());
    }

    #[test]
    fn rejects_reversed_timings() {
        let mut value = document();
        value.lines[0].end_ms = Some(900);
        assert!(validate(&value).is_err());
    }

    #[test]
    fn rejects_timings_outside_the_audio_duration() {
        let mut value = document();
        value.lines[0].start_ms = Some(2_001);
        value.lines[0].end_ms = None;
        assert!(validate_duration(&value, Some(2.0)).is_err());
        assert!(validate_duration(&value, None).is_ok());
    }

    #[test]
    fn rejects_unbounded_offsets_and_unordered_words() {
        let mut value = document();
        value.offset_ms = i32::MIN;
        assert!(validate(&value).is_err());

        value.offset_ms = 0;
        value.sync_level = LyricsSyncLevel::Word;
        value.lines[0].words = vec![
            LyricsWord {
                text: "ligne".into(),
                start_ms: 1_500,
                end_ms: Some(1_700),
            },
            LyricsWord {
                text: "Une ".into(),
                start_ms: 1_000,
                end_ms: Some(1_400),
            },
        ];
        assert!(validate(&value).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_lyrics_document_symlinked_outside_the_project() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let package = temporary.path().join("Project.sac");
        let directory = package.join("Lyrics");
        fs::create_dir_all(&directory).unwrap();
        let outside = temporary.path().join("outside.json");
        fs::write(&outside, b"{}").unwrap();
        let track_id = Uuid::new_v4();
        symlink(&outside, directory.join(format!("{track_id}.json"))).unwrap();

        assert!(matches!(
            lyrics_path(&package, track_id, false),
            Err(AppError::LyricsOutsideProject(_))
        ));
    }

    #[test]
    fn exports_synchronized_lrc_and_simple_markdown() {
        let temporary = tempfile::tempdir().unwrap();
        let mut value = document();
        value.offset_ms = 100;
        let lrc = temporary.path().join("lyrics.lrc");
        export(&lrc, "Test", &value, LyricsExportFormat::Lrc).unwrap();
        assert_eq!(
            fs::read_to_string(lrc).unwrap(),
            "[offset:100]\n[00:01.00]Une ligne\n"
        );

        let markdown = temporary.path().join("lyrics.md");
        export(&markdown, "Test", &value, LyricsExportFormat::Markdown).unwrap();
        assert_eq!(
            fs::read_to_string(markdown).unwrap(),
            "# Test\n\nUne ligne\n"
        );
    }
}
