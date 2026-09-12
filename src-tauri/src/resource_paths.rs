//! Resolution of bundled native resource paths.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn configure(resource_dir: &Path) {
    let _ = RESOURCE_DIR.set(resource_dir.to_path_buf());
}

pub fn resource_path(relative: impl AsRef<Path>) -> Option<PathBuf> {
    RESOURCE_DIR.get().map(|root| root.join(relative))
}
