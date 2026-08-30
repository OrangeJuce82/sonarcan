use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("project name cannot be empty")]
    EmptyProjectName,
    #[error("track name cannot be empty")]
    EmptyTrackName,
    #[error("track {0} was not found in the project")]
    TrackNotFound(uuid::Uuid),
    #[error("track media is outside the project audio directory: {0}")]
    TrackMediaOutsideProject(PathBuf),
    #[error("analysis cache is outside the project package: {0}")]
    AnalysisCacheOutsideProject(PathBuf),
    #[error("invalid practice state: {0}")]
    InvalidPracticeState(String),
    #[error("a project already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("the selected project destination is invalid: {0}")]
    InvalidProjectDestination(PathBuf),
    #[error("unsupported audio format for {0}")]
    UnsupportedAudioFormat(PathBuf),
    #[error("could not read audio metadata from {path}: {reason}")]
    InvalidAudio { path: PathBuf, reason: String },
    #[error("audio fingerprinting failed for {path}: {reason}")]
    AudioFingerprint { path: PathBuf, reason: String },
    #[error("this audio is already in the project as \"{existing_title}\": {incoming}")]
    DuplicateAudio {
        incoming: PathBuf,
        existing_title: String,
    },
    #[error("internal background task failed: {0}")]
    BackgroundTask(String),
    #[error("audio engine error: {0}")]
    AudioEngine(String),
    #[error("stem separation error: {0}")]
    StemSeparation(String),
    #[error("chord analysis error: {0}")]
    ChordAnalysis(String),
    #[error("the selected file does not exist: {0}")]
    MissingSource(PathBuf),
    #[error("the path is not a SonArcan project package: {0}")]
    InvalidProjectPackage(PathBuf),
    #[error("project manifest exceeds the 8 MiB safety limit: {0}")]
    ProjectManifestTooLarge(PathBuf),
    #[error("import text exceeds the 1 MiB per-file or 2 MiB total safety limit: {0}")]
    ImportTextTooLarge(PathBuf),
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
