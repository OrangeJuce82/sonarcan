use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    pub cpu_percent: Option<f32>,
    pub memory_megabytes: Option<u64>,
}

pub fn snapshot() -> SystemMetrics {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
    {
        return unix_process_snapshot();
    }

    #[allow(unreachable_code)]
    SystemMetrics {
        cpu_percent: None,
        memory_megabytes: None,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
fn unix_process_snapshot() -> SystemMetrics {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "%cpu=", "-o", "rss=", "-p", pid.as_str()])
        .output();
    let Ok(output) = output else {
        return SystemMetrics {
            cpu_percent: None,
            memory_megabytes: None,
        };
    };
    if !output.status.success() {
        return SystemMetrics {
            cpu_percent: None,
            memory_megabytes: None,
        };
    }
    let output_text = String::from_utf8_lossy(&output.stdout);
    let mut values = output_text.split_whitespace();
    let cpu_percent = values.next().and_then(|value| value.parse::<f32>().ok());
    let memory_megabytes = values
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|kilobytes| kilobytes.div_ceil(1024));
    SystemMetrics {
        cpu_percent,
        memory_megabytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_bounded_when_available() {
        let metrics = snapshot();
        if let Some(cpu) = metrics.cpu_percent {
            assert!(cpu.is_finite() && (0.0..=100_000.0).contains(&cpu));
        }
        if let Some(memory) = metrics.memory_megabytes {
            assert!(memory > 0);
        }
    }
}
