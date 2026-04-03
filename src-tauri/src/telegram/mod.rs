use crate::config::AppConfig;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use tracing::info;
use teloxide::prelude::{Request, Requester};
use teloxide::payloads::SendMessageSetters;
use teloxide::types::{ChatId, BotCommand};

pub async fn send_notification(
    config: &Arc<Mutex<AppConfig>>,
    name: &str,
    description: &str,
) -> anyhow::Result<()> {
    let config_guard = config.lock().await;

    if !config_guard.notifications.telegram_enabled {
        return Ok(());
    }

    let token = config_guard.telegram.bot_token.clone();
    let admin_chat_id = config_guard.telegram.admin_chat_id.clone();

    if token.is_empty() || admin_chat_id.is_empty() {
        info!("Telegram notification skipped: no token or admin_chat_id");
        return Ok(());
    }

    drop(config_guard);

    let bot = teloxide::Bot::new(&token);
    let message = format!("🔔 *EndlessWar Event*\n\n*Pattern:* {}\n*Description:* {}", name, description);

    let chat_id_num: i64 = admin_chat_id.parse().unwrap_or(0);
    bot.send_message(ChatId(chat_id_num), message)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .send()
        .await?;

    info!("Telegram notification sent to admin: {}", name);
    Ok(())
}

pub async fn test_connection(token: &str, admin_chat_id: &str) -> anyhow::Result<()> {
    let bot = teloxide::Bot::new(token);
    
    let commands = vec![
        BotCommand {
            command: "myid".to_string(),
            description: "Узнать свой Telegram ID".to_string(),
        },
        BotCommand {
            command: "start".to_string(),
            description: "Зарегистрироваться в боте".to_string(),
        },
    ];
    let _ = bot.set_my_commands(commands).send();
    
    if !admin_chat_id.is_empty() {
        let chat_id_num: i64 = admin_chat_id.parse().unwrap_or(0);
        bot.send_message(ChatId(chat_id_num), "✅ EndlessWar Notifier подключён!\n\nТы будешь получать уведомления о событиях в игре.")
            .send()
            .await?;
    }
    Ok(())
}

pub async fn start_bot(token: &str) -> anyhow::Result<()> {
    use teloxide::payloads::GetUpdatesSetters;
    use teloxide::types::UpdateKind;
    
    let bot = teloxide::Bot::new(token);
    
    let commands = vec![
        BotCommand {
            command: "myid".to_string(),
            description: "Узнать свой Telegram ID".to_string(),
        },
        BotCommand {
            command: "start".to_string(),
            description: "Зарегистрироваться".to_string(),
        },
    ];
    let _ = bot.set_my_commands(commands).send();

    info!("Telegram bot started");

    let mut offset: i32 = 0;
    loop {
        let updates = bot.get_updates()
            .offset(offset)
            .timeout(30)
            .send()
            .await;
        
        match updates {
            Ok(updates) => {
                for update in updates {
                    if let UpdateKind::Message(message) = &update.kind {
                        if let Some(text) = message.text().as_ref() {
                            let chat_id = message.chat.id.0;
                            
                            if text == "/start" {
                                let welcome = "👋 Привет! Добро пожаловать в EndlessWar Notifier!\n\n\
📋 *Команды:*\n\
/myid - Узнать свой ID\n\
/start - Зарегистрироваться\n\n\
⚠️ Чтобы получать уведомления, сообщи админу свой ID: `/myid`";
                                let _ = bot.send_message(ChatId(chat_id), welcome)
                                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                                    .send()
                                    .await;
                                info!("User {} sent /start", chat_id);
                            }
                            else if text == "/myid" {
                                let msg = format!("🆔 Ваш Telegram ID: `{}`\n\nПередай этот ID администратору для получения уведомлений.", chat_id);
                                let _ = bot.send_message(ChatId(chat_id), msg)
                                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                                    .send()
                                    .await;
                            }
                        }
                    }
                    offset = (update.id.0 as i32) + 1;
                }
            }
            Err(e) => {
                tracing::warn!("Error getting updates: {:?}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}
