mod audio;
mod error;
mod project;
mod waveform;

use std::path::PathBuf;

use project::ProjectSummary;
use serde::Serialize;
use tracing::info;
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

#[tauri::command]
fn create_project(name: String, parent_directory: PathBuf) -> Result<ProjectSummary, AppError> {
    info!(project_name = name, parent = %parent_directory.display(), "creating project");
    project::create_project(&parent_directory, &name)
}

#[tauri::command]
fn open_project(package_path: PathBuf) -> Result<ProjectSummary, AppError> {
    info!(path = %package_path.display(), "opening project");
    project::open_project(&package_path)
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
fn save_project_as(
    source_package: PathBuf,
    parent_directory: PathBuf,
    name: String,
) -> Result<ProjectSummary, AppError> {
    info!(source = %source_package.display(), parent = %parent_directory.display(), new_name = name, "saving project as");
    project::save_as(&source_package, &parent_directory, &name)
}

#[tauri::command]
async fn get_waveform(
    package_path: PathBuf,
    track_id: uuid::Uuid,
) -> Result<waveform::WaveformData, AppError> {
    info!(project = %package_path.display(), %track_id, "loading waveform");
    tauri::async_runtime::spawn_blocking(move || {
        waveform::load_or_generate(&package_path, track_id)
    })
    .await
    .map_err(|error| AppError::BackgroundTask(error.to_string()))?
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

pub fn run() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sonarcan=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
    info!(version = env!("CARGO_PKG_VERSION"), "starting SonArcan");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_project,
            open_project,
            import_audio,
            rename_project,
            rename_track,
            save_project_as,
            get_waveform,
            diagnostics_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("SonArcan runtime failed");
}
