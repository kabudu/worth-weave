use std::process::Command;

use crate::error::{LedgerlyError, Result};
use crate::models::AiRecommendation;

fn memory_gib() -> u64 {
    Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|bytes| bytes / 1024 / 1024 / 1024)
        .unwrap_or(0)
}

fn available(command: &str) -> bool {
    Command::new("/usr/bin/which")
        .arg(command)
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn recommendation() -> AiRecommendation {
    let memory = memory_gib();
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        let model = match memory {
            0..=23 => "qwen3.5-4b-4bit",
            24..=47 => "gpt-oss-20b-mxfp4-q8",
            48..=95 => "qwen3.6-35b-8bit",
            _ => "gpt-oss-120b-mxfp4-q8",
        };
        AiRecommendation {
            runtime: "rapid-mlx",
            runtime_name: "Rapid-MLX",
            model: model.into(),
            endpoint: "http://127.0.0.1:8000/v1",
            rationale: format!(
                "Apple Silicon with {memory} GB unified memory; model follows Rapid-MLX's published RAM tier."
            ),
            installed: available("rapid-mlx"),
            supported: true,
        }
    } else {
        AiRecommendation {
            runtime: "ollama",
            runtime_name: "Ollama",
            model: "qwen3.5:4b".into(),
            endpoint: "http://127.0.0.1:11434/v1",
            rationale: "Ollama is the supported cross-platform fallback for this device.".into(),
            installed: available("ollama"),
            supported: true,
        }
    }
}

pub fn install(recommendation: &AiRecommendation) -> Result<()> {
    if recommendation.runtime == "rapid-mlx" {
        if !available("uv") {
            return Err(LedgerlyError::InvalidSettings(
                "Rapid-MLX setup requires the uv package manager".into(),
            ));
        }
        let install = Command::new("uv")
            .args(["tool", "install", "rapid-mlx==0.10.7"])
            .status()?;
        if !install.success() {
            return Err(LedgerlyError::InvalidSettings(
                "Rapid-MLX installation failed".into(),
            ));
        }
        let pull = Command::new("uv")
            .args([
                "tool",
                "run",
                "--from",
                "rapid-mlx==0.10.7",
                "rapid-mlx",
                "pull",
                &recommendation.model,
            ])
            .status()?;
        if !pull.success() {
            return Err(LedgerlyError::InvalidSettings(
                "model download failed".into(),
            ));
        }
    } else {
        if !available("ollama") {
            return Err(LedgerlyError::InvalidSettings(
                "install Ollama from its official macOS application first".into(),
            ));
        }
        let pull = Command::new("ollama")
            .args(["pull", &recommendation.model])
            .status()?;
        if !pull.success() {
            return Err(LedgerlyError::InvalidSettings(
                "model download failed".into(),
            ));
        }
    }
    Ok(())
}
