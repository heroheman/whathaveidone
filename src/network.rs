use std::error::Error;
use crate::config;
// use reqwest;
// use serde_json;

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
    let text = response.candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .and_then(|p| p.text.as_ref())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "No summary received.".to_string());
    Ok(text)
}