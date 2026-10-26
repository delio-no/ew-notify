use crate::config::{AppConfig, RegisteredUser};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::prelude::{Request, Requester};
use teloxide::types::{
    BotCommand, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile,
};
use tokio::sync::Mutex;
use tracing::info;

pub async fn send_notification(
    config: &Arc<Mutex<AppConfig>>,
    event_title: &str,
    pattern_id: &str,
) -> anyhow::Result<()> {
    let config_guard = config.lock().await;

    if !config_guard.notifications.telegram_enabled {
        return Ok(());
    }

    let token = config_guard.telegram.bot_token.clone();
    let notify_all = config_guard.telegram.notify_all_users;
    let enabled_users: Vec<RegisteredUser> = config_guard
        .telegram
        .registered_users
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
    let message = if pattern_id.is_empty() || pattern_id == event_title {
        format!("🔔 {event_title}")
    } else {
        format!("🔔 {event_title}\n\n{pattern_id}")
    };

    if !notify_all {
        info!("Telegram notification skipped: notify_all_users=false");
        return Ok(());
    }

    if enabled_users.is_empty() {
        return Err(anyhow::anyhow!("нет включённых получателей"));
    }

    let mut sent = 0usize;
    let mut last_err: Option<String> = None;
    for user in enabled_users {
        let chat_id_num: i64 = user.chat_id.parse().unwrap_or(0);
        if chat_id_num == 0 {
            continue;
        }
        match bot
            .send_message(ChatId(chat_id_num), &message)
            .send()
            .await
        {
            Ok(_) => sent += 1,
            Err(e) => {
                tracing::warn!("Telegram send to {} failed: {}", user.chat_id, e);
                last_err = Some(e.to_string());
            }
        }
    }

    if sent == 0 {
        return Err(anyhow::anyhow!(
            last_err.unwrap_or_else(|| "не удалось отправить ни одному получателю".into())
        ));
    }

    info!("Telegram notification sent to {} user(s): {}", sent, event_title);
    Ok(())
}

pub async fn test_connection(token: &str) -> anyhow::Result<()> {
    let bot = teloxide::Bot::new(token);

    let commands = vec![
        BotCommand {
            command: "screen".to_string(),
            description: "Скриншот игровых окон".to_string(),
        },
        BotCommand {
            command: "myid".to_string(),
            description: "Узнать свой Telegram ID".to_string(),
        },
        BotCommand {
            command: "start".to_string(),
            description: "Зарегистрироваться в боте".to_string(),
        },
    ];
    let _ = bot.set_my_commands(commands).send().await;
    let _ = bot.get_me().send().await?;
    Ok(())
}

fn is_terminated_by_other(err: &teloxide::RequestError) -> bool {
    let s = format!("{:?}", err);
    s.contains("TerminatedByOtherGetUpdates")
}

pub async fn start_bot(
    token: &str,
    config: Arc<Mutex<AppConfig>>,
    running: Arc<AtomicBool>,
    app: AppHandle,
) -> anyhow::Result<()> {
    use teloxide::payloads::{
        AnswerCallbackQuerySetters, GetUpdatesSetters, SendMessageSetters, SendPhotoSetters,
    };
    use teloxide::types::UpdateKind;

    let bot = teloxide::Bot::new(token);

    let commands = vec![
        BotCommand {
            command: "screen".to_string(),
            description: "Скриншот игровых окон".to_string(),
        },
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
    running.store(true, Ordering::SeqCst);

    let mut offset: i32 = 0;
    let mut backoff_secs: u64 = 1;
    while running.load(Ordering::SeqCst) {
        let updates_result = bot
            .get_updates()
            .offset(offset)
            .timeout(5)
            .send()
            .await;

        if !running.load(Ordering::SeqCst) {
            break;
        }

        match updates_result {
            Ok(updates) => {
                backoff_secs = 1;
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

                            if *text == "/start" {
                                let new_user = RegisteredUser {
                                    chat_id: chat_id.to_string(),
                                    username: username.clone(),
                                    first_name: first_name.clone(),
                                    enabled: true,
                                    allow_screenshot: false,
                                };

                                {
                                    let mut config_guard = config.lock().await;
                                    crate::config::add_registered_user(
                                        &mut config_guard,
                                        new_user.clone(),
                                    );
                                    let _ = crate::config::save_config(&config_guard);
                                }

                                let _ = app.emit("users-updated", ());

                                let welcome = format!(
                                    "👋 Привет, {}!\nДобро пожаловать в EndlessWar Notifier!\n\n\
📋 КОМАНДЫ:\n\
/screen - Скриншот игровых окон\n\
/myid - Узнать свой ID\n\
/start - Зарегистрироваться\n\n\
✅ Ты зарегистрирован!",
                                    first_name.unwrap_or_else(|| "друг".to_string())
                                );
                                if let Err(e) =
                                    bot.send_message(ChatId(chat_id), welcome).send().await
                                {
                                    tracing::error!("Failed to send welcome message: {}", e);
                                    let _ = app.emit(
                                        "bot-error",
                                        format!("Telegram send failed: {e}"),
                                    );
                                }
                                info!(
                                    "User registered: {} ({})",
                                    chat_id,
                                    username.unwrap_or_default()
                                );
                            } else if *text == "/myid" {
                                let msg = format!("🆔 Ваш Telegram ID: {}", chat_id);
                                if let Err(e) =
                                    bot.send_message(ChatId(chat_id), msg).send().await
                                {
                                    tracing::error!("Failed to send /myid: {}", e);
                                    let _ = app.emit(
                                        "bot-error",
                                        format!("Telegram send failed: {e}"),
                                    );
                                }
                            } else if *text == "/users" {
                                let config_guard = config.lock().await;
                                let admin = config_guard.telegram.admin_chat_id.trim().to_string();
                                let users = config_guard.telegram.registered_users.clone();
                                let notify_all = config_guard.telegram.notify_all_users;
                                drop(config_guard);

                                let chat_str = chat_id.to_string();
                                if admin.is_empty() || admin != chat_str {
                                    let _ = bot
                                        .send_message(
                                            ChatId(chat_id),
                                            "⛔ Команда /users только для администратора.\n\
Укажи admin_chat_id в настройках (свой ID из /myid).",
                                        )
                                        .send()
                                        .await;
                                } else {
                                    let mut user_list =
                                        String::from("📋 ЗАРЕГИСТРИРОВАННЫЕ ПОЛЬЗОВАТЕЛИ:\n\n");
                                    for user in &users {
                                        let status = if user.enabled { "✅" } else { "❌" };
                                        let sc = if user.allow_screenshot { " 📷" } else { "" };
                                        let name = user
                                            .first_name
                                            .clone()
                                            .unwrap_or_else(|| "Unknown".to_string());
                                        user_list.push_str(&format!(
                                            "{} {} (@{}){}\n",
                                            status,
                                            name,
                                            user.username.as_ref().unwrap_or(&"-".to_string()),
                                            sc
                                        ));
                                    }
                                    user_list.push_str(&format!(
                                        "\n📊 Рассылка всем: {}\n(📷 = разрешены скриншоты)",
                                        if notify_all {
                                            "включена"
                                        } else {
                                            "выключена"
                                        }
                                    ));
                                    let _ =
                                        bot.send_message(ChatId(chat_id), user_list).send().await;
                                }
                            } else if *text == "/screen" || *text == "/shot" || *text == "/screenshot" {
                                let config_guard = config.lock().await;
                                let chat_str = chat_id.to_string();
                                let allowed = crate::config::can_user_screenshot(&config_guard, &chat_str);
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                if !allowed {
                                    let _ = bot
                                        .send_message(
                                            ChatId(chat_id),
                                            "⛔ У вас нет прав на получение скриншотов.\n\
                                             Обратитесь к администратору для включения доступа.",
                                        )
                                        .send()
                                        .await;
                                } else {
                                    let windows = crate::screen_capture::find_target_windows(&process_name);
                                    if windows.is_empty() {
                                        let _ = bot
                                            .send_message(
                                                ChatId(chat_id),
                                                format!(
                                                    "⚠️ Окна процесса «{}» не найдены.\n\
                                                     Убедитесь, что игра запущена.",
                                                    process_name
                                                ),
                                            )
                                            .send()
                                            .await;
                                    } else {
                                        for (idx, win) in windows.iter().enumerate() {
                                            let win_num = idx + 1;
                                            let now_str = chrono::Local::now().format("%H:%M:%S").to_string();

                                            if win.is_minimized {
                                                let prompt = format!(
                                                    "⚠️ Окно #{win_num} «{}» (PID {}) свёрнуто.\n\
                                                     Принудительно развернуть для скриншота?",
                                                    win.title, win.pid
                                                );
                                                let btn = InlineKeyboardButton::callback(
                                                    "📷 Развернуть и сделать снимок",
                                                    format!("unmin_screen:{}", win.hwnd),
                                                );
                                                let keyboard = InlineKeyboardMarkup::default().append_row(vec![btn]);
                                                let _ = bot
                                                    .send_message(ChatId(chat_id), prompt)
                                                    .reply_markup(keyboard)
                                                    .send()
                                                    .await;
                                            } else {
                                                match crate::screen_capture::capture_window(win, false) {
                                                    Ok(jpeg) => {
                                                        let caption = format!(
                                                            "🎮 Окно #{win_num} «{}» [PID {}] | {}",
                                                            win.title, win.pid, now_str
                                                        );
                                                        let file = InputFile::memory(jpeg)
                                                            .file_name(format!("screen_{}.jpg", win.pid));
                                                        let _ = bot
                                                            .send_photo(ChatId(chat_id), file)
                                                            .caption(caption)
                                                            .send()
                                                            .await;
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to capture window {}: {}", win.hwnd, e);
                                                        let _ = bot
                                                            .send_message(
                                                                ChatId(chat_id),
                                                                format!("❌ Ошибка снимка окна #{win_num}: {e}"),
                                                            )
                                                            .send()
                                                            .await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if let UpdateKind::CallbackQuery(callback) = &update.kind {
                        if let (Some(data), Some(message)) = (&callback.data, &callback.message) {
                            if data.starts_with("unmin_screen:") {
                                let hwnd_str = &data["unmin_screen:".len()..];
                                let chat_id = message.chat().id.0;
                                let chat_str = chat_id.to_string();
                                let config_guard = config.lock().await;
                                let allowed = crate::config::can_user_screenshot(&config_guard, &chat_str);
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                if !allowed {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("⛔ Нет прав на скриншоты")
                                        .send()
                                        .await;
                                } else if let Ok(hwnd_val) = hwnd_str.parse::<isize>() {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("Разворачиваю окно и делаю снимок...")
                                        .send()
                                        .await;

                                    let windows = crate::screen_capture::find_target_windows(&process_name);
                                    if let Some(target_win) = windows.into_iter().find(|w| w.hwnd == hwnd_val) {
                                        let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
                                        match crate::screen_capture::capture_window(&target_win, true) {
                                            Ok(jpeg) => {
                                                let caption = format!(
                                                    "🎮 «{}» [PID {}] (развёрнуто программно) | {}",
                                                    target_win.title, target_win.pid, now_str
                                                );
                                                let file = InputFile::memory(jpeg)
                                                    .file_name(format!("screen_{}.jpg", target_win.pid));
                                                let _ = bot
                                                    .send_photo(ChatId(chat_id), file)
                                                    .caption(caption)
                                                    .send()
                                                    .await;
                                            }
                                            Err(e) => {
                                                let _ = bot
                                                    .send_message(
                                                        ChatId(chat_id),
                                                        format!("❌ Не удалось сделать снимок: {e}"),
                                                    )
                                                    .send()
                                                    .await;
                                            }
                                        }
                                    } else {
                                        let _ = bot
                                            .send_message(
                                                ChatId(chat_id),
                                                "⚠️ Окно больше не найдено (возможно, игра закрыта).",
                                            )
                                            .send()
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    offset = (update.id.0 as i32) + 1;
                }
            }
            Err(e) => {
                if is_terminated_by_other(&e) {
                    tracing::error!(
                        "Telegram getUpdates conflict (другой процесс с этим ботом). \
                         Оставляю паузу 30с — закрой второй экземпляр / BotFather webhook."
                    );
                    let _ = app.emit(
                        "bot-error",
                        "Конфликт Telegram: бот уже опрашивается другим процессом",
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    backoff_secs = 5;
                } else {
                    tracing::warn!("Error getting updates: {:?} (retry in {}s)", e, backoff_secs);
                    let _ = app.emit(
                        "bot-error",
                        format!("Telegram getUpdates: {e}"),
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(60);
                }
            }
        }
    }

    running.store(false, Ordering::SeqCst);
    info!("Telegram bot stopped");
    Ok(())
}


