use crate::config::{AppConfig, RegisteredUser};
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
    let notify_all = config_guard.telegram.notify_all_users;
    let enabled_users: Vec<RegisteredUser> = config_guard.telegram.registered_users
        .iter()
        .filter(|u| u.enabled)
        .cloned()
        .collect();

    if token.is_empty() {
        info!("Telegram notification skipped: no token");
        return Ok(());
    }

    drop(config_guard);

    let bot = teloxide::Bot::new(&token);
    let message = format!("🔔 EndlessWar Event\n\nPattern: {}\nDescription: {}", name, description);

    if notify_all {
        for user in enabled_users {
            let chat_id_num: i64 = user.chat_id.parse().unwrap_or(0);
            if chat_id_num != 0 {
                let _ = bot.send_message(ChatId(chat_id_num), &message)
                    .send()
                    .await;
            }
        }
    }

    info!("Telegram notification sent: {}", name);
    Ok(())
}

pub async fn test_connection(token: &str) -> anyhow::Result<()> {
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
    Ok(())
}

pub async fn start_bot(token: &str, config: Arc<Mutex<AppConfig>>) -> anyhow::Result<()> {
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
        BotCommand {
            command: "users".to_string(),
            description: "Список пользователей (админ)".to_string(),
        },
    ];
    if let Err(e) = bot.set_my_commands(commands).send().await {
        tracing::error!("Failed to set bot commands: {}", e);
    }

    info!("Telegram bot started, waiting for messages...");

    let mut offset: i32 = 0;
    loop {
        let updates_result = bot.get_updates()
            .offset(offset)
            .timeout(60)
            .send()
            .await;
        
        match updates_result {
            Ok(updates) => {
                if updates.is_empty() {
                    continue;
                }
                for update in updates {
                    if let UpdateKind::Message(message) = &update.kind {
                        info!("Received message: {:?}", message.text());
                            
                        if let Some(text) = message.text().as_ref() {
                            let chat_id = message.chat.id.0;
                            let username = message.chat.username().map(|s| s.to_string());
                            let first_name = message.chat.first_name().map(|s| s.to_string());
                            
                            info!("Chat ID: {}, Text: {}", chat_id, text);
                            
                            if *text == "/start" {
                                let new_user = RegisteredUser {
                                    chat_id: chat_id.to_string(),
                                    username: username.clone(),
                                    first_name: first_name.clone(),
                                    enabled: true,
                                };
                                
                                {
                                    let mut config_guard = config.lock().await;
                                    crate::config::add_registered_user(&mut config_guard, new_user.clone());
                                    let _ = crate::config::save_config(&config_guard);
                                }
                                
                                let welcome = format!(
                                    "👋 Привет, {}!\nДобро пожаловать в EndlessWar Notifier!\n\n\
📋 КОМАНДЫ:\n\
/myid - Узнать свой ID\n\
/start - Зарегистрироваться\n\n\
✅ Ты зарегистрирован! Админ добавит тебя в список рассылки.",
                                    first_name.unwrap_or_else(|| "друг".to_string())
                                );
                                if let Err(e) = bot.send_message(ChatId(chat_id), welcome)
                                    .send()
                                    .await
                                {
                                    tracing::error!("Failed to send welcome message: {}", e);
                                }
                                info!("User registered: {} ({})", chat_id, username.unwrap_or_default());
                            }
                            else if *text == "/myid" {
                                let msg = format!("🆔 Ваш Telegram ID: {}\n\nПередай этот ID администратору для получения уведомлений.", chat_id);
                                if let Err(e) = bot.send_message(ChatId(chat_id), msg)
                                    .send()
                                    .await
                                {
                                    tracing::error!("Failed to send myid response: {}", e);
                                }
                            }
                            else if *text == "/users" {
                                let config_guard = config.lock().await;
                                let users: Vec<RegisteredUser> = config_guard.telegram.registered_users.clone();
                                let notify_all = config_guard.telegram.notify_all_users;
                                
                                let mut user_list = String::from("📋 ЗАРЕГИСТРИРОВАННЫЕ ПОЛЬЗОВАТЕЛИ:\n\n");
                                for user in &users {
                                    let status = if user.enabled { "✅" } else { "❌" };
                                    let name = user.first_name.clone().unwrap_or_else(|| "Unknown".to_string());
                                    user_list.push_str(&format!("{} {} (@{})\n", status, name, user.username.as_ref().unwrap_or(&"-".to_string())));
                                }
                                user_list.push_str(&format!("\n📊 Рассылка всем: {}", if notify_all { "включена" } else { "выключена" }));
                                
                                if let Err(e) = bot.send_message(ChatId(chat_id), user_list)
                                    .send()
                                    .await
                                {
                                    tracing::error!("Failed to send users list: {}", e);
                                }
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
