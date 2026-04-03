pub mod config;
pub mod packet;
pub mod telegram;
pub mod tray;

use config::{AppConfig, Pattern};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio::sync::Mutex;
use tracing::info;

#[cfg(windows)]
use packet::{PacketAnalyzer, PacketCapture};

#[cfg(not(windows))]
use packet::PacketCapture;

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub capture: Arc<Mutex<Option<PacketCapture>>>,
    #[cfg(windows)]
    pub analyzer: Arc<Mutex<PacketAnalyzer>>,
}

#[tauri::command]
async fn start_monitoring(
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<String, String> {
    let config = state.config.lock().await;
    let process_name = config.target.process_name.clone();
    drop(config);

    info!("Starting monitoring for process: {}", process_name);

    let capture = PacketCapture::new(&process_name).map_err(|e| e.to_string())?;
    let mut capture_guard = state.capture.lock().await;
    *capture_guard = Some(capture);

    #[cfg(windows)]
    {
        let analyzer = state.analyzer.lock().await;
        let _patterns = analyzer.get_patterns().clone();
        let config = state.config.clone();
        let analyzer = state.analyzer.clone();

        tokio::spawn(async move {
            let mut capture_guard = config.lock().await;
            if let Some(ref mut capture) = *capture_guard {
                while let Ok(packet) = capture.read_packet() {
                    let pattern = analyzer.lock().await.analyze(&packet);
                    if let Some(pattern) = pattern {
                        info!("Pattern detected: {}", pattern.name);
                        let _ = telegram::send_notification(&config, &pattern.name, &pattern.description).await;
                        let _ = tray::show_notification(&config, &pattern.name, &pattern.description);
                        tokio::time::sleep(tokio::time::Duration::from_secs(
                            config.lock().await.notifications.cooldown_seconds,
                        ))
                        .await;
                    }
                }
            }
        });
    }

    #[cfg(not(windows))]
    {
        let config = state.config.clone();
        tokio::spawn(async move {
            info!("Linux stub: monitoring not available, but Telegram will work");
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });
    }

    Ok("Monitoring started".to_string())
}

#[tauri::command]
async fn stop_monitoring(state: State<'_, AppState>) -> Result<String, String> {
    info!("Stopping monitoring");
    let mut capture_guard = state.capture.lock().await;
    *capture_guard = None;
    Ok("Monitoring stopped".to_string())
}

#[tauri::command]
async fn add_pattern(
    state: State<'_, AppState>,
    name: String,
    bytes: String,
    description: String,
) -> Result<String, String> {
    info!("Adding pattern: {}", name);
    let _pattern = Pattern::new(name.clone(), bytes.clone(), description.clone())
        .map_err(|e| e.to_string())?;

    #[cfg(windows)]
    {
        state.analyzer.lock().await.add_pattern(_pattern);
    }

    let mut config = state.config.lock().await;
    config.target.patterns.push(Pattern::new(name, bytes, description).unwrap());
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern added".to_string())
}

#[tauri::command]
async fn remove_pattern(state: State<'_, AppState>, name: String) -> Result<String, String> {
    info!("Removing pattern: {}", name);

    #[cfg(windows)]
    {
        state.analyzer.lock().await.remove_pattern(&name);
    }

    let mut config = state.config.lock().await;
    config.target.patterns.retain(|p| p.name != name);
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern removed".to_string())
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    // Reload from disk to get latest changes (e.g., new users from bot)
    match config::load_config() {
        Ok(new_config) => {
            let mut config = state.config.lock().await;
            *config = new_config.clone();
            Ok(new_config)
        }
        Err(_) => {
            // Fallback to in-memory config
            let config = state.config.lock().await;
            Ok(config.clone())
        }
    }
}

#[tauri::command]
async fn update_config(state: State<'_, AppState>, new_config: AppConfig) -> Result<String, String> {
    info!("Updating config");
    let mut config = state.config.lock().await;
    *config = new_config.clone();
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok("Config updated".to_string())
}

#[tauri::command]
async fn get_patterns(state: State<'_, AppState>) -> Result<Vec<Pattern>, String> {
    let config = state.config.lock().await;
    Ok(config.target.patterns.clone())
}

#[tauri::command]
async fn test_telegram(token: String) -> Result<String, String> {
    telegram::test_connection(&token)
        .await
        .map_err(|e| e.to_string())?;
    Ok("Telegram connected successfully!".to_string())
}

#[tauri::command]
async fn start_telegram_bot(state: State<'_, AppState>, token: String) -> Result<String, String> {
    if token.is_empty() {
        return Err("Token is empty".to_string());
    }
    
    let config = state.config.clone();
    
    tokio::spawn(async move {
        if let Err(e) = telegram::start_bot(&token, config).await {
            tracing::error!("Failed to start telegram bot: {}", e);
        }
    });
    
    Ok("Telegram bot started!".to_string())
}

#[tauri::command]
async fn get_registered_users(state: State<'_, AppState>) -> Result<Vec<config::RegisteredUser>, String> {
    let config = state.config.lock().await;
    Ok(config.telegram.registered_users.clone())
}

#[tauri::command]
async fn set_user_enabled(state: State<'_, AppState>, chat_id: String, enabled: bool) -> Result<String, String> {
    let mut config = state.config.lock().await;
    config::set_user_enabled(&mut config, &chat_id, enabled);
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(format!("User {} {}", chat_id, if enabled { "enabled" } else { "disabled" }))
}

#[tauri::command]
async fn set_notify_all_users(state: State<'_, AppState>, enabled: bool) -> Result<String, String> {
    let mut config = state.config.lock().await;
    config.telegram.notify_all_users = enabled;
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(format!("Notify all users: {}", enabled))
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting EndlessWar Notifier");

    let config = config::load_config().unwrap_or_else(|_| {
        let default = AppConfig::default();
        let _ = config::save_config(&default);
        default
    });

    let app_state = AppState {
        config: Arc::new(Mutex::new(config.clone())),
        capture: Arc::new(Mutex::new(None)),
        #[cfg(windows)]
        analyzer: Arc::new(Mutex::new(PacketAnalyzer::new(Vec::new()))),
    };

    let auto_start_token = if config.telegram.auto_start && !config.telegram.bot_token.is_empty() {
        Some(config.telegram.bot_token.clone())
    } else {
        None
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            let start = MenuItem::with_id(app, "start", "Старт мониторинга", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, "stop", "Стоп мониторинга", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Показать", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&start, &stop, &show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "start" => {
                            let _ = app.emit("tray-start", ());
                        }
                        "stop" => {
                            let _ = app.emit("tray-stop", ());
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            if let Some(token) = auto_start_token.clone() {
                info!("Auto-starting Telegram bot...");
                let config = Arc::new(Mutex::new(config.clone()));
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async move {
                        if let Err(e) = telegram::start_bot(&token, config).await {
                            tracing::error!("Failed to start telegram bot: {}", e);
                        }
                    });
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_monitoring,
            stop_monitoring,
            add_pattern,
            remove_pattern,
            get_config,
            update_config,
            get_patterns,
            test_telegram,
            start_telegram_bot,
            get_registered_users,
            set_user_enabled,
            set_notify_all_users,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
