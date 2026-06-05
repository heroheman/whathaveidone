use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[allow(unused)]
pub struct Settings {
    pub gemini_model: String,
    pub gemini_api_key: Option<String>,
    pub prompt_for_api_key: bool,
    pub custom_prompt_path: Option<String>,
    pub lang: Option<String>,
    /// Which AI backend to use: "gemini" (default) or "custom".
    pub provider: Option<String>,
    /// Base URL of a custom OpenAI-compatible endpoint (e.g. OpenRouter, Vercel).
    pub custom_base_url: Option<String>,
    /// Model name for the custom OpenAI-compatible provider.
    pub custom_model: Option<String>,
    /// API key for the custom provider (falls back to CUSTOM_API_KEY).
    pub custom_api_key: Option<String>,
    /// Per-provider key stores. The setup wizard saves the key for each gateway
    /// separately so switching providers can recall the right one; the active
    /// provider's key is mirrored into `custom_api_key` for the runtime path.
    pub openrouter_api_key: Option<String>,
    pub vercel_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    /// How many of the most recent AI generations to keep in the history store.
    /// Older ones are pruned when a new summary is saved (TUI and direct mode).
    pub recent_generations: Option<usize>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let user_config_path = get_user_config_path();

        // Ensure the user config directory exists
        if let Some(parent) = user_config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| ConfigError::Message(format!(
                    "Could not create config directory {}: {e}", parent.display()
                )))?;
            }
        }

        // Read the blueprint (compiled-in asset; a parse failure is a build bug).
        let blueprint_content = include_str!("../whid.toml");
        let blueprint_table: toml::Table = blueprint_content.parse()
            .expect("embedded blueprint whid.toml is not valid TOML");

        // Read the existing user config. If the file is present but invalid,
        // surface an error instead of silently discarding the user's settings.
        let mut user_table: toml::Table = match fs::read_to_string(&user_config_path) {
            Ok(content) => content.parse().map_err(|e| ConfigError::Message(format!(
                "User config at {} is not valid TOML: {e}. Fix or remove the file.",
                user_config_path.display()
            )))?,
            Err(_) => toml::Table::new(),
        };

        let mut config_was_updated = false;
        // Iterate over the blueprint and add missing keys to the user config
        for (key, value) in blueprint_table.iter() {
            if !user_table.contains_key(key) {
                user_table.insert(key.clone(), value.clone());
                config_was_updated = true;
            }
        }

        // If the user config was modified, write it back to the file
        if config_was_updated || !user_config_path.exists() {
            fs::write(&user_config_path, user_table.to_string()).map_err(|e| ConfigError::Message(format!(
                "Could not write user config {}: {e}", user_config_path.display()
            )))?;
        }


        let s = Config::builder()
            // 1. Load project defaults from whid.toml (blueprint). Required.
            // This still acts as the base for deserialization structure.
            .add_source(config::File::from_str(blueprint_content, config::FileFormat::Toml))
            // 2. Merge user's global config.
            .add_source(File::from(user_config_path).required(true))
            // 3. Merge local whid.toml from CWD. Optional override.
            .add_source(File::with_name("whid.toml").required(false))
            .build()?;

        s.try_deserialize()
    }
}

pub fn get_user_config_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Failed to get home directory");
    path.push(".config");
    path.push("whid");
    path.push("whid.toml");
    path
}

pub fn save_api_key(api_key: &str) -> Result<(), anyhow::Error> {
    let user_config_path = get_user_config_path();

    let config_str = fs::read_to_string(&user_config_path).unwrap_or_else(|_| "".to_string());
    let mut doc = config_str.parse::<toml::Table>()?;

    doc.insert("gemini_api_key".to_string(), toml::Value::String(api_key.to_string()));

    fs::write(&user_config_path, doc.to_string())?;

    Ok(())
}

/// Write several keys into the user config in one pass, creating the file if it
/// does not exist yet and preserving any keys the caller does not touch. Used by
/// the first-run setup wizard (`onboarding`) to persist the chosen provider,
/// API key and language together.
pub fn save_config_values(values: &[(&str, toml::Value)]) -> Result<(), anyhow::Error> {
    let user_config_path = get_user_config_path();

    if let Some(parent) = user_config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let config_str = fs::read_to_string(&user_config_path).unwrap_or_default();
    let mut doc = config_str.parse::<toml::Table>().unwrap_or_default();

    for (key, value) in values {
        doc.insert((*key).to_string(), value.clone());
    }

    fs::write(&user_config_path, doc.to_string())?;
    Ok(())
}

pub fn disable_api_key_prompt() -> Result<(), anyhow::Error> {
    let user_config_path = get_user_config_path();

    let config_str = fs::read_to_string(&user_config_path).unwrap_or_else(|_| "".to_string());
    let mut doc = config_str.parse::<toml::Table>()?;

    doc.insert("prompt_for_api_key".to_string(), toml::Value::Boolean(false));

    fs::write(&user_config_path, doc.to_string())?;

    Ok(())
} 