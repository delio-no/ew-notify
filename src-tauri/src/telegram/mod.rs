use crate::config::{AppConfig, RegisteredUser};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::payloads::{EditMessageMediaSetters, SendPhotoSetters};
use teloxide::prelude::{Request, Requester};
use teloxide::types::{
    BotCommand, ChatId, InlineKeyboardButton, WebAppInfo, InlineKeyboardMarkup, InputFile, InputMedia,
    InputMediaPhoto, KeyboardButton, KeyboardMarkup,
};

const PLACEHOLDER_JPEG: &[u8] = include_bytes!("../../icons/placeholder.jpg");
use tokio::sync::Mutex;
use tracing::info;

pub async fn send_notification(
    config: &Arc<Mutex<AppConfig>>,
    event_title: &str,
    pattern_id: &str,
    window_label: Option<&str>,
    target_win: Option<crate::screen_capture::TargetWindow>,
) -> anyhow::Result<()> {
    tracing::info!("telegram::send_notification started for '{}' (target_win: {})", event_title, target_win.is_some());
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
    let delay_ms = config_guard.notifications.screenshot_delay_ms;

    if token.is_empty() {
        info!("Telegram notification skipped: no token");
        return Ok(());
    }

    drop(config_guard);

    let bot = teloxide::Bot::new(&token);
    let message = match window_label {
        Some(label) => {
            if pattern_id.is_empty() || pattern_id == event_title {
                format!("🔔 {event_title}\n\n{label}")
            } else {
                format!("🔔 {event_title}\n{pattern_id}\n\n{label}")
            }
        }
        None => {
            if pattern_id.is_empty() || pattern_id == event_title {
                format!("🔔 {event_title}")
            } else {
                format!("🔔 {event_title}\n\n{pattern_id}")
            }
        }
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
    let mut sent_photo_alerts: Vec<(i64, teloxide::types::MessageId)> = Vec::new();

    // If we have a target window to screenshot, start with placeholder photo so we can editMessageMedia in-place!
    if target_win.is_some() {
        let init_caption = format!("{message}\n\n⏳ Захват кадра игры...");
        for user in &enabled_users {
            let chat_id_num: i64 = user.chat_id.parse().unwrap_or(0);
            if chat_id_num == 0 {
                continue;
            }

            let file = InputFile::memory(PLACEHOLDER_JPEG.to_vec()).file_name("loading.jpg");
            match bot
                .send_photo(ChatId(chat_id_num), file)
                .caption(&init_caption)
                .send()
                .await
            {
                Ok(msg) => {
                    sent += 1;
                    sent_photo_alerts.push((chat_id_num, msg.id));
                }
                Err(e) => {
                    tracing::warn!("Telegram initial photo send to {} failed: {}", user.chat_id, e);
                    last_err = Some(e.to_string());
                }
            }
        }
    } else {
        // Plain text message (no screenshot expected, e.g. pattern with screenshot_enabled=false)
        for user in &enabled_users {
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
    }

    if sent == 0 {
        return Err(anyhow::anyhow!(
            last_err.unwrap_or_else(|| "не удалось отправить ни одному получателю".into())
        ));
    }

    info!("Telegram notification sent to {} user(s): {}", sent, event_title);

    // If there is a target window, capture screenshot with delay and EDIT the photo in-place!
    if let Some(win) = target_win {
        let bot_edit = bot.clone();
        let final_caption = message.clone();
        tauri::async_runtime::spawn(async move {
            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }

            match crate::screen_capture::capture_window(&win, true) {
                Ok(photo_bytes) => {
                    let media = InputMedia::Photo(
                        InputMediaPhoto::new(InputFile::memory(photo_bytes).file_name("screen.jpg"))
                            .caption(&final_caption),
                    );

                    for (chat_id, msg_id) in sent_photo_alerts {
                        if let Err(e) = bot_edit
                            .edit_message_media(ChatId(chat_id), msg_id, media.clone())
                            .send()
                            .await
                        {
                            tracing::warn!("Failed to editMessageMedia for chat {}: {}", chat_id, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Delayed window capture failed for PID {}: {}", win.pid, e);
                    if win.is_minimized {
                        let btn = InlineKeyboardButton::callback(
                            "📸 Развернуть и сделать снимок",
                            format!("unmin_screen:{}", win.hwnd),
                        );
                        let keyboard = InlineKeyboardMarkup::default().append_row(vec![btn]);
                        let caption_fail = format!("{final_caption}\n\n⚠️ Окно свёрнуто. Нажмите кнопку:");
                        let media = InputMedia::Photo(
                            InputMediaPhoto::new(
                                InputFile::memory(PLACEHOLDER_JPEG.to_vec()).file_name("loading.jpg"),
                            )
                            .caption(&caption_fail),
                        );

                        for (chat_id, msg_id) in sent_photo_alerts {
                            let _ = bot_edit
                                .edit_message_media(ChatId(chat_id), msg_id, media.clone())
                                .reply_markup(keyboard.clone())
                                .send()
                                .await;
                        }
                    }
                }
            }
        });
    }

    Ok(())
}

pub fn make_user_keyboard(is_admin: bool) -> KeyboardMarkup {
    let mut rows = vec![
        vec![
            KeyboardButton::new("📸 Скриншот"),
            KeyboardButton::new("📺 Стрим"),
        ],
        vec![
            KeyboardButton::new("⚙️ Настройки"),
            KeyboardButton::new("🆔 Мой ID"),
        ],
    ];
    if is_admin {
        rows[1].push(KeyboardButton::new("👥 Пользователи"));
    }
    KeyboardMarkup::new(rows).resize_keyboard().persistent()
}

pub async fn test_connection(token: &str) -> anyhow::Result<()> {
    let bot = teloxide::Bot::new(token);

    let commands = vec![
        BotCommand {
            command: "screen".to_string(),
            description: "Скриншот игровых окон".to_string(),
        },
        BotCommand {
            command: "stream".to_string(),
            description: "P2P-стриминг игровых окон (1080p)".to_string(),
        },
        BotCommand {
            command: "stream".to_string(),
            description: "P2P-стриминг игровых окон (1080p)".to_string(),
        },
        BotCommand {
            command: "settings".to_string(),
            description: "Настройки (скриншот при смерти)".to_string(),
        },
        BotCommand {
            command: "myid".to_string(),
            description: "Узнать свой Telegram ID".to_string(),
        },
        BotCommand {
            command: "start".to_string(),
            description: "Главное меню / клавиатура".to_string(),
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
        AnswerCallbackQuerySetters, EditMessageTextSetters, GetUpdatesSetters, SendMessageSetters,
        SendPhotoSetters,
    };
    use teloxide::types::UpdateKind;

    let bot = teloxide::Bot::new(token);

    let commands = vec![
        BotCommand {
            command: "screen".to_string(),
            description: "Скриншот игровых окон".to_string(),
        },
        BotCommand {
            command: "settings".to_string(),
            description: "Настройки (скриншот при смерти)".to_string(),
        },
        BotCommand {
            command: "myid".to_string(),
            description: "Узнать свой Telegram ID".to_string(),
        },
        BotCommand {
            command: "start".to_string(),
            description: "Главное меню / клавиатура".to_string(),
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
                            let chat_str = chat_id.to_string();
                            let is_admin = {
                                let config_guard = config.lock().await;
                                let admin = config_guard.telegram.admin_chat_id.trim().to_string();
                                !admin.is_empty() && admin == chat_str
                            };
                            let text_cmd = text.trim();
                            let username = message.chat.username().map(|s| s.to_string());
                            let first_name = message.chat.first_name().map(|s| s.to_string());

                            if text_cmd == "/start" {
                                let new_user = RegisteredUser {
                                    chat_id: chat_id.to_string(),
                                    username: username.clone(),
                                    first_name: first_name.clone(),
                                    enabled: true,
                                    allow_screenshot: false,
                                    allow_stream: false,
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
📋 Кнопки быстрого доступа доступны на клавиатуре ниже.\n\n\
✅ Ты зарегистрирован!",
                                    first_name.unwrap_or_else(|| "друг".to_string())
                                );
                                if let Err(e) = bot
                                    .send_message(ChatId(chat_id), welcome)
                                    .reply_markup(make_user_keyboard(is_admin))
                                    .send()
                                    .await
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
                            } else if text_cmd == "/myid" || text_cmd == "🆔 Мой ID" {
                                let msg = format!("🆔 Ваш Telegram ID: {}", chat_id);
                                if let Err(e) = bot
                                    .send_message(ChatId(chat_id), msg)
                                    .reply_markup(make_user_keyboard(is_admin))
                                    .send()
                                    .await
                                {
                                    tracing::error!("Failed to send /myid: {}", e);
                                    let _ = app.emit(
                                        "bot-error",
                                        format!("Telegram send failed: {e}"),
                                    );
                                }
                            } else if text_cmd == "/settings" || text_cmd == "⚙️ Настройки" {
                                let config_guard = config.lock().await;
                                let on_death = config_guard
                                    .target
                                    .patterns
                                    .iter()
                                    .find(|p| p.name == "DEATH")
                                    .map(|p| p.screenshot_enabled)
                                    .unwrap_or(true);
                                let delay = config_guard.notifications.screenshot_delay_ms;
                                drop(config_guard);

                                let text_msg = format!(
                                    "⚙️ Настройки уведомлений:\n\n\
• 💀 Скриншот при смерти (DEATH): {}\n\
• ⏱ Задержка скриншота: {} мс\n\n\
Нажмите кнопку ниже для переключения:",
                                    if on_death { "Включён ✅" } else { "Выключен ❌" },
                                    delay
                                );
                                let btn_text = if on_death {
                                    "💀 Скрин при смерти: Вкл ✅"
                                } else {
                                    "💀 Скрин при смерти: Выкл ❌"
                                };
                                let kb = InlineKeyboardMarkup::default().append_row(vec![
                                    InlineKeyboardButton::callback(btn_text, "toggle_death_screen"),
                                ]);
                                let _ = bot
                                    .send_message(ChatId(chat_id), text_msg)
                                    .reply_markup(kb)
                                    .send()
                                    .await;
                            } else if text_cmd == "/users" || text_cmd == "👥 Пользователи" {
                                let config_guard = config.lock().await;
                                let users = config_guard.telegram.registered_users.clone();
                                let notify_all = config_guard.telegram.notify_all_users;
                                drop(config_guard);

                                if !is_admin {
                                    let _ = bot
                                        .send_message(
                                            ChatId(chat_id),
                                            "⛔ Команда /users только для администратора.\n\
Укажи admin_chat_id в настройках (свой ID из /myid).",
                                        )
                                        .reply_markup(make_user_keyboard(is_admin))
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
                                    let _ = bot
                                        .send_message(ChatId(chat_id), user_list)
                                        .reply_markup(make_user_keyboard(is_admin))
                                        .send()
                                        .await;
                                }
                            } else if text_cmd == "/stream" || text_cmd == "📺 Стрим" {
                                let config_guard = config.lock().await;
                                let chat_str = chat_id.to_string();
                                let allowed = crate::config::can_user_stream(&config_guard, &chat_str);
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                if !allowed {
                                    let _ = bot
                                        .send_message(
                                            ChatId(chat_id),
                                            "⛔ У вас нет доступа к функции стриминга.\n\
                                             Обратитесь к администратору для включения доступа.",
                                        )
                                        .reply_markup(make_user_keyboard(is_admin))
                                        .send()
                                        .await;
                                } else if let Some(active) = crate::streamer::get_stream_info().await {
                                    let info_msg = format!(
                                        "📺 Сейчас активна трансляция:\n\n\
                                         🎮 Окно: «{}» (PID {})\n\
                                         👁 Зрителей: {}\n\
                                         ⏱ Время в эфире: {} сек\n\n\
                                         🔗 Ссылка на стрим:\n{}\n\n\
                                         Вы можете открыть плеер или остановить вещание:",
                                        active.title, active.pid, active.viewer_count, active.uptime_secs, active.player_url
                                    );
                                    let mut kb = InlineKeyboardMarkup::default();
                                    if let Ok(url) = active.player_url.parse::<reqwest::Url>() {
                                        kb = kb.append_row(vec![InlineKeyboardButton::web_app("▶️ Смотреть в Telegram", WebAppInfo { url: url.clone() })]);
                                        kb = kb.append_row(vec![InlineKeyboardButton::url("🌐 Открыть в браузере", url)]);
                                    }
                                    kb = kb.append_row(vec![
                                        InlineKeyboardButton::callback("🔄 Обновить", "stream_refresh"),
                                        InlineKeyboardButton::callback("🛑 Остановить стрим", "stream_stop"),
                                    ]);
                                    let _ = bot
                                        .send_message(ChatId(chat_id), info_msg)
                                        .reply_markup(kb)
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
                                        let mut kb = InlineKeyboardMarkup::default();
                                        for (idx, win) in windows.iter().enumerate() {
                                            let win_num = idx + 1;
                                            let min_mark = if win.is_minimized { " [Свёрнуто ⚠️]" } else { "" };
                                            let label = format!("🎮 Окно #{} (PID {}){}", win_num, win.pid, min_mark);
                                            kb = kb.append_row(vec![
                                                InlineKeyboardButton::callback(label, format!("stream_pick_{}", win.hwnd)),
                                            ]);
                                        }
                                        let prompt = "📺 P2P-стриминг игровых окон (1080p, 30 FPS)\n\n                                                      Видео транслируется напрямую на любое устройство без серверов.\n                                                      Выберите окно для начала стрима:";
                                        let _ = bot
                                            .send_message(ChatId(chat_id), prompt)
                                            .reply_markup(kb)
                                            .send()
                                            .await;
                                    }
                                }
                            } else if text_cmd == "/screen" || text_cmd == "/shot" || text_cmd == "/screenshot" || text_cmd == "📸 Скриншот" {
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
                            } else if data.starts_with("stream_pick_") {
                                let hwnd_str = &data["stream_pick_".len()..];
                                let chat_id = message.chat().id.0;
                                let chat_str = chat_id.to_string();
                                let config_guard = config.lock().await;
                                let allowed = crate::config::can_user_stream(&config_guard, &chat_str);
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                if !allowed {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("⛔ Нет прав на запуск стрима")
                                        .send()
                                        .await;
                                } else if let Ok(hwnd_val) = hwnd_str.parse::<isize>() {
                                    let windows = crate::screen_capture::find_target_windows(&process_name);
                                    if let Some(target_win) = windows.into_iter().find(|w| w.hwnd == hwnd_val) {
                                        if target_win.is_minimized {
                                            let warn_msg = format!(
                                                "⚠️ Окно «{}» (PID {}) свёрнуто!\n\n                                                 В свёрнутом состоянии Windows (DWM) приостанавливает отрисовку кадров, поэтому трансляция будет чёрным экраном.\n\n                                                 Развернуть окно без перехвата фокуса и начать стрим?",
                                                target_win.title, target_win.pid
                                            );
                                            let kb = InlineKeyboardMarkup::default()
                                                .append_row(vec![
                                                    InlineKeyboardButton::callback("✅ Развернуть и начать", format!("stream_start_force_{}", target_win.hwnd)),
                                                ])
                                                .append_row(vec![
                                                    InlineKeyboardButton::callback("❌ Отмена", "stream_cancel"),
                                                ]);
                                            let _ = bot
                                                .edit_message_text(message.chat().id, message.id(), warn_msg)
                                                .reply_markup(kb)
                                                .send()
                                                .await;
                                        } else {
                                            let _ = bot
                                                .answer_callback_query(callback.id.clone())
                                                .text("Запуск трансляции 720p...")
                                                .send()
                                                .await;
                                            match crate::streamer::start_stream(target_win.hwnd, target_win.pid, target_win.title.clone()).await {
                                                Ok(info) => {
                                                    let success_msg = format!(
                                                        "📺 Трансляция окна запущена!\n\n🎮 Окно: «{}» (PID {})\n🌐 Качество: 720p HD, 30 FPS\n🔗 Ссылка для просмотра:\n{}\n\n👁 Зрителей сейчас: 0\nОткройте прямо в Telegram (Mini App) или в браузере:",
                                                        info.title, info.pid, info.player_url
                                                    );
                                                    let mut kb = InlineKeyboardMarkup::default();
                                                    if let Ok(url) = info.player_url.parse::<reqwest::Url>() {
                                                        kb = kb.append_row(vec![InlineKeyboardButton::web_app("▶️ Смотреть в Telegram", WebAppInfo { url: url.clone() })]);
                                                        kb = kb.append_row(vec![InlineKeyboardButton::url("🌐 Открыть в браузере", url)]);
                                                    }
                                                    kb = kb.append_row(vec![
                                                        InlineKeyboardButton::callback("🔄 Обновить", "stream_refresh"),
                                                        InlineKeyboardButton::callback("🛑 Остановить стрим", "stream_stop"),
                                                    ]);
                                                    let _ = bot
                                                        .edit_message_text(message.chat().id, message.id(), success_msg)
                                                        .reply_markup(kb)
                                                        .send()
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = bot
                                                        .edit_message_text(message.chat().id, message.id(), format!("❌ Ошибка запуска стрима: {e}"))
                                                        .send()
                                                        .await;
                                                }
                                            }
                                        }
                                    } else {
                                        let _ = bot
                                            .edit_message_text(message.chat().id, message.id(), "⚠️ Окно больше не найдено (возможно, игра закрыта).")
                                            .send()
                                            .await;
                                    }
                                }
                            } else if data.starts_with("stream_start_force_") {
                                let hwnd_str = &data["stream_start_force_".len()..];
                                let chat_id = message.chat().id.0;
                                let chat_str = chat_id.to_string();
                                let config_guard = config.lock().await;
                                let allowed = crate::config::can_user_stream(&config_guard, &chat_str);
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                if !allowed {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("⛔ Нет прав на запуск стрима")
                                        .send()
                                        .await;
                                } else if let Ok(hwnd_val) = hwnd_str.parse::<isize>() {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("Разворачиваю окно и запускаю 720p...")
                                        .send()
                                        .await;

                                    #[cfg(windows)]
                                    unsafe {
                                        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SHOW_WINDOW_CMD, SW_SHOWNOACTIVATE};
                                        ShowWindow(
                                            windows::Win32::Foundation::HWND(hwnd_val),
                                            SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0 as u32),
                                        );
                                    }
                                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

                                    let windows = crate::screen_capture::find_target_windows(&process_name);
                                    if let Some(target_win) = windows.into_iter().find(|w| w.hwnd == hwnd_val) {
                                        match crate::streamer::start_stream(target_win.hwnd, target_win.pid, target_win.title.clone()).await {
                                            Ok(info) => {
                                                let success_msg = format!(
                                                    "📺 Трансляция окна запущена (развёрнуто программно)!\n\n🎮 Окно: «{}» (PID {})\n🌐 Качество: 720p HD, 30 FPS\n🔗 Ссылка для просмотра:\n{}\n\n👁 Зрителей сейчас: 0\nОткройте прямо в Telegram (Mini App) или в браузере:",
                                                    info.title, info.pid, info.player_url
                                                );
                                                let mut kb = InlineKeyboardMarkup::default();
                                                if let Ok(url) = info.player_url.parse::<reqwest::Url>() {
                                                    kb = kb.append_row(vec![InlineKeyboardButton::web_app("▶️ Смотреть в Telegram", WebAppInfo { url: url.clone() })]);
                                                    kb = kb.append_row(vec![InlineKeyboardButton::url("🌐 Открыть в браузере", url)]);
                                                }
                                                kb = kb.append_row(vec![
                                                    InlineKeyboardButton::callback("🔄 Обновить", "stream_refresh"),
                                                    InlineKeyboardButton::callback("🛑 Остановить стрим", "stream_stop"),
                                                ]);
                                                let _ = bot
                                                    .edit_message_text(message.chat().id, message.id(), success_msg)
                                                    .reply_markup(kb)
                                                    .send()
                                                    .await;
                                            }
                                            Err(e) => {
                                                let _ = bot
                                                    .edit_message_text(message.chat().id, message.id(), format!("❌ Ошибка запуска стрима: {e}"))
                                                    .send()
                                                    .await;
                                            }
                                        }
                                    } else {
                                        let _ = bot
                                            .edit_message_text(message.chat().id, message.id(), "⚠️ Окно больше не найдено.")
                                            .send()
                                            .await;
                                    }
                                }
                            } else if data == "stream_stop" {
                                crate::streamer::stop_stream().await;
                                let _ = bot
                                    .answer_callback_query(callback.id.clone())
                                    .text("🛑 Трансляция остановлена")
                                    .send()
                                    .await;

                                let kb = InlineKeyboardMarkup::default().append_row(vec![
                                    InlineKeyboardButton::callback("📺 Запустить стрим заново", "stream_restart"),
                                ]);
                                let _ = bot
                                    .edit_message_text(message.chat().id, message.id(), "🛑 Трансляция окна остановлена.")
                                    .reply_markup(kb)
                                    .send()
                                    .await;
                            } else if data == "stream_refresh" {
                                if let Some(active) = crate::streamer::get_stream_info().await {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("Статус обновлен")
                                        .send()
                                        .await;

                                    let info_msg = format!(
                                        "📺 Сейчас активна трансляция:\n\n🎮 Окно: «{}» (PID {})\n🌐 Разрешение: 720p HD, 30 FPS\n👁 Зрителей: {}\n⏱ В эфире: {} сек\n\n🔗 Ссылка на стрим:\n{}\n\nОткройте плеер прямо в Telegram или в браузере:",
                                        active.title, active.pid, active.viewer_count, active.uptime_secs, active.player_url
                                    );
                                    let mut kb = InlineKeyboardMarkup::default();
                                    if let Ok(url) = active.player_url.parse::<reqwest::Url>() {
                                        kb = kb.append_row(vec![InlineKeyboardButton::web_app("▶️ Смотреть в Telegram", WebAppInfo { url: url.clone() })]);
                                        kb = kb.append_row(vec![InlineKeyboardButton::url("🌐 Открыть в браузере", url)]);
                                    }
                                    kb = kb.append_row(vec![
                                        InlineKeyboardButton::callback("🔄 Обновить", "stream_refresh"),
                                        InlineKeyboardButton::callback("🛑 Остановить стрим", "stream_stop"),
                                    ]);
                                    let _ = bot
                                        .edit_message_text(message.chat().id, message.id(), info_msg)
                                        .reply_markup(kb)
                                        .send()
                                        .await;
                                } else {
                                    let _ = bot
                                        .answer_callback_query(callback.id.clone())
                                        .text("Трансляция не активна")
                                        .send()
                                        .await;

                                    let kb = InlineKeyboardMarkup::default().append_row(vec![
                                        InlineKeyboardButton::callback("📺 Запустить стрим заново", "stream_restart"),
                                    ]);
                                    let _ = bot
                                        .edit_message_text(message.chat().id, message.id(), "🛑 Трансляция не активна или была остановлена.")
                                        .reply_markup(kb)
                                        .send()
                                        .await;
                                }
                            } else if data == "stream_restart" {
                                let _ = bot
                                    .answer_callback_query(callback.id.clone())
                                    .send()
                                    .await;

                                let config_guard = config.lock().await;
                                let process_name = config_guard.target.process_name.clone();
                                drop(config_guard);

                                let windows = crate::screen_capture::find_target_windows(&process_name);
                                if windows.is_empty() {
                                    let kb = InlineKeyboardMarkup::default().append_row(vec![
                                        InlineKeyboardButton::callback("🔄 Повторить поиск", "stream_restart"),
                                    ]);
                                    let _ = bot
                                        .edit_message_text(
                                            message.chat().id,
                                            message.id(),
                                            format!(
                                                "⚠️ Окна процесса «{}» не найдены.\nУбедитесь, что игра запущена на ПК.",
                                                process_name
                                            ),
                                        )
                                        .reply_markup(kb)
                                        .send()
                                        .await;
                                } else {
                                    let mut kb = InlineKeyboardMarkup::default();
                                    for (idx, win) in windows.iter().enumerate() {
                                        let win_num = idx + 1;
                                        let min_mark = if win.is_minimized { " [Свёрнуто ⚠️]" } else { "" };
                                        let label = format!("🎮 Окно #{} (PID {}){}", win_num, win.pid, min_mark);
                                        kb = kb.append_row(vec![
                                            InlineKeyboardButton::callback(label, format!("stream_pick_{}", win.hwnd)),
                                        ]);
                                    }
                                    kb = kb.append_row(vec![
                                        InlineKeyboardButton::callback("❌ Отмена", "stream_cancel"),
                                    ]);
                                    let prompt = "📺 Выберите окно игры для начала стрима:";
                                    let _ = bot
                                        .edit_message_text(message.chat().id, message.id(), prompt)
                                        .reply_markup(kb)
                                        .send()
                                        .await;
                                }
                            } else if data == "stream_cancel" {
                                let _ = bot
                                    .answer_callback_query(callback.id.clone())
                                    .text("Отменено")
                                    .send()
                                    .await;
                                let _ = bot
                                    .edit_message_text(message.chat().id, message.id(), "❌ Запуск трансляции отменён.")
                                    .send()
                                    .await;
                            } else if data == "toggle_death_screen" {
                                let (new_val, delay) = {
                                    let mut config_guard = config.lock().await;
                                    let mut v = true;
                                    if let Some(p) = config_guard
                                        .target
                                        .patterns
                                        .iter_mut()
                                        .find(|p| p.name == "DEATH")
                                    {
                                        p.screenshot_enabled = !p.screenshot_enabled;
                                        v = p.screenshot_enabled;
                                    }
                                    let d = config_guard.notifications.screenshot_delay_ms;
                                    let _ = crate::config::save_config(&config_guard);
                                    (v, d)
                                };
                                let _ = app.emit("config-updated", ());

                                let status_toast = if new_val {
                                    "Скриншот при смерти включён ✅"
                                } else {
                                    "Скриншот при смерти выключен ❌"
                                };
                                let _ = bot
                                    .answer_callback_query(callback.id.clone())
                                    .text(status_toast)
                                    .send()
                                    .await;

                                let btn_text = if new_val {
                                    "💀 Скрин при смерти: Вкл ✅"
                                } else {
                                    "💀 Скрин при смерти: Выкл ❌"
                                };
                                let new_text = format!(
                                    "⚙️ Настройки бота:\n\n\
• 💀 Скриншот при смерти (DEATH): {}\n\
• ⏱ Задержка скриншота: {} мс\n\n\
Нажмите кнопку ниже, чтобы переключить:",
                                    if new_val { "Включён ✅" } else { "Выключен ❌" },
                                    delay
                                );
                                let kb = InlineKeyboardMarkup::default().append_row(vec![
                                    InlineKeyboardButton::callback(btn_text, "toggle_death_screen"),
                                ]);
                                let _ = bot
                                    .edit_message_text(message.chat().id, message.id(), new_text)
                                    .reply_markup(kb)
                                    .send()
                                    .await;
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


