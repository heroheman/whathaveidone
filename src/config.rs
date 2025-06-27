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
        let user_config_path = get_user_config_path();

        // Ensure the user config directory exists
        if let Some(parent) = user_config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).expect("Could not create config directory");
            }
        }

        // Read the blueprint
        let blueprint_content = include_str!("../whid.toml");
        let blueprint_table: toml::Table = blueprint_content.parse()
            .expect("Could not parse blueprint config as TOML");

        // Read the user config, or create an empty table if it doesn't exist
        let user_config_content = fs::read_to_string(&user_config_path).unwrap_or_default();
        let mut user_table: toml::Table = user_config_content.parse()
            .unwrap_or_else(|_| toml::Table::new());

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
            fs::write(&user_config_path, user_table.to_string())
                .expect("Could not write updated user config file");
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

pub fn disable_api_key_prompt() -> Result<(), anyhow::Error> {
    let user_config_path = get_user_config_path();

    let config_str = fs::read_to_string(&user_config_path).unwrap_or_else(|_| "".to_string());
    let mut doc = config_str.parse::<toml::Table>()?;

    doc.insert("prompt_for_api_key".to_string(), toml::Value::Boolean(false));

    fs::write(&user_config_path, doc.to_string())?;

    Ok(())
} 