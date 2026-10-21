use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub telegram: TelegramConfig,
    pub target: TargetConfig,
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub startup: StartupConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            telegram: TelegramConfig {
                bot_token: String::new(),
                admin_chat_id: String::new(),
                auto_start: false,
                registered_users: Vec::new(),
                notify_all_users: true,
            },
            target: TargetConfig {
                process_name: "EndlessWar.exe".to_string(),
                capture_mode: CaptureMode::default(),
                tcp_port: Some(7777),
                udp_port: None,
                remote_ip: None,
                patterns: default_patterns(),
            },
            notifications: NotificationsConfig {
                telegram_enabled: true,
                tray_enabled: true,
                cooldown_seconds: 60,
                burst_dedup_ms: default_burst_dedup_ms(),
            },
            startup: StartupConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupConfig {
    #[serde(default)]
    pub auto_start_monitoring: bool,
    #[serde(default)]
    pub start_minimized_to_tray: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            auto_start_monitoring: false,
            start_minimized_to_tray: false,
            close_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    /// Only this chat_id may use /users (empty = nobody).
    #[serde(default)]
    pub admin_chat_id: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub registered_users: Vec<RegisteredUser>,
    #[serde(default = "default_true")]
    pub notify_all_users: bool,
}

fn default_true() -> bool {
    true
}

fn default_burst_dedup_ms() -> u64 {
    2000
}

pub fn validate_pattern_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Pattern name cannot be empty"));
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!("Pattern name cannot be empty"));
    };
    if !first.is_ascii_alphabetic() {
        return Err(anyhow!(
            "Pattern name must start with a Latin letter (A-Z / a-z)"
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(anyhow!(
            "Pattern name: only Latin letters, digits and underscore (e.g. STOP_FARM)"
        ));
    }
    if name.len() > 64 {
        return Err(anyhow!("Pattern name is too long (max 64)"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredUser {
    pub chat_id: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    /// Discover remote IP/port from process TCP table.
    #[default]
    Auto,
    /// Use configured tcp_port / remote_ip.
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub process_name: String,
    #[serde(default)]
    pub capture_mode: CaptureMode,
    #[serde(default = "default_tcp_port")]
    pub tcp_port: Option<u16>,
    #[serde(default)]
    pub udp_port: Option<u16>,
    /// Optional remote server IP filter (WinDivert `ip.SrcAddr`). Manual mode only.
    #[serde(default)]
    pub remote_ip: Option<String>,
    pub patterns: Vec<Pattern>,
}

fn default_tcp_port() -> Option<u16> {
    Some(7777)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub telegram_enabled: bool,
    pub tray_enabled: bool,
    pub cooldown_seconds: u64,
    /// Suppress same pattern on another local IP (LAN+VPN) within this window.
    #[serde(default = "default_burst_dedup_ms")]
    pub burst_dedup_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub bytes: String,
    pub description: String,
}

impl Pattern {
    pub fn new(name: String, bytes: String, description: String) -> Result<Self> {
        let name = name.trim().to_string();
        let description = description.trim().to_string();
        let bytes = bytes.trim().to_string();

        validate_pattern_name(&name)?;
        if description.is_empty() {
            return Err(anyhow!("Pattern description cannot be empty"));
        }
        if bytes.is_empty() {
            return Err(anyhow!("Pattern bytes cannot be empty"));
        }
        let _ = Self {
            name: name.clone(),
            bytes: bytes.clone(),
            description: description.clone(),
        }
        .to_bytes()?;

        Ok(Self {
            name,
            bytes,
            description,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let bytes_str = self.bytes.replace(' ', "");
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

/// Config always lives next to the executable (stable path).
/// Override with env `EW_NOTIFY_CONFIG=/path/to/config.json`.
pub fn get_config_path() -> PathBuf {
    if let Ok(custom) = std::env::var("EW_NOTIFY_CONFIG") {
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join("config.json");
        }
    }

    PathBuf::from("config.json")
}

/// One-time migrate: if exe config is missing but cwd has one, copy it.
fn maybe_migrate_from_cwd(exe_config: &PathBuf) {
    if exe_config.exists() {
        return;
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_config = cwd.join("config.json");
        if cwd_config.exists() && cwd_config != *exe_config {
            if let Err(e) = fs::copy(&cwd_config, exe_config) {
                tracing::warn!(
                    "Could not migrate config {} → {}: {}",
                    cwd_config.display(),
                    exe_config.display(),
                    e
                );
            } else {
                tracing::info!(
                    "Migrated config from {} to {}",
                    cwd_config.display(),
                    exe_config.display()
                );
            }
        }
    }
}

pub fn default_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            name: "DEATH".to_string(),
            bytes: "00FE06032C010000".to_string(),
            description: "Персонаж умер".to_string(),
        },
        Pattern {
            name: "STOP_FARM".to_string(),
            bytes: "4E00FE11006100750074006F006600610072006D000000".to_string(),
            description: "Автофарм остановлен".to_string(),
        },
    ]
}

pub fn ensure_default_patterns(config: &mut AppConfig) -> bool {
    if config.target.patterns.is_empty() {
        config.target.patterns = default_patterns();
        true
    } else {
        false
    }
}

pub fn load_config() -> Result<AppConfig> {
    let path = get_config_path();
    maybe_migrate_from_cwd(&path);
    if !path.exists() {
        return Err(anyhow!("Config file not found at {}", path.display()));
    }
    let content = fs::read_to_string(&path)?;
    let mut config: AppConfig = serde_json::from_str(&content)?;
    if ensure_default_patterns(&mut config) {
        tracing::warn!(
            "config at {} had empty patterns — restored DEATH/STOP_FARM",
            path.display()
        );
        let _ = save_config(&config);
    }
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_to_bytes_ok() {
        let p = Pattern::new(
            "DEATH".into(),
            "00 FE 06 03 2C 01 00 00".into(),
            "desc".into(),
        )
        .unwrap();
        assert_eq!(
            p.to_bytes().unwrap(),
            vec![0x00, 0xFE, 0x06, 0x03, 0x2C, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn pattern_rejects_empty_and_odd_hex() {
        assert!(Pattern::new("".into(), "00".into(), "d".into()).is_err());
        assert!(Pattern::new("x".into(), "".into(), "d".into()).is_err());
        assert!(Pattern::new("x".into(), "00".into(), "".into()).is_err());
        assert!(Pattern::new("x".into(), "0".into(), "d".into()).is_err());
        assert!(Pattern::new("STOP FARM".into(), "00".into(), "d".into()).is_err());
        assert!(Pattern::new("Смерть".into(), "00".into(), "d".into()).is_err());
        assert!(Pattern::new("STOP_FARM".into(), "00FE".into(), "ok".into()).is_ok());
    }

    #[test]
    fn default_config_has_death_and_stop_farm() {
        let cfg = AppConfig::default();
        let names: Vec<_> = cfg.target.patterns.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["DEATH", "STOP_FARM"]);
        assert_eq!(cfg.target.tcp_port, Some(7777));
        for p in &cfg.target.patterns {
            assert!(p.to_bytes().is_ok(), "{} hex must parse", p.name);
        }
    }

    #[test]
    fn config_json_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.target.patterns.len(), 2);
        assert_eq!(back.target.tcp_port, Some(7777));
        assert_eq!(back.notifications.burst_dedup_ms, 2000);
    }

    #[test]
    fn set_user_enabled_toggles() {
        let mut cfg = AppConfig::default();
        add_registered_user(
            &mut cfg,
            RegisteredUser {
                chat_id: "1".into(),
                username: Some("u".into()),
                first_name: None,
                enabled: true,
            },
        );
        set_user_enabled(&mut cfg, "1", false);
        assert!(!cfg.telegram.registered_users[0].enabled);
    }

    #[test]
    fn capture_mode_defaults_to_auto() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.target.capture_mode, CaptureMode::Auto);

        let partial = r#"{
            "telegram": {
                "bot_token": "",
                "registered_users": [],
                "notify_all_users": true
            },
            "target": {
                "process_name": "EndlessWar.exe",
                "patterns": []
            },
            "notifications": {
                "telegram_enabled": true,
                "tray_enabled": true,
                "cooldown_seconds": 60
            }
        }"#;
        let parsed: AppConfig = serde_json::from_str(partial).unwrap();
        assert_eq!(parsed.target.capture_mode, CaptureMode::Auto);
        assert_eq!(parsed.telegram.admin_chat_id, "");
        assert_eq!(parsed.notifications.burst_dedup_ms, 2000);
    }

    #[test]
    fn config_roundtrip_admin_and_manual_mode() {
        let mut cfg = AppConfig::default();
        cfg.telegram.admin_chat_id = "439273164".into();
        cfg.target.capture_mode = CaptureMode::Manual;
        cfg.notifications.burst_dedup_ms = 1500;
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.telegram.admin_chat_id, "439273164");
        assert_eq!(back.target.capture_mode, CaptureMode::Manual);
        assert_eq!(back.notifications.burst_dedup_ms, 1500);
    }

    #[test]
    fn ensure_default_patterns_only_when_empty() {
        let mut empty = AppConfig::default();
        empty.target.patterns.clear();
        assert!(ensure_default_patterns(&mut empty));
        assert_eq!(empty.target.patterns.len(), 2);

        let mut filled = AppConfig::default();
        filled.target.patterns = vec![Pattern {
            name: "CUSTOM".into(),
            bytes: "AA".into(),
            description: "c".into(),
        }];
        assert!(!ensure_default_patterns(&mut filled));
        assert_eq!(filled.target.patterns.len(), 1);
        assert_eq!(filled.target.patterns[0].name, "CUSTOM");
    }

    #[test]
    fn validate_pattern_name_edges() {
        assert!(validate_pattern_name("A").is_ok());
        assert!(validate_pattern_name("STOP_FARM_2").is_ok());
        assert!(validate_pattern_name("1BAD").is_err());
        assert!(validate_pattern_name("_BAD").is_err());
        assert!(validate_pattern_name("bad-name").is_err());
        assert!(validate_pattern_name(&"A".repeat(65)).is_err());
        assert!(validate_pattern_name(&"A".repeat(64)).is_ok());
    }

    #[test]
    fn add_registered_user_no_dup() {
        let mut cfg = AppConfig::default();
        let u = RegisteredUser {
            chat_id: "42".into(),
            username: None,
            first_name: Some("A".into()),
            enabled: true,
        };
        add_registered_user(&mut cfg, u.clone());
        add_registered_user(&mut cfg, u);
        assert_eq!(cfg.telegram.registered_users.len(), 1);
    }
}
