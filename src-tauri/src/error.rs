use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("project name cannot be empty")]
    EmptyProjectName,
    #[error("track name cannot be empty")]
    EmptyTrackName,
    #[error("track {0} was not found in the project")]
    TrackNotFound(uuid::Uuid),
    #[error("a project already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("unsupported audio format for {0}")]
    UnsupportedAudioFormat(PathBuf),
    #[error("could not read audio metadata from {path}: {reason}")]
    InvalidAudio { path: PathBuf, reason: String },
    #[error("internal background task failed: {0}")]
    BackgroundTask(String),
    #[error("the selected file does not exist: {0}")]
    MissingSource(PathBuf),
    #[error("the path is not a SonArcan project package: {0}")]
    InvalidProjectPackage(PathBuf),
    #[error(
        "project format version {found} is not supported; this build supports version {supported}"
    )]
    UnsupportedProjectVersion { found: u32, supported: u32 },
    #[error("I/O error while accessing {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid project metadata in {path}: {source}")]
    InvalidProjectMetadata {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not serialize project metadata: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl AppError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
