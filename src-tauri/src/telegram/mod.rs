use crate::config::AppConfig;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

pub async fn send_notification(
    config: &Arc<Mutex<AppConfig>>,
    name: &str,
    description: &str,
) -> Result<()> {
    let config_guard = config.lock().await;

    if !config_guard.notifications.telegram_enabled {
        return Ok(());
    }

    let token = config_guard.telegram.bot_token.clone();
    let chat_id = config_guard.telegram.chat_id.clone();

    if token.is_empty() || chat_id.is_empty() {
        info!("Telegram notification skipped: no token or chat_id");
        return Ok(());
    }

    drop(config_guard);

    let bot = teloxide::Bot::new(&token);
    let message = format!("🔔 *EndlessWar Event*\n\n*Pattern:* {}\n*Description:* {}", name, description);

    bot.send_message(&chat_id, message)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .send()
        .await?;

    info!("Telegram notification sent: {}", name);
    Ok(())
}

pub async fn test_connection(token: &str, chat_id: &str) -> Result<()> {
    let bot = teloxide::Bot::new(token);
    bot.send_message(chat_id, "✅ EndlessWar Notifier connected!")
        .send()
        .await?;
    Ok(())
}