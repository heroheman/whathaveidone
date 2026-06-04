use std::error::Error;
use crate::config;
use crate::models::{LlmConfig, LlmProvider};

/// Dispatches a summary request to the configured AI backend. Returns the
/// summary text; API errors are returned as the text string (never `Err`) so
/// the UI can display them in the popup like any other result.
pub async fn fetch_commit_summary(prompt: &str, lang: &str, cfg: &LlmConfig) -> Result<String, Box<dyn std::error::Error>> {
    match cfg.provider {
        LlmProvider::Gemini => fetch_gemini_commit_summary(prompt, lang, &cfg.model).await,
        LlmProvider::Custom => fetch_custom_commit_summary(prompt, cfg).await,
    }
}

/// Sends the commit list and a summary prompt to Gemini using the specified model, returns the summary text.
pub async fn fetch_gemini_commit_summary(prompt: &str, _lang: &str, model: &str) -> Result<String, Box<dyn std::error::Error>> {
    let user_message = prompt;
    let response = match gemini_rs::chat(model).send_message(user_message).await {
        Ok(r) => r,
        Err(e) => {
            let msg = if let Some(inner) = e.source() {
                let s = inner.to_string();
                if s.contains("API key must be set") || s.contains("GEMINI_API_KEY") || s.contains("401") {
                    let config_path = config::get_user_config_path().display().to_string();
                    format!(
                        "Gemini API key not found.\n\nPlease add it to your configuration file at:\n{}\n\nOr set it as an environment variable: export GEMINI_API_KEY=your-key",
                        config_path
                    )
                } else {
                    format!("Gemini API error: {}", s)
                }
            } else {
                format!("Gemini API error: {}", e)
            };
            return Ok(msg);
        }
    };
    let text = response.candidates.first()
        .and_then(|c| c.content.parts.first())
        .and_then(|p| p.text.as_ref())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "No summary received.".to_string());
    Ok(text)
}

/// Sends the prompt to any custom OpenAI-compatible `/chat/completions`
/// endpoint (OpenRouter, Vercel AI Gateway, a local server, OpenAI itself). The
/// base URL and key come from `cfg`; errors are returned as the summary text so
/// the UI can show them.
pub async fn fetch_custom_commit_summary(prompt: &str, cfg: &LlmConfig) -> Result<String, Box<dyn std::error::Error>> {
    let config_path = config::get_user_config_path().display().to_string();

    if cfg.base_url.trim().is_empty() {
        return Ok(format!(
            "No custom base URL configured.\n\nSet `custom_base_url` in your configuration file at:\n{}\n\nOr pass it with --base-url, e.g. https://openrouter.ai/api/v1",
            config_path
        ));
    }
    if cfg.model.trim().is_empty() {
        return Ok(format!(
            "No custom model configured.\n\nSet `custom_model` in your configuration file at:\n{}\n\nOr pass it with --model, e.g. openai/gpt-4o-mini",
            config_path
        ));
    }
    if cfg.api_key.trim().is_empty() {
        return Ok(format!(
            "Custom API key not found.\n\nSet `custom_api_key` in your configuration file at:\n{}\n\nOr set it as an environment variable: export CUSTOM_API_KEY=your-key",
            config_path
        ));
    }

    // Trim a trailing slash so we can join "/chat/completions" cleanly.
    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            { "role": "user", "content": prompt }
        ],
    });

    let client = reqwest::Client::new();
    let response = match client
        .post(&endpoint)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Ok(format!("Custom API request failed: {}", e)),
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(t) => t,
        Err(e) => return Ok(format!("Custom API: failed to read response body: {}", e)),
    };

    if !status.is_success() {
        return Ok(format!("Custom API error ({}): {}", status, text.trim()));
    }

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Ok(format!("Custom API: invalid JSON response: {}\n\n{}", e, text.trim())),
    };

    let summary = json["choices"]
        .get(0)
        .and_then(|c| c["message"]["content"].as_str())
        .map(|s| s.trim().to_string());

    match summary {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Ok("No summary received.".to_string()),
    }
}
