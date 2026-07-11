use std::process::{Command, Stdio};
use std::time::Duration;

use crate::error::{LedgerlyError, Result};
use crate::models::{AiRecommendation, PortfolioExplanation};

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
        Command::new("uv")
            .args([
                "tool",
                "run",
                "--from",
                "rapid-mlx==0.10.7",
                "rapid-mlx",
                "serve",
                &recommendation.model,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
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

#[derive(serde::Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
}

#[derive(serde::Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatAnswer,
}

#[derive(serde::Deserialize)]
struct ChatAnswer {
    content: String,
}

pub async fn explain(
    endpoint: &str,
    model: &str,
    question: &str,
    analytics: &str,
) -> Result<PortfolioExplanation> {
    let question = question.trim();
    if question.is_empty() || question.chars().count() > 500 {
        return Err(LedgerlyError::LocalAi(
            "question must contain between 1 and 500 characters".into(),
        ));
    }
    if !(endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:")) {
        return Err(LedgerlyError::LocalAi(
            "only loopback local-AI endpoints are allowed".into(),
        ));
    }
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let system = "You explain a private investment portfolio using only the deterministic JSON analytics supplied by Worthweave. Never recalculate, invent missing values, predict prices, or give personalised financial advice. Clearly state unavailable or stale data. Be concise and cite the relevant values from the context.";
    let user = format!("Question: {question}\n\nDeterministic analytics JSON:\n{analytics}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| LedgerlyError::LocalAi(error.to_string()))?;
    let response = client
        .post(url)
        .json(&ChatRequest {
            model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system,
                },
                ChatMessage {
                    role: "user",
                    content: &user,
                },
            ],
            temperature: 0.1,
        })
        .send()
        .await
        .map_err(|error| LedgerlyError::LocalAi(format!("runtime is unavailable: {error}")))?;
    if !response.status().is_success() {
        return Err(LedgerlyError::LocalAi(format!(
            "runtime returned HTTP {}",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > 1_048_576)
    {
        return Err(LedgerlyError::LocalAi(
            "runtime response is too large".into(),
        ));
    }
    let response: ChatResponse = response
        .json()
        .await
        .map_err(|error| LedgerlyError::LocalAi(format!("invalid runtime response: {error}")))?;
    let answer = response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content.trim().to_owned())
        .filter(|answer| !answer.is_empty())
        .ok_or_else(|| LedgerlyError::LocalAi("runtime returned no explanation".into()))?;
    Ok(PortfolioExplanation {
        answer,
        model: model.into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanations_reject_non_loopback_endpoints() {
        let result = tauri::async_runtime::block_on(explain(
            "https://example.com/v1",
            "test-model",
            "Summarise my portfolio",
            "{}",
        ));
        assert!(result.is_err());
        assert!(
            result
                .expect_err("remote endpoint must fail")
                .to_string()
                .contains("loopback")
        );
    }
}
