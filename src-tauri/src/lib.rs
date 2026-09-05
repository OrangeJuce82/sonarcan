mod app_log;
mod audio;
mod audio_engine;
mod audio_fingerprint;
mod chord_analysis;
mod chord_contract;
mod chord_export;
mod error;
mod ffmpeg;
mod importer;
mod loudness;
mod lyrics;
mod native_menu;
mod native_menu_translations;
mod preferences;
mod project;
mod python_runtime;
mod recent;
mod spectrum;
mod stem_contract;
mod stems;
mod system_metrics;
mod waveform;
mod youtube_search;

use std::{
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use project::{PracticeState, ProjectSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::error::AppError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsSnapshot {
    app_version: &'static str,
    os: &'static str,
    architecture: &'static str,
    rust_log: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisCapabilities {
    accelerated: bool,
    backend: Option<&'static str>,
    edition: &'static str,
    reason: Option<&'static str>,
}

#[derive(Default)]
struct AnalysisCapabilityState(AtomicBool);

fn application_edition() -> &'static str {
    option_env!("SONARCAN_EDITION").unwrap_or("full")
}

fn accelerated_analysis_available() -> bool {
    if application_edition() != "full" {
        return false;
    }
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
        || (cfg!(any(target_os = "windows", target_os = "linux"))
            && cfg!(target_arch = "x86_64")
            && matches!(option_env!("SONARCAN_GPU_BACKEND"), Some("nvidia" | "amd")))
}

fn accelerated_analysis_backend() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("MLX / MPS")
    } else {
        match option_env!("SONARCAN_GPU_BACKEND") {
            Some("nvidia") => Some("NVIDIA CUDA"),
            Some("amd") => Some("AMD ROCm"),
            _ => None,
        }
    }
}

fn require_accelerated_analysis(state: &AnalysisCapabilityState) -> Result<(), AppError> {
    state.0.load(Ordering::Acquire).then_some(()).ok_or_else(|| {
        let reason = if application_edition() == "light" {
            "Chord, beat, and separated-track analysis is not included in SonArcan Light."
        } else {
            "Chord, beat, and separated-track analysis is disabled because no qualified GPU backend is available on this platform."
        };
        AppError::BackgroundTask(reason.into())
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupProject {
    project: ProjectSummary,
    unavailable_project_path: Option<PathBuf>,
}

#[derive(Default)]
struct ProjectSession {
    temporary: AtomicBool,
    exit_allowed: AtomicBool,
}

const APPLICATION_EXIT_REQUESTED: &str = "application-exit-requested";
#[cfg(target_os = "macos")]
const PROJECT_OPEN_REQUESTED: &str = "project-open-requested";
static PENDING_OPEN_PROJECT: Mutex<Option<PathBuf>> = Mutex::new(None);
#[cfg(target_os = "macos")]
static PROJECT_RUNTIME_READY: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "macos", test))]
fn queue_open_project(path: PathBuf) {
    *PENDING_OPEN_PROJECT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(path);
}

fn take_open_project() -> Option<PathBuf> {
    PENDING_OPEN_PROJECT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

#[cfg(any(target_os = "macos", test))]
fn is_sonarcan_project_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sac"))
}

#[tauri::command]
fn create_project(app: AppHandle, package_path: PathBuf) -> Result<ProjectSummary, AppError> {
    info!(path = %package_path.display(), "creating project");
    let summary = project::create_project_at(&package_path)?;
    remember_project(&app, &summary.package_path)?;
    Ok(summary)
}

#[tauri::command]
fn create_temporary_project(app: AppHandle) -> Result<ProjectSummary, AppError> {
    let summary = project::create_temporary_project()?;
    remember_project(&app, &summary.package_path)?;
    Ok(summary)
}

#[tauri::command]
fn initialize_project(app: AppHandle) -> Result<StartupProject, AppError> {
    let mut unavailable_project_path = None;
    if let Some(path) = take_open_project() {
        match project::open_project(&path) {
            Ok(project) => {
                remember_project(&app, &project.package_path)?;
                return Ok(StartupProject {
                    project,
                    unavailable_project_path,
                });
            }
            Err(error) => {
                warn!(path = %path.display(), %error, "requested project is unavailable");
                unavailable_project_path = Some(path);
            }
        }
    } else if let Some(path) = recent::latest() {
        match project::open_project(&path) {
            Ok(project) => {
                remember_project(&app, &project.package_path)?;
                return Ok(StartupProject {
                    project,
                    unavailable_project_path,
                });
            }
            Err(error) => {
                warn!(path = %path.display(), %error, "last project is unavailable");
                unavailable_project_path = Some(path.clone());
                if let Err(error) = recent::forget(&path) {
                    warn!(%error, "could not remove unavailable recent project");
                }
            }
        }
    }
    let project = project::create_temporary_project()?;
    remember_project(&app, &project.package_path)?;
    Ok(StartupProject {
        project,
        unavailable_project_path,
    })
}

#[tauri::command]
fn take_open_project_request() -> Option<PathBuf> {
    take_open_project()
}

#[tauri::command]
fn open_project(app: AppHandle, package_path: PathBuf) -> Result<ProjectSummary, AppError> {
    info!(path = %package_path.display(), "opening project");
    let summary = project::open_project(&package_path)?;
    remember_project(&app, &summary.package_path)?;
    Ok(summary)
}

#[tauri::command]
fn verify_project_access(package_path: PathBuf) -> Result<(), AppError> {
    project::verify_project_access(&package_path)
}

#[tauri::command]
fn verify_project_destination_access(destination: PathBuf) -> Result<(), AppError> {
    project::verify_destination_access(&destination)
}

#[tauri::command]
fn import_audio(
    project_path: PathBuf,
    source_paths: Vec<PathBuf>,
) -> Result<ProjectSummary, AppError> {
    info!(project = %project_path.display(), source_count = source_paths.len(), "importing audio");
    project::import_audio(&project_path, &source_paths)
}

#[tauri::command]
fn rename_project(package_path: PathBuf, name: String) -> Result<ProjectSummary, AppError> {
    info!(project = %package_path.display(), new_name = name, "renaming project");
    project::rename_project(&package_path, &name)
}

#[tauri::command]
fn rename_track(
    package_path: PathBuf,
    track_id: uuid::Uuid,
    name: String,
) -> Result<ProjectSummary, AppError> {
    info!(project = %package_path.display(), %track_id, new_name = name, "renaming track");
    project::rename_track(&package_path, track_id, &name)
}

#[tauri::command]
fn reorder_track(
    package_path: PathBuf,
    track_id: uuid::Uuid,
    new_index: usize,
) -> Result<ProjectSummary, AppError> {
    project::reorder_track(&package_path, track_id, new_index)
}

#[tauri::command]
fn delete_track(package_path: PathBuf, track_id: uuid::Uuid) -> Result<ProjectSummary, AppError> {
    project::delete_track(&package_path, track_id)
}

#[tauri::command]
fn get_lyrics(
    package_path: PathBuf,
    track_id: uuid::Uuid,
) -> Result<Option<lyrics::LyricsDocument>, AppError> {
    lyrics::load(&package_path, track_id)
}

#[tauri::command]
fn save_lyrics(
    package_path: PathBuf,
    track_id: uuid::Uuid,
    document: lyrics::LyricsDocument,
) -> Result<lyrics::LyricsDocument, AppError> {
    lyrics::save(&package_path, track_id, document)
}

#[tauri::command]
fn delete_lyrics(package_path: PathBuf, track_id: uuid::Uuid) -> Result<(), AppError> {
    lyrics::remove(&package_path, track_id)
}

#[tauri::command]
async fn search_lrclib_lyrics(query: String) -> Result<Vec<lyrics::LyricsSearchResult>, AppError> {
    tauri::async_runtime::spawn_blocking(move || lyrics::search_lrclib(&query))
        .await
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
async fn get_lrclib_lyrics(id: u64) -> Result<lyrics::RemoteLyricsRecord, AppError> {
    tauri::async_runtime::spawn_blocking(move || lyrics::get_lrclib(id))
        .await
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
fn export_lyrics(
    destination: PathBuf,
    title: String,
    document: lyrics::LyricsDocument,
    format: lyrics::LyricsExportFormat,
) -> Result<(), AppError> {
    lyrics::export(&destination, &title, &document, format)
}

#[tauri::command]
fn export_playlist(
    package_path: PathBuf,
    destination: PathBuf,
    format: String,
) -> Result<(), AppError> {
    info!(project = %package_path.display(), path = %destination.display(), %format, "exporting playlist");
    project::export_playlist(&package_path, &destination, &format)
}

#[tauri::command]
fn export_chords(
    destination: PathBuf,
    title: String,
    duration: f64,
    mode: String,
    segments: Vec<chord_export::ChordExportSegment>,
) -> Result<(), AppError> {
    info!(path = %destination.display(), %mode, count = segments.len(), "exporting chords as JAMS");
    chord_export::export_jams(&destination, &title, duration, &mode, &segments)
}

#[tauri::command]
fn update_practice_state(
    package_path: PathBuf,
    track_id: uuid::Uuid,
    state: PracticeState,
) -> Result<ProjectSummary, AppError> {
    project::update_practice_state(&package_path, track_id, state)
}

#[tauri::command]
fn save_project_as(
    app: AppHandle,
    source_package: PathBuf,
    destination: PathBuf,
) -> Result<ProjectSummary, AppError> {
    info!(source = %source_package.display(), destination = %destination.display(), "saving project as");
    let summary = project::save_as_to(&source_package, &destination)?;
    remember_project(&app, &summary.package_path)?;
    Ok(summary)
}

#[tauri::command]
fn list_recent_projects() -> Vec<PathBuf> {
    recent::list()
}

#[tauri::command]
fn request_application_exit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn confirm_application_exit(app: AppHandle) {
    app.state::<ProjectSession>()
        .exit_allowed
        .store(true, Ordering::Release);
    app.exit(0);
}

#[tauri::command]
fn set_language(app: AppHandle, language: String) -> Result<(), AppError> {
    native_menu::set_language(&app, &language)
        .map_err(|error| AppError::BackgroundTask(error.to_string()))
}

#[tauri::command]
fn get_preferences(
    store: State<'_, preferences::PreferencesStore>,
) -> preferences::UserPreferences {
    store.get()
}

#[tauri::command]
fn save_preferences(
    app: AppHandle,
    store: State<'_, preferences::PreferencesStore>,
    value: preferences::UserPreferences,
) -> Result<preferences::UserPreferences, AppError> {
    let saved = store.save(value)?;
    native_menu::set_language(&app, &saved.language)
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
    let engine = app.state::<audio_engine::AudioEngine>();
    engine.set_volume(saved.master_volume);
    engine.set_music_volume(saved.music_volume);
    engine.set_loudness_normalization(saved.loudness_normalization);
    let metronome_enabled = engine.status().metronome_enabled;
    engine.set_metronome(
        metronome_enabled,
        saved.metronome_volume,
        saved.metronome_sound,
    );
    Ok(saved)
}

#[tauri::command]
fn analyze_import_text(text: String) -> Vec<importer::ImportCandidate> {
    importer::parse_text(&text)
}

#[tauri::command]
fn begin_youtube_searches(service: State<'_, youtube_search::YoutubeSearchService>) -> u64 {
    service.begin()
}

#[tauri::command]
async fn resolve_youtube_search(
    service: State<'_, youtube_search::YoutubeSearchService>,
    query: String,
    generation: u64,
) -> Result<Vec<importer::ImportCandidate>, AppError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.resolve(&query, generation))
        .await
        .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
fn read_import_text_files(paths: Vec<PathBuf>) -> Result<String, AppError> {
    const MAX_FILE_BYTES: u64 = 1024 * 1024;
    const MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024;
    let mut combined = String::new();
    for path in paths.into_iter().take(10) {
        let supported = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                value.eq_ignore_ascii_case("txt") || value.eq_ignore_ascii_case("md")
            });
        if !supported {
            continue;
        }
        let remaining = MAX_TOTAL_BYTES.saturating_sub(combined.len());
        let limit = MAX_FILE_BYTES.min(remaining as u64);
        let file = std::fs::File::open(&path).map_err(|error| AppError::io(&path, error))?;
        let mut value = String::new();
        file.take(limit + 1)
            .read_to_string(&mut value)
            .map_err(|error| AppError::io(&path, error))?;
        if value.len() as u64 > limit {
            return Err(AppError::ImportTextTooLarge(path));
        }
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&value);
    }
    Ok(combined)
}

#[tauri::command]
fn enqueue_imports(
    request: importer::ImportRequest,
    service: State<'_, importer::ImportService>,
    store: State<'_, preferences::PreferencesStore>,
) -> Result<Vec<importer::ImportJob>, AppError> {
    service.enqueue(request, importer::preferences_from_store(&store))
}

#[tauri::command]
fn import_jobs(service: State<'_, importer::ImportService>) -> Vec<importer::ImportJob> {
    service.jobs()
}

#[tauri::command]
fn cancel_import(
    service: State<'_, importer::ImportService>,
    job_id: uuid::Uuid,
) -> Result<(), AppError> {
    service.cancel(job_id)
}

#[tauri::command]
fn remove_import_job(
    service: State<'_, importer::ImportService>,
    job_id: uuid::Uuid,
) -> Result<(), AppError> {
    service.remove(job_id)
}

#[tauri::command]
fn logs_snapshot() -> Vec<app_log::LogEntry> {
    app_log::snapshot()
}

#[tauri::command]
fn push_frontend_log(level: String, message: String) {
    app_log::push_frontend(&level, &message);
}

#[tauri::command]
fn reveal_project(package_path: PathBuf) -> Result<(), AppError> {
    if !package_path.exists() {
        return Err(AppError::MissingSource(package_path));
    }
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg("-R").arg(&package_path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer");
        command.arg(format!("/select,{}", package_path.display()));
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(package_path.parent().unwrap_or(&package_path));
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::BackgroundTask(format!("could not reveal project: {error}")))
}

#[tauri::command]
fn open_external_link(target: String) -> Result<(), AppError> {
    let url = match target.as_str() {
        "github" => "https://github.com/OrangeJuce82/sonarcan",
        "donate" => "https://www.paypal.com/paypalme/z5omes",
        _ => {
            return Err(AppError::BackgroundTask(
                "external link is not allowed".into(),
            ))
        }
    };
    open_url_in_browser(url)
}

#[tauri::command]
fn open_lrclib_search(query: String) -> Result<(), AppError> {
    open_url_in_browser(&lrclib_search_url(&query)?)
}

fn lrclib_search_url(query: &str) -> Result<String, AppError> {
    if query.trim().is_empty() || query.chars().count() > 512 || query.chars().any(char::is_control)
    {
        return Err(AppError::BackgroundTask(
            "the LRCLIB search query is invalid".into(),
        ));
    }
    let mut encoded = String::with_capacity(query.len());
    for byte in query.trim().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    Ok(format!("https://lrclib.net/search/{encoded}"))
}

#[tauri::command]
fn open_youtube_video(video_id: String) -> Result<(), AppError> {
    if video_id.is_empty()
        || video_id.len() > 64
        || !video_id
            .bytes()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_'))
    {
        return Err(AppError::BackgroundTask(
            "YouTube video identifier is invalid".into(),
        ));
    }
    open_url_in_browser(&format!("https://www.youtube.com/watch?v={video_id}"))
}

fn open_url_in_browser(url: &str) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("rundll32");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::BackgroundTask(format!("could not open external link: {error}")))
}

#[tauri::command]
async fn audio_load(
    app: AppHandle,
    package_path: PathBuf,
    track_id: uuid::Uuid,
) -> Result<audio_engine::AudioStatus, AppError> {
    let generation = app.state::<audio_engine::AudioEngine>().begin_load();
    tauri::async_runtime::spawn_blocking(move || {
        let media_path = project::track_media_path(&package_path, track_id)?;
        info!(path = %media_path.display(), %track_id, "loading track into audio engine");
        app.state::<audio_engine::AudioEngine>()
            .load(&media_path, generation)
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
async fn audio_preload(
    app: AppHandle,
    package_path: PathBuf,
    track_id: uuid::Uuid,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let media_path = project::track_media_path(&package_path, track_id)?;
        app.state::<audio_engine::AudioEngine>()
            .preload(&media_path)
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
fn audio_play(engine: State<'_, audio_engine::AudioEngine>) {
    engine.play();
}

#[tauri::command]
fn audio_pause(engine: State<'_, audio_engine::AudioEngine>) {
    engine.pause();
}

#[tauri::command]
fn audio_seek(engine: State<'_, audio_engine::AudioEngine>, seconds: f64) -> bool {
    engine.seek(seconds)
}

#[tauri::command]
fn audio_set_loop(
    engine: State<'_, audio_engine::AudioEngine>,
    a_seconds: Option<f64>,
    b_seconds: Option<f64>,
) {
    engine.set_loop(a_seconds, b_seconds);
}

#[tauri::command]
fn audio_set_volume(engine: State<'_, audio_engine::AudioEngine>, volume: f32) {
    engine.set_volume(volume);
}

#[tauri::command]
fn audio_set_music_volume(engine: State<'_, audio_engine::AudioEngine>, volume: f32) {
    engine.set_music_volume(volume);
}

#[tauri::command]
fn audio_set_loudness_normalization(engine: State<'_, audio_engine::AudioEngine>, enabled: bool) {
    engine.set_loudness_normalization(enabled);
}

#[tauri::command]
fn audio_set_playback_rate(engine: State<'_, audio_engine::AudioEngine>, rate: f64) {
    info!(rate, "setting playback rate");
    engine.set_playback_rate(rate);
}

#[tauri::command]
fn audio_set_pitch(engine: State<'_, audio_engine::AudioEngine>, semitones: f32) {
    info!(semitones, "setting track pitch");
    engine.set_pitch_semitones(semitones);
}

#[tauri::command]
fn audio_set_beat_timeline(
    engine: State<'_, audio_engine::AudioEngine>,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
) -> Result<(), AppError> {
    engine.set_beat_timeline(&beats, &downbeats)
}

#[tauri::command]
fn audio_set_metronome(
    engine: State<'_, audio_engine::AudioEngine>,
    enabled: bool,
    volume: f32,
    sound: audio_engine::MetronomeSound,
) {
    engine.set_metronome(enabled, volume, sound);
}

#[tauri::command]
fn audio_set_loop_trainer(
    engine: State<'_, audio_engine::AudioEngine>,
    settings: audio_engine::LoopTrainerSettings,
) {
    engine.set_loop_trainer(settings);
}

#[tauri::command]
fn audio_set_end_behavior(
    engine: State<'_, audio_engine::AudioEngine>,
    behavior: audio_engine::EndBehavior,
) {
    engine.set_end_behavior(behavior);
}

#[tauri::command]
fn audio_status(engine: State<'_, audio_engine::AudioEngine>) -> audio_engine::AudioStatus {
    engine.status()
}

#[tauri::command]
fn system_metrics() -> system_metrics::SystemMetrics {
    system_metrics::snapshot()
}

#[tauri::command]
fn audio_spectrum(engine: State<'_, audio_engine::AudioEngine>) -> spectrum::SpectrumFrame {
    engine.spectrum()
}

#[tauri::command]
fn stem_start(
    app: AppHandle,
    package_path: PathBuf,
    track_id: uuid::Uuid,
    capability: State<'_, AnalysisCapabilityState>,
) -> Result<(), AppError> {
    require_accelerated_analysis(&capability)?;
    app.state::<stems::StemService>()
        .start(app.clone(), package_path, track_id)
}

#[tauri::command]
fn stem_status(service: State<'_, stems::StemService>) -> stems::StemStatus {
    service.status()
}

#[tauri::command]
fn stem_disable(
    engine: State<'_, audio_engine::AudioEngine>,
    service: State<'_, stems::StemService>,
) {
    service.disable(&engine);
}

#[tauri::command]
fn stem_set_enabled(
    engine: State<'_, audio_engine::AudioEngine>,
    service: State<'_, stems::StemService>,
    enabled: bool,
) -> bool {
    service.set_enabled(&engine, enabled)
}

#[tauri::command]
fn stem_set_mix(
    engine: State<'_, audio_engine::AudioEngine>,
    index: usize,
    gain: f32,
    pan: f32,
    muted: bool,
    soloed: bool,
) {
    engine.set_stem_mix(index, gain, pan, muted, soloed);
}

fn remember_project(app: &AppHandle, path: &std::path::Path) -> Result<(), AppError> {
    recent::remember(path)?;
    let session = app.state::<ProjectSession>();
    session
        .temporary
        .store(project::is_temporary_project_path(path), Ordering::Release);
    session.exit_allowed.store(false, Ordering::Release);
    native_menu::install(app).map_err(|error| AppError::BackgroundTask(error.to_string()))?;
    Ok(())
}

#[tauri::command]
async fn stem_export(
    package_path: PathBuf,
    track_id: uuid::Uuid,
    destination: PathBuf,
    format: stems::StemExportFormat,
    display_names: Vec<String>,
    preferences: State<'_, preferences::PreferencesStore>,
) -> Result<(), AppError> {
    let preferences = preferences.get();
    tauri::async_runtime::spawn_blocking(move || {
        stems::export(
            &package_path,
            track_id,
            &destination,
            format,
            &display_names,
            &preferences,
        )
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
async fn get_waveform(
    app: AppHandle,
    package_path: PathBuf,
    track_id: uuid::Uuid,
) -> Result<waveform::WaveformData, AppError> {
    info!(project = %package_path.display(), %track_id, "loading waveform");
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(cached) = waveform::load_cached(&package_path, track_id) {
            return Ok(cached);
        }
        let media_path = project::track_media_path(&package_path, track_id)?;
        let decoded = app
            .state::<audio_engine::AudioEngine>()
            .decoded_for_analysis(&media_path)?;
        waveform::generate_and_store_from_decoded(&package_path, track_id, &decoded)
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
async fn analyze_chords(
    app: AppHandle,
    package_path: PathBuf,
    track_id: uuid::Uuid,
    capability: State<'_, AnalysisCapabilityState>,
) -> Result<chord_contract::ChordAnalysis, AppError> {
    require_accelerated_analysis(&capability)?;
    info!(project = %package_path.display(), %track_id, "analyzing timed chords");
    let generation = app.state::<chord_analysis::ChordAnalysisService>().begin();
    tauri::async_runtime::spawn_blocking(move || {
        let media_path = project::track_media_path(&package_path, track_id)?;
        app.state::<chord_analysis::ChordAnalysisService>().analyze(
            &app,
            &package_path,
            track_id,
            &media_path,
            generation,
        )
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
}

#[tauri::command]
fn cancel_chord_analysis(service: State<'_, chord_analysis::ChordAnalysisService>) {
    service.cancel();
}

#[tauri::command]
fn diagnostics_snapshot() -> DiagnosticsSnapshot {
    DiagnosticsSnapshot {
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        rust_log: std::env::var("RUST_LOG").unwrap_or_else(|_| "sonarcan=info".to_owned()),
    }
}

#[tauri::command]
async fn analysis_capabilities(
    app: AppHandle,
    state: State<'_, AnalysisCapabilityState>,
) -> Result<AnalysisCapabilities, AppError> {
    // A platform is enabled only after both release qualification and a
    // bounded on-device inference test at application startup.
    let accelerated = if accelerated_analysis_available() {
        tauri::async_runtime::spawn_blocking(move || {
            chord_analysis::accelerator_self_test(&app) && stems::accelerator_self_test(&app)
        })
        .await
        .unwrap_or(false)
    } else {
        false
    };
    state.0.store(accelerated, Ordering::Release);
    Ok(AnalysisCapabilities {
        accelerated,
        backend: accelerated.then(accelerated_analysis_backend).flatten(),
        edition: application_edition(),
        reason: (!accelerated).then_some(if application_edition() == "light" {
            "editionLight"
        } else {
            "acceleratorUnavailable"
        }),
    })
}

pub fn run() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sonarcan=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(false)
        .with_writer(app_log::make_writer)
        .init();
    info!(version = env!("CARGO_PKG_VERSION"), "starting SonArcan");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(native_menu::build)
        .on_menu_event(|app, event| native_menu::handle_event(app, event.id().as_ref()))
        .setup(|app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                ffmpeg::configure_bundled(&resource_dir);
                python_runtime::configure(&resource_dir);
            }
            app.manage(audio_engine::AudioEngine::new()?);
            app.manage(stems::StemService::default());
            app.manage(chord_analysis::ChordAnalysisService::default());
            app.manage(AnalysisCapabilityState::default());
            app.manage(preferences::PreferencesStore::load());
            app.manage(importer::ImportService::default());
            app.manage(youtube_search::YoutubeSearchService::default());
            app.manage(ProjectSession::default());
            #[cfg(target_os = "macos")]
            PROJECT_RUNTIME_READY.store(true, Ordering::Release);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_project,
            create_temporary_project,
            initialize_project,
            take_open_project_request,
            open_project,
            verify_project_access,
            verify_project_destination_access,
            import_audio,
            rename_project,
            rename_track,
            reorder_track,
            delete_track,
            get_lyrics,
            save_lyrics,
            delete_lyrics,
            search_lrclib_lyrics,
            get_lrclib_lyrics,
            export_lyrics,
            export_playlist,
            export_chords,
            update_practice_state,
            save_project_as,
            get_waveform,
            analyze_chords,
            cancel_chord_analysis,
            list_recent_projects,
            request_application_exit,
            confirm_application_exit,
            set_language,
            get_preferences,
            save_preferences,
            analyze_import_text,
            begin_youtube_searches,
            resolve_youtube_search,
            read_import_text_files,
            enqueue_imports,
            import_jobs,
            cancel_import,
            remove_import_job,
            logs_snapshot,
            push_frontend_log,
            reveal_project,
            open_external_link,
            open_lrclib_search,
            open_youtube_video,
            audio_load,
            audio_preload,
            audio_play,
            audio_pause,
            audio_seek,
            audio_set_loop,
            audio_set_volume,
            audio_set_music_volume,
            audio_set_loudness_normalization,
            audio_set_playback_rate,
            audio_set_pitch,
            audio_set_beat_timeline,
            audio_set_metronome,
            audio_set_loop_trainer,
            audio_set_end_behavior,
            audio_status,
            analysis_capabilities,
            system_metrics,
            audio_spectrum,
            stem_start,
            stem_status,
            stem_disable,
            stem_set_enabled,
            stem_set_mix,
            stem_export,
            diagnostics_snapshot
        ])
        .build(tauri::generate_context!())
        .expect("SonArcan runtime initialization failed");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            let session = app.state::<ProjectSession>();
            if session.temporary.load(Ordering::Acquire)
                && !session.exit_allowed.load(Ordering::Acquire)
            {
                api.prevent_exit();
                let _ = app.emit(APPLICATION_EXIT_REQUESTED, ());
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            if let Some(path) = urls
                .into_iter()
                .filter_map(|url| url.to_file_path().ok())
                .find(|path| is_sonarcan_project_path(path))
            {
                info!(path = %path.display(), "received macOS project open request");
                queue_open_project(path);
                // AppKit can deliver a cold-start document before Tauri runs setup.
                // The frontend consumes the queued path once setup has completed.
                if PROJECT_RUNTIME_READY.load(Ordering::Acquire) {
                    let _ = app.emit(PROJECT_OPEN_REQUESTED, ());
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_analysis_requires_a_successful_startup_probe() {
        let capability = AnalysisCapabilityState::default();
        assert!(require_accelerated_analysis(&capability).is_err());

        capability.0.store(true, Ordering::Release);
        assert!(require_accelerated_analysis(&capability).is_ok());
    }

    #[test]
    fn build_edition_has_a_consistent_analysis_contract() {
        assert!(matches!(application_edition(), "full" | "light"));
        if application_edition() == "light" {
            assert!(!accelerated_analysis_available());
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert!(accelerated_analysis_available());
        }
    }

    #[test]
    fn bounds_import_text_files_before_loading_them_into_ipc() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("oversized.txt");
        std::fs::write(&path, vec![b'a'; 1024 * 1024 + 1]).unwrap();

        assert!(matches!(
            read_import_text_files(vec![path.clone()]),
            Err(AppError::ImportTextTooLarge(rejected)) if rejected == path
        ));
    }

    #[test]
    fn recognizes_sonarcan_project_extensions_case_insensitively() {
        assert!(is_sonarcan_project_path(std::path::Path::new("Band.sac")));
        assert!(is_sonarcan_project_path(std::path::Path::new("Band.SAC")));
        assert!(!is_sonarcan_project_path(std::path::Path::new("Band.zip")));
    }

    #[test]
    fn queued_project_open_request_is_consumed_once() {
        queue_open_project(PathBuf::from("Band.sac"));
        assert_eq!(take_open_project(), Some(PathBuf::from("Band.sac")));
        assert_eq!(take_open_project(), None);
    }

    #[test]
    fn lrclib_search_rejects_untrusted_or_empty_queries() {
        assert!(lrclib_search_url("").is_err());
        assert!(lrclib_search_url("line\nbreak").is_err());
        assert_eq!(
            lrclib_search_url("Lou Reed").unwrap(),
            "https://lrclib.net/search/Lou%20Reed"
        );
    }
}
