//! Resolution of the shared bundled Python 3.13 runtime.

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

pub fn bundled_python_313() -> Option<PathBuf> {
    let resources = RESOURCE_DIR.get()?;
    [
        resources.join("python-runtime/runtime/bin/python3.13"),
        resources.join("python-runtime/runtime/bin/python3"),
        resources.join("python-runtime/runtime/python.exe"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}
