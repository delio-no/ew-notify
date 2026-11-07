use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub telegram: TelegramConfig,
    #[serde(default)]
    pub target: TargetConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub startup: StartupConfig,
    #[serde(default)]
    pub clicker: ClickerConfig,
    #[serde(default)]
    pub updater: UpdaterConfig,
    #[serde(default)]
    pub streamer: StreamerConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            telegram: TelegramConfig::default(),
            target: TargetConfig::default(),
            notifications: NotificationsConfig::default(),
            startup: StartupConfig::default(),
            clicker: ClickerConfig::default(),
            updater: UpdaterConfig::default(),
            streamer: StreamerConfig::default(),
        }
    }
}

fn default_rtmp_url() -> String {
    "rtmps://live.me.t.me/live".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamerConfig {
    #[serde(default)]
    pub udp_port: Option<u16>,
    #[serde(default)]
    pub turn_server: Option<String>,
    #[serde(default)]
    pub turn_username: Option<String>,
    #[serde(default)]
    pub turn_password: Option<String>,
    #[serde(default = "default_rtmp_url")]
    pub rtmp_url: String,
    #[serde(default)]
    pub rtmp_key: String,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            udp_port: None,
            turn_server: None,
            turn_username: None,
            turn_password: None,
            rtmp_url: default_rtmp_url(),
            rtmp_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdaterConfig {
    #[serde(default = "default_true")]
    pub check_on_startup: bool,
    #[serde(default)]
    pub ignored_version: Option<String>,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            check_on_startup: true,
            ignored_version: None,
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
    #[serde(default)]
    pub bot_token: String,
    /// Only this chat_id may use /users (empty = nobody).
    #[serde(default, alias = "chat_id")]
    pub admin_chat_id: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub registered_users: Vec<RegisteredUser>,
    #[serde(default = "default_true")]
    pub notify_all_users: bool,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            admin_chat_id: String::new(),
            auto_start: false,
            registered_users: Vec::new(),
            notify_all_users: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_burst_dedup_ms() -> u64 {
    2000
}

fn default_clicker_key() -> String {
    "1".to_string()
}

fn default_clicker_interval_ms() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum ClickerTarget {
    Captured,
    Slot(u32),
    All,
}

impl Default for ClickerTarget {
    fn default() -> Self {
        Self::Slot(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClickerRule {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_clicker_key")]
    pub key: String,
    #[serde(default = "default_clicker_interval_ms")]
    pub interval_ms: u64,
    #[serde(default)]
    pub target: ClickerTarget,
    #[serde(default)]
    pub stop_on_pattern: bool,
    #[serde(default)]
    pub stop_patterns: Vec<String>,
    #[serde(default)]
    pub start_on_pattern: bool,
    #[serde(default)]
    pub start_patterns: Vec<String>,
    #[serde(default)]
    pub single_press: bool,
}

impl Default for ClickerRule {
    fn default() -> Self {
        Self {
            id: "rule-1".to_string(),
            name: "Клавиша 1".to_string(),
            enabled: true,
            key: "1".to_string(),
            interval_ms: 1000,
            target: ClickerTarget::Slot(1),
            stop_on_pattern: false,
            stop_patterns: Vec::new(),
            start_on_pattern: false,
            start_patterns: Vec::new(),
            single_press: false,
        }
    }
}

fn default_clicker_rules() -> Vec<ClickerRule> {
    vec![ClickerRule::default()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SlotNotificationConfig {
    #[serde(default)]
    pub notify_on_stop: bool,
    #[serde(default)]
    pub notify_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClickerConfig {
    #[serde(default = "default_clicker_rules")]
    pub rules: Vec<ClickerRule>,
    #[serde(default)]
    pub notify_on_stop: bool,
    #[serde(default)]
    pub notify_on_start: bool,
    #[serde(default)]
    pub slot_notifications: std::collections::HashMap<String, SlotNotificationConfig>,
    // Legacy fields for backward compatibility
    #[serde(default, skip_serializing)]
    pub keys: Vec<String>,
    #[serde(default, skip_serializing)]
    pub key: String,
    #[serde(default, skip_serializing)]
    pub interval_ms: u64,
    #[serde(default, skip_serializing)]
    pub send_to_all_windows: bool,
    #[serde(default, skip_serializing)]
    pub stop_on_pattern: bool,
    #[serde(default, skip_serializing)]
    pub stop_patterns: Vec<String>,
}

impl ClickerConfig {
    pub fn is_notify_on_stop_for_slot(&self, slot: Option<u32>) -> bool {
        if let Some(s) = slot {
            let key = format!("slot-{}", s);
            if let Some(slot_cfg) = self.slot_notifications.get(&key) {
                return slot_cfg.notify_on_stop;
            }
        } else if let Some(all_cfg) = self.slot_notifications.get("all-windows") {
            return all_cfg.notify_on_stop;
        }
        self.notify_on_stop
    }

    pub fn is_notify_on_start_for_slot(&self, slot: Option<u32>) -> bool {
        if let Some(s) = slot {
            let key = format!("slot-{}", s);
            if let Some(slot_cfg) = self.slot_notifications.get(&key) {
                return slot_cfg.notify_on_start;
            }
        } else if let Some(all_cfg) = self.slot_notifications.get("all-windows") {
            return all_cfg.notify_on_start;
        }
        self.notify_on_start
    }

    pub fn get_rules(&self) -> Vec<ClickerRule> {
        if !self.rules.is_empty() {
            return self.rules.clone();
        }
        let keys = if !self.keys.is_empty() {
            self.keys.clone()
        } else if !self.key.is_empty() {
            vec![self.key.clone()]
        } else {
            vec!["1".to_string()]
        };
        let interval = if self.interval_ms > 0 { self.interval_ms } else { 1000 };
        let target = if self.send_to_all_windows {
            ClickerTarget::All
        } else {
            ClickerTarget::Slot(1)
        };
        keys.into_iter()
            .enumerate()
            .map(|(i, k)| ClickerRule {
                id: format!("rule-{}", i + 1),
                name: format!("Клавиша {}", k),
                enabled: true,
                key: k,
                interval_ms: interval,
                target: target.clone(),
                stop_on_pattern: self.stop_on_pattern,
                stop_patterns: self.stop_patterns.clone(),
                start_on_pattern: false,
                start_patterns: Vec::new(),
                single_press: false,
            })
            .collect()
    }
}

impl Default for ClickerConfig {
    fn default() -> Self {
        Self {
            rules: default_clicker_rules(),
            notify_on_stop: false,
            notify_on_start: false,
            slot_notifications: std::collections::HashMap::new(),
            keys: Vec::new(),
            key: "1".to_string(),
            interval_ms: 1000,
            send_to_all_windows: false,
            stop_on_pattern: false,
            stop_patterns: Vec::new(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredUser {
    pub chat_id: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub allow_screenshot: bool,
    #[serde(default)]
    pub allow_stream: bool,
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

fn default_process_name() -> String {
    "EndlessWar.exe".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(default = "default_process_name")]
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
    #[serde(default = "default_patterns")]
    pub patterns: Vec<Pattern>,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            process_name: default_process_name(),
            capture_mode: CaptureMode::default(),
            tcp_port: default_tcp_port(),
            udp_port: None,
            remote_ip: None,
            patterns: default_patterns(),
        }
    }
}

fn default_tcp_port() -> Option<u16> {
    Some(7777)
}

fn default_cooldown_seconds() -> u64 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default = "default_true")]
    pub telegram_enabled: bool,
    #[serde(default = "default_true")]
    pub tray_enabled: bool,
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: u64,
    /// Suppress same pattern on another local IP (LAN+VPN) within this window.
    #[serde(default = "default_burst_dedup_ms")]
    pub burst_dedup_ms: u64,
    /// Delay before capturing screenshot so game finishes animations and updates UI state.
    #[serde(default = "default_screenshot_delay_ms")]
    pub screenshot_delay_ms: u64,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            telegram_enabled: true,
            tray_enabled: true,
            cooldown_seconds: default_cooldown_seconds(),
            burst_dedup_ms: default_burst_dedup_ms(),
            screenshot_delay_ms: default_screenshot_delay_ms(),
        }
    }
}

fn default_screenshot_delay_ms() -> u64 {
    1500
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pattern {
    pub name: String,
    pub bytes: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
    #[serde(default)]
    pub screenshot_enabled: bool,
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
        let is_base = name == "DEATH" || name == "STOP_FARM";
        let _ = Self {
            name: name.clone(),
            bytes: bytes.clone(),
            description: description.clone(),
            notify_enabled: true,
            screenshot_enabled: is_base,
        }
        .to_bytes()?;

        Ok(Self {
            name,
            bytes,
            description,
            notify_enabled: true,
            screenshot_enabled: is_base,
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
/// Override with env `EW_NOTIFY_CONFIG=/path/to/config.json` (или `EW-NOTIFY-CONFIG`).
pub fn get_config_path() -> PathBuf {
    if let Ok(custom) = std::env::var("EW_NOTIFY_CONFIG").or_else(|_| std::env::var("EW-NOTIFY-CONFIG")) {
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
            notify_enabled: true,
            screenshot_enabled: true,
        },
        Pattern {
            name: "STOP_FARM".to_string(),
            bytes: "4E00FE11006100750074006F006600610072006D000000".to_string(),
            description: "Автофарм остановлен".to_string(),
            notify_enabled: true,
            screenshot_enabled: true,
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

/// Recursively merge patch JSON into target JSON (in-place).
pub fn deep_merge_json(target: &mut serde_json::Value, patch: serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(t), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                deep_merge_json(t.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (target, patch) => *target = patch,
    }
}

/// Run step-by-step migrations on raw JSON value before deserialization.
pub fn migrate_raw_json(mut val: serde_json::Value) -> Result<(serde_json::Value, bool)> {
    let raw_version = val
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let mut migrated = false;

    if raw_version < 1 {
        val = migrate_legacy_to_v1(val)?;
        migrated = true;
    }

    // Strip obsolete fields from notifications if present
    if let Some(notif) = val.get_mut("notifications").and_then(|n| n.as_object_mut()) {
        if notif.remove("screenshot_on_death").is_some() || notif.remove("actions_enabled").is_some() {
            migrated = true;
        }
    }

    // Strip obsolete action field and ensure base patterns screenshot_enabled default
    if let Some(patterns) = val.get_mut("target").and_then(|t| t.get_mut("patterns")).and_then(|p| p.as_array_mut()) {
        for pat in patterns {
            if let Some(obj) = pat.as_object_mut() {
                if obj.remove("action").is_some() {
                    migrated = true;
                }
                if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                    if (name == "DEATH" || name == "STOP_FARM") && !obj.contains_key("screenshot_enabled") {
                        obj.insert("screenshot_enabled".to_string(), serde_json::json!(true));
                        migrated = true;
                    }
                }
            }
        }
    }

    val["schema_version"] = serde_json::json!(CURRENT_SCHEMA_VERSION);
    Ok((val, migrated))
}

fn migrate_legacy_to_v1(mut val: serde_json::Value) -> Result<serde_json::Value> {
    if let Some(tg) = val.get_mut("telegram").and_then(|t| t.as_object_mut()) {
        if let Some(old_chat) = tg.get("chat_id").cloned() {
            let needs_admin = !tg.contains_key("admin_chat_id")
                || tg["admin_chat_id"].as_str().unwrap_or("").is_empty();
            if needs_admin {
                tg.insert("admin_chat_id".to_string(), old_chat);
            }
        }
    }
    Ok(val)
}

fn load_and_finalize_config(
    raw_json: serde_json::Value,
    path: &std::path::Path,
    recovered_from_bak: bool,
) -> Result<AppConfig> {
    let (migrated_json, was_migrated) = migrate_raw_json(raw_json)?;

    let default_cfg = AppConfig::default();
    let mut base_json = serde_json::to_value(&default_cfg)?;
    deep_merge_json(&mut base_json, migrated_json);

    let mut config: AppConfig = serde_json::from_value(base_json)?;

    let raw_token = config.telegram.bot_token.clone();
    let was_encrypted = raw_token.starts_with("enc:dpapi:");

    match crate::crypto::decrypt_string(&raw_token) {
        Ok(decrypted) => config.telegram.bot_token = decrypted,
        Err(e) => {
            tracing::error!("Failed to decrypt telegram.bot_token: {e}");
        }
    }

    let patterns_restored = ensure_default_patterns(&mut config);
    if patterns_restored {
        tracing::info!(
            "Appended missing default patterns to config at {}",
            path.display()
        );
    }

    if (!was_encrypted && !config.telegram.bot_token.is_empty())
        || patterns_restored
        || was_migrated
        || recovered_from_bak
    {
        let _ = save_config_to_path(&config, path);
    }

    Ok(config)
}

pub fn load_config_from_path(path: &std::path::Path) -> Result<AppConfig> {
    if !path.exists() {
        return Err(anyhow!("Config file not found at {}", path.display()));
    }
    let content = fs::read_to_string(path)?;

    let raw_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                "Config file {} is corrupted (invalid JSON syntax): {e}",
                path.display()
            );
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            let broken_path = path.with_extension(format!("corrupted.{timestamp}.json"));
            let _ = fs::copy(path, &broken_path);
            tracing::warn!("Saved corrupted config copy to {}", broken_path.display());

            let bak_path = path.with_extension("json.bak");
            if bak_path.exists() {
                tracing::info!("Attempting recovery from backup {}", bak_path.display());
                if let Ok(bak_content) = fs::read_to_string(&bak_path) {
                    if let Ok(v) = serde_json::from_str(&bak_content) {
                        return load_and_finalize_config(v, path, true);
                    }
                }
            }
            return Err(anyhow!(
                "Failed to parse config and backup recovery unavailable: {e}"
            ));
        }
    };

    load_and_finalize_config(raw_json, path, false)
}

pub fn save_config_to_path(config: &AppConfig, path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // Always backup existing config file before overwriting
    if path.exists() {
        let bak_path = path.with_extension("json.bak");
        let _ = fs::copy(path, &bak_path);
    }

    let mut disk_config = config.clone();
    if !disk_config.telegram.bot_token.is_empty() {
        match crate::crypto::encrypt_string(&disk_config.telegram.bot_token) {
            Ok(enc) => disk_config.telegram.bot_token = enc,
            Err(e) => {
                tracing::error!("Could not encrypt bot_token for disk save: {e}");
            }
        }
    }
    let content = serde_json::to_string_pretty(&disk_config)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn load_config() -> Result<AppConfig> {
    let path = get_config_path();
    maybe_migrate_from_cwd(&path);
    load_config_from_path(&path)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = get_config_path();
    save_config_to_path(config, &path)
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

pub fn set_user_screenshot_allowed(config: &mut AppConfig, chat_id: &str, allowed: bool) {
    if let Some(user) = config
        .telegram
        .registered_users
        .iter_mut()
        .find(|u| u.chat_id.trim() == chat_id.trim())
    {
        user.allow_screenshot = allowed;
    }
}

pub fn can_user_screenshot(config: &AppConfig, chat_id: &str) -> bool {
    let clean_id = chat_id.trim();
    if !config.telegram.admin_chat_id.trim().is_empty()
        && config.telegram.admin_chat_id.trim() == clean_id
    {
        return true;
    }
    config
        .telegram
        .registered_users
        .iter()
        .any(|u| u.chat_id.trim() == clean_id && u.enabled && u.allow_screenshot)
}

pub fn set_user_stream_allowed(config: &mut AppConfig, chat_id: &str, allowed: bool) {
    if let Some(user) = config
        .telegram
        .registered_users
        .iter_mut()
        .find(|u| u.chat_id.trim() == chat_id.trim())
    {
        user.allow_stream = allowed;
    }
}

pub fn can_user_stream(config: &AppConfig, chat_id: &str) -> bool {
    let clean_id = chat_id.trim();
    if !config.telegram.admin_chat_id.trim().is_empty()
        && config.telegram.admin_chat_id.trim() == clean_id
    {
        return true;
    }
    config
        .telegram
        .registered_users
        .iter()
        .any(|u| u.chat_id.trim() == clean_id && u.enabled && u.allow_stream)
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
            assert!(p.screenshot_enabled, "{} should have screenshot_enabled=true by default", p.name);
        }

        let custom_json = r#"{"name":"CUSTOM","bytes":"AA","description":"desc"}"#;
        let p: Pattern = serde_json::from_str(custom_json).unwrap();
        assert!(!p.screenshot_enabled, "custom pattern must have screenshot_enabled=false by default");
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
                allow_screenshot: false,
                allow_stream: false,
            },
        );
        set_user_enabled(&mut cfg, "1", false);
        assert!(!cfg.telegram.registered_users[0].enabled);
    }

    #[test]
    fn test_user_screenshot_permissions() {
        let mut cfg = AppConfig::default();
        cfg.telegram.admin_chat_id = "100".into();
        add_registered_user(
            &mut cfg,
            RegisteredUser {
                chat_id: "200".into(),
                username: None,
                first_name: None,
                enabled: true,
                allow_screenshot: false,
                allow_stream: false,
            },
        );

        // Admin can always screenshot
        assert!(can_user_screenshot(&cfg, "100"));
        // Regular user cannot screenshot by default
        assert!(!can_user_screenshot(&cfg, "200"));
        // Unknown user cannot screenshot
        assert!(!can_user_screenshot(&cfg, "999"));

        // Allow regular user
        set_user_screenshot_allowed(&mut cfg, "200", true);
        assert!(can_user_screenshot(&cfg, "200"));

        // If user is disabled, cannot screenshot even if allow_screenshot is true
        set_user_enabled(&mut cfg, "200", false);
        assert!(!can_user_screenshot(&cfg, "200"));
    }

    #[test]
    fn test_user_stream_permissions() {
        let mut cfg = AppConfig::default();
        cfg.telegram.admin_chat_id = "100".into();
        add_registered_user(
            &mut cfg,
            RegisteredUser {
                chat_id: "200".into(),
                username: None,
                first_name: None,
                enabled: true,
                allow_screenshot: false,
                allow_stream: false,
            },
        );

        // Admin can always stream
        assert!(can_user_stream(&cfg, "100"));
        // Regular user cannot stream by default
        assert!(!can_user_stream(&cfg, "200"));
        // Unknown user cannot stream
        assert!(!can_user_stream(&cfg, "999"));

        // Allow regular user
        set_user_stream_allowed(&mut cfg, "200", true);
        assert!(can_user_stream(&cfg, "200"));

        // If user is disabled, cannot stream even if allow_stream is true
        set_user_enabled(&mut cfg, "200", false);
        assert!(!can_user_stream(&cfg, "200"));
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
            notify_enabled: true,
            screenshot_enabled: false,
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
            allow_screenshot: false,
            allow_stream: false,
        };
        add_registered_user(&mut cfg, u.clone());
        add_registered_user(&mut cfg, u);
        assert_eq!(cfg.telegram.registered_users.len(), 1);
    }

    #[test]
    fn test_config_token_encrypted_on_disk() {
        let temp_dir = std::env::temp_dir().join(format!("ew_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let config_path = temp_dir.join("config.json");

        let mut cfg = AppConfig::default();
        cfg.telegram.bot_token = "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ_123456".into();

        // 1. Save to disk -> must be encrypted on disk
        save_config_to_path(&cfg, &config_path).expect("save failed");
        let disk_content = fs::read_to_string(&config_path).expect("read failed");
        assert!(disk_content.contains("enc:dpapi:"), "Token on disk must be encrypted with enc:dpapi:");
        assert!(!disk_content.contains("123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ_123456"), "Plaintext token must not be written to disk");

        // 2. Load from disk -> in memory must be plaintext
        let loaded = load_config_from_path(&config_path).expect("load failed");
        assert_eq!(loaded.telegram.bot_token, "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ_123456");

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_config_plaintext_token_auto_migration() {
        let temp_dir = std::env::temp_dir().join(format!("ew_test_mig_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let config_path = temp_dir.join("config.json");
        fs::create_dir_all(&temp_dir).unwrap();

        let raw_json = r#"{
            "telegram": {
                "bot_token": "plain_token_12345",
                "admin_chat_id": "",
                "auto_start": false,
                "registered_users": [],
                "notify_all_users": true
            },
            "target": {
                "process_name": "EndlessWar.exe",
                "capture_mode": "auto",
                "tcp_port": 7777,
                "udp_port": null,
                "remote_ip": null,
                "patterns": [
                    { "name": "DEATH", "bytes": "00FE", "description": "d" }
                ]
            },
            "notifications": {
                "telegram_enabled": true,
                "tray_enabled": true,
                "cooldown_seconds": 60,
                "burst_dedup_ms": 2000
            },
            "startup": {
                "auto_start_monitoring": false,
                "start_minimized_to_tray": false,
                "close_to_tray": true
            }
        }"#;
        fs::write(&config_path, raw_json).unwrap();

        let loaded = load_config_from_path(&config_path).expect("load failed");
        assert_eq!(loaded.telegram.bot_token, "plain_token_12345");

        let disk_content_after = fs::read_to_string(&config_path).unwrap();
        assert!(disk_content_after.contains("enc:dpapi:"), "Plaintext token must be automatically migrated to encrypted on disk");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_migrate_legacy_config_adds_schema_version_and_defaults() {
        let legacy_json = serde_json::json!({
            "telegram": {
                "bot_token": "token",
                "chat_id": "999"
            },
            "target": {
                "process_name": "EndlessWar.exe"
            }
        });

        let (migrated, was_migrated) = migrate_raw_json(legacy_json).unwrap();
        assert!(was_migrated);
        assert_eq!(migrated["schema_version"], CURRENT_SCHEMA_VERSION);
        assert_eq!(migrated["telegram"]["admin_chat_id"], "999");
    }

    #[test]
    fn test_deep_merge_preserves_user_settings_and_populates_new_fields() {
        let mut base = serde_json::json!({
            "telegram": { "bot_token": "", "admin_chat_id": "", "notify_all_users": true },
            "updater": { "check_on_startup": true, "ignored_version": null }
        });

        let user = serde_json::json!({
            "telegram": { "bot_token": "my_secret_token" }
        });

        deep_merge_json(&mut base, user);
        assert_eq!(base["telegram"]["bot_token"], "my_secret_token");
        assert_eq!(base["telegram"]["notify_all_users"], true);
        assert_eq!(base["updater"]["check_on_startup"], true);
    }

    #[test]
    fn test_corrupted_config_quarantine_and_backup_recovery() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ew_test_corrupt_{unique_id}"));
        let _ = fs::create_dir_all(&temp_dir);
        let config_path = temp_dir.join("config.json");
        let bak_path = temp_dir.join("config.json.bak");

        // Create a valid backup
        let valid_backup = AppConfig::default();
        save_config_to_path(&valid_backup, &bak_path).unwrap();

        // Write corrupt JSON to main config file
        fs::write(&config_path, "{ broken invalid json...").unwrap();

        // Loading should recover from backup and save quarantine file
        let loaded = load_config_from_path(&config_path).expect("Should recover from backup");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);

        // Verify corrupted file was quarantined
        let files: Vec<_> = fs::read_dir(&temp_dir).unwrap().filter_map(|e| e.ok()).collect();
        let has_corrupted = files.iter().any(|f| f.file_name().to_string_lossy().contains("corrupted"));
        assert!(has_corrupted, "Corrupted file must be preserved in quarantine");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
