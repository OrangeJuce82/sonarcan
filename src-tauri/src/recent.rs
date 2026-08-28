use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;

use crate::error::AppError;

const MAX_RECENT_PROJECTS: usize = 10;
const MAX_RECENT_FILE_BYTES: u64 = 64 * 1024;

pub fn list() -> Vec<PathBuf> {
    read_all()
        .into_iter()
        .filter(|path| path.is_dir())
        .take(MAX_RECENT_PROJECTS)
        .collect()
}

pub fn latest() -> Option<PathBuf> {
    read_all().into_iter().next()
}

pub fn forget(path: &Path) -> Result<(), AppError> {
    let Some(storage_path) = storage_path() else {
        return Ok(());
    };
    let mut recent = read_all();
    recent.retain(|entry| entry != path);
    write(&storage_path, &recent)
}

fn read_all() -> Vec<PathBuf> {
    let Some(path) = storage_path() else {
        return Vec::new();
    };
    let Ok(file) = File::open(&path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_RECENT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_RECENT_FILE_BYTES
    {
        return Vec::new();
    }
    serde_json::from_slice::<Vec<PathBuf>>(&bytes)
        .unwrap_or_default()
        .into_iter()
        .take(MAX_RECENT_PROJECTS)
        .collect()
}

pub fn remember(path: &Path) -> Result<(), AppError> {
    let Some(storage_path) = storage_path() else {
        return Ok(());
    };
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut recent = list();
    recent.retain(|entry| entry != &canonical);
    recent.insert(0, canonical);
    recent.truncate(MAX_RECENT_PROJECTS);
    write(&storage_path, &recent)
}

fn write(storage_path: &Path, recent: &[PathBuf]) -> Result<(), AppError> {
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    let temporary = storage_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let contents = serde_json::to_vec_pretty(recent)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| AppError::io(&temporary, error))?;
        file.write_all(&contents)
            .map_err(|error| AppError::io(&temporary, error))?;
        file.sync_data()
            .map_err(|error| AppError::io(&temporary, error))?;
        fs::rename(&temporary, storage_path).map_err(|error| AppError::io(storage_path, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn storage_path() -> Option<PathBuf> {
    ProjectDirs::from("music", "SonArcan", "SonArcan")
        .map(|directories| directories.config_dir().join("recent-projects.json"))
}
