use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

use crate::preferences::Mp3Quality;

static BUNDLED_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn configure_bundled(resource_dir: &Path) {
    let _ = BUNDLED_RESOURCE_DIR.set(resource_dir.join("audio-tools/bin"));
}

pub fn find() -> Option<PathBuf> {
    let mut candidates = BUNDLED_RESOURCE_DIR
        .get()
        .map(|directory| {
            vec![directory.join(if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            })]
        })
        .unwrap_or_default();
    if cfg!(debug_assertions) {
        if let Some(path) = executable_on_path(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        }) {
            candidates.push(path);
        }
        #[cfg(target_os = "macos")]
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin/ffmpeg"),
            PathBuf::from("/usr/local/bin/ffmpeg"),
        ]);
    }
    candidates.into_iter().find(|candidate| {
        if candidate.is_absolute() && !candidate.is_file() {
            return false;
        }
        Command::new(candidate)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

fn executable_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

pub fn apply_mp3_quality(command: &mut Command, quality: Mp3Quality) {
    match quality {
        Mp3Quality::VbrHigh => {
            command.args(["-q:a", "0"]);
        }
        Mp3Quality::Kbps320 => {
            command.args(["-b:a", "320k"]);
        }
        Mp3Quality::Kbps256 => {
            command.args(["-b:a", "256k"]);
        }
        Mp3Quality::Kbps192 => {
            command.args(["-b:a", "192k"]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_quality_maps_to_ffmpeg_arguments() {
        let mut command = Command::new("ffmpeg");
        apply_mp3_quality(&mut command, Mp3Quality::Kbps256);
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(arguments, ["-b:a", "256k"]);
    }
}
