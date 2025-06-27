use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use toml;

#[derive(Debug, Deserialize, Serialize)]
#[allow(unused)]
pub struct Settings {
    pub gemini_model: String,
    pub gemini_api_key: Option<String>,
    pub prompt_for_api_key: bool,
    pub custom_prompt_path: Option<String>,
    pub lang: Option<String>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let blueprint_path = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(manifest_dir) => {
                let mut path = PathBuf::from(manifest_dir);
                path.push("whid.toml");
                path
            }
            Err(_) => {
                // Fallback for release builds or when not using Cargo.
                // Assumes whid.toml is in the current working directory.
                PathBuf::from("whid.toml")
            }
        };

        let user_config_path = get_user_config_path();

        // If the user config doesn't exist, create it from the blueprint `whid.toml`
        if !user_config_path.exists() {
            // Use the determined blueprint path to read the content
            if let Ok(blueprint_content) = fs::read_to_string(&blueprint_path) {
                if let Some(parent) = user_config_path.parent() {
                    fs::create_dir_all(parent).expect("Could not create config directory");
                }
                fs::write(&user_config_path, blueprint_content)
                    .expect("Could not write user config file from blueprint");
            }
            // If whid.toml doesn't exist at blueprint_path, builder will fail. This is intended.
        }

        let s = Config::builder()
            // 1. Load project defaults from whid.toml (blueprint). Required.
            .add_source(File::from(blueprint_path).required(true))
            // 2. Merge user's global config. Required as we just created it if it was missing.
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

pub fn disable_api_key_prompt() -> Result<(), anyhow::Error> {
    let user_config_path = get_user_config_path();

    let config_str = fs::read_to_string(&user_config_path).unwrap_or_else(|_| "".to_string());
    let mut doc = config_str.parse::<toml::Table>()?;

    doc.insert("prompt_for_api_key".to_string(), toml::Value::Boolean(false));

    fs::write(&user_config_path, doc.to_string())?;

    Ok(())
} 