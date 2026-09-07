use std::{process::Command, sync::Mutex};

use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    pub cpu_percent: Option<f32>,
    pub gpu_percent: Option<f32>,
    pub memory_megabytes: Option<u64>,
    pub memory_percent: Option<f32>,
}

pub struct SystemMetricsService(Mutex<System>);

impl Default for SystemMetricsService {
    fn default() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        system.refresh_memory();
        Self(Mutex::new(system))
    }
}

impl SystemMetricsService {
    pub fn snapshot(&self) -> SystemMetrics {
        let Ok(mut system) = self.0.lock() else {
            return unavailable_snapshot();
        };
        system.refresh_cpu_usage();
        system.refresh_memory();
        let cpu_percent = system.global_cpu_usage().clamp(0.0, 100.0);
        let memory_bytes = system.used_memory();
        let total_memory = system.total_memory();
        SystemMetrics {
            cpu_percent: cpu_percent.is_finite().then_some(cpu_percent),
            gpu_percent: gpu_snapshot(),
            memory_megabytes: Some(memory_bytes.div_ceil(1024 * 1024)),
            memory_percent: (total_memory > 0)
                .then_some(memory_bytes as f32 / total_memory as f32 * 100.0)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 100.0)),
        }
    }
}

fn unavailable_snapshot() -> SystemMetrics {
    SystemMetrics {
        cpu_percent: None,
        gpu_percent: None,
        memory_megabytes: None,
        memory_percent: None,
    }
}

#[cfg(target_os = "macos")]
fn gpu_snapshot() -> Option<f32> {
    let output = Command::new("ioreg")
        .args(["-r", "-d", "1", "-w", "0", "-c", "AGXAccelerator"])
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    parse_values_after_key(
        &String::from_utf8_lossy(&output.stdout),
        "\"Device Utilization %\"=",
    )
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn gpu_snapshot() -> Option<f32> {
    match option_env!("SONARCAN_GPU_BACKEND") {
        Some("nvidia") => command_output(
            "nvidia-smi",
            &[
                "--query-gpu=utilization.gpu",
                "--format=csv,noheader,nounits",
            ],
        )
        .and_then(|output| parse_numeric_lines(&output)),
        Some("amd") => command_output("rocm-smi", &["--showuse", "--json"])
            .and_then(|output| parse_values_after_key(&output, "GPU use (%)")),
        _ => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn gpu_snapshot() -> Option<f32> {
    None
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn parse_numeric_lines(output: &str) -> Option<f32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .reduce(f32::max)
        .map(|value| value.clamp(0.0, 100.0))
}

fn parse_values_after_key(output: &str, key: &str) -> Option<f32> {
    output
        .match_indices(key)
        .filter_map(|(index, _)| {
            let suffix = &output[index + key.len()..];
            let number: String = suffix
                .chars()
                .skip_while(|character| !character.is_ascii_digit() && *character != '.')
                .take_while(|character| character.is_ascii_digit() || *character == '.')
                .collect();
            number.parse::<f32>().ok()
        })
        .filter(|value| value.is_finite())
        .reduce(f32::max)
        .map(|value| value.clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_bounded_when_available() {
        let service = SystemMetricsService::default();
        let metrics = service.snapshot();
        for value in [
            metrics.cpu_percent,
            metrics.gpu_percent,
            metrics.memory_percent,
        ]
        .into_iter()
        .flatten()
        {
            assert!(value.is_finite() && (0.0..=100.0).contains(&value));
        }
        if let Some(memory) = metrics.memory_megabytes {
            assert!(memory > 0);
        }
    }

    #[test]
    fn parses_apple_and_amd_gpu_utilization() {
        assert_eq!(
            parse_values_after_key(
                r#""PerformanceStatistics" = {"Device Utilization %"=68}"#,
                "\"Device Utilization %\"=",
            ),
            Some(68.0)
        );
        assert_eq!(
            parse_values_after_key(r#"{"GPU use (%)": "92"}"#, "GPU use (%)"),
            Some(92.0)
        );
    }
}
