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
                auto_start: false,
                registered_users: Vec::new(),
                notify_all_users: true,
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
    pub auto_start: bool,
    pub registered_users: Vec<RegisteredUser>,
    pub notify_all_users: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredUser {
    pub chat_id: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub enabled: bool,
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
        Ok(Self {
            name,
            bytes,
            description,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let bytes_str = self.bytes.replace(" ", "");
        if bytes_str.len() % 2 != 0 {
            return Err(anyhow!("Invalid hex string: odd length"));
        }
        let mut bytes = Vec::new();
        for i in (0..bytes_str.len()).step_by(2) {
            let byte = u8::from_str_radix(&bytes_str[i..i + 2], 16)
                .map_err(|_| anyhow!("Invalid hex byte"))?;
            bytes.push(byte);
        }
        Ok(bytes)
    }
}

fn get_config_path() -> PathBuf {
    // Try to get the executable's directory first
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join("config.json");
        }
    }
    // Fallback to user's config directory
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("ew_notify");
    fs::create_dir_all(&path).ok();
    path.push("config.json");
    path
}

pub fn load_config() -> Result<AppConfig> {
    let path = get_config_path();
    if !path.exists() {
        return Err(anyhow!("Config file not found"));
    }
    let content = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = get_config_path();
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn add_registered_user(config: &mut AppConfig, user: RegisteredUser) {
    if !config
        .telegram
        .registered_users
        .iter()
        .any(|u| u.chat_id == user.chat_id)
    {
        config.telegram.registered_users.push(user);
    }
}

pub fn set_user_enabled(config: &mut AppConfig, chat_id: &str, enabled: bool) {
    if let Some(user) = config
        .telegram
        .registered_users
        .iter_mut()
        .find(|u| u.chat_id == chat_id)
    {
        user.enabled = enabled;
    }
}
