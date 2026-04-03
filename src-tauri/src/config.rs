use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub telegram: TelegramConfig,
    pub target: TargetConfig,
    pub notifications: NotificationsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            telegram: TelegramConfig {
                bot_token: String::new(),
                chat_id: String::new(),
            },
            target: TargetConfig {
                process_name: "EndlessWar.exe".to_string(),
                patterns: vec![
                    Pattern {
                        name: "event_started".to_string(),
                        bytes: "A1B2C3D4".to_string(),
                        description: "Событие началось".to_string(),
                    },
                    Pattern {
                        name: "tournament".to_string(),
                        bytes: "FF000100".to_string(),
                        description: "Тнир".to_string(),
                    },
                ],
            },
            notifications: NotificationsConfig {
                telegram_enabled: true,
                tray_enabled: true,
                cooldown_seconds: 60,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub process_name: String,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub telegram_enabled: bool,
    pub tray_enabled: bool,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub bytes: String,
    pub description: String,
}

impl Pattern {
    pub fn new(name: String, bytes: String, description: String) -> Result<Self> {
        if name.is_empty() {
            return Err(anyhow!("Pattern name cannot be empty"));
        }
        if bytes.is_empty() {
            return Err(anyhow!("Pattern bytes cannot be empty"));
        }
        Self::parse_hex(&bytes)?;
        Ok(Self {
            name,
            bytes,
            description,
        })
    }

    pub fn parse_hex(hex: &str) -> Result<Vec<u8>> {
        let hex_clean = hex.replace(' ', "");
        if hex_clean.len() % 2 != 0 {
            return Err(anyhow!("Invalid hex string: odd length"));
        }
        let mut bytes = Vec::new();
        for i in (0..hex_clean.len()).step_by(2) {
            let byte_str = &hex_clean[i..i + 2];
            let byte = u8::from_str_radix(byte_str, 16)
                .map_err(|_| anyhow!("Invalid hex byte: {}", byte_str))?;
            bytes.push(byte);
        }
        Ok(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Self::parse_hex(&self.bytes)
    }
}

fn get_config_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.set_file_name("config.json");
    if !path.exists() {
        path = PathBuf::from("config.json");
    }
    path
}

pub fn load_config() -> Result<AppConfig> {
    let path = get_config_path();
    let content = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = get_config_path();
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}
