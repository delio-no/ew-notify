pub mod config;
pub mod packet;
pub mod telegram;
pub mod tray;

use anyhow::Result;
use config::{AppConfig, Pattern};
use packet::{PacketAnalyzer, PacketCapture};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder,
};
use tokio::sync::Mutex;
use tracing::{error, info};

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub capture: Arc<Mutex<Option<PacketCapture>>>,
    pub analyzer: Arc<Mutex<PacketAnalyzer>>,
}

#[tauri::command]
async fn start_monitoring(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let config = state.config.lock().await;
    let process_name = config.target.process_name.clone();
    drop(config);

    info!("Starting monitoring for process: {}", process_name);

    let capture = PacketCapture::new(&process_name).map_err(|e| e.to_string())?;
    let mut capture_guard = state.capture.lock().await;
    *capture_guard = Some(capture);

    let analyzer = state.analyzer.lock().await;
    let patterns = analyzer.get_patterns().clone();

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
                    let _ = tray::show_notification(&app, &pattern.name, &pattern.description);
                    tokio::time::sleep(tokio::time::Duration::from_secs(
                        config.lock().await.notifications.cooldown_seconds,
                    ))
                    .await;
                }
            }
        }
    });

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
    let pattern = Pattern::new(name.clone(), bytes.clone(), description.clone())
        .map_err(|e| e.to_string())?;
    state.analyzer.lock().await.add_pattern(pattern);

    let mut config = state.config.lock().await;
    config.target.patterns.push(Pattern::new(name, bytes, description).unwrap());
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern added".to_string())
}

#[tauri::command]
async fn remove_pattern(state: State<'_, AppState>, name: String) -> Result<String, String> {
    info!("Removing pattern: {}", name);
    state.analyzer.lock().await.remove_pattern(&name);

    let mut config = state.config.lock().await;
    config.target.patterns.retain(|p| p.name != name);
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern removed".to_string())
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
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
    let analyzer = state.analyzer.lock().await;
    Ok(analyzer.get_patterns().clone())
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

    let analyzer = PacketAnalyzer::new(config.target.patterns.clone());

    let app_state = AppState {
        config: Arc::new(Mutex::new(config)),
        capture: Arc::new(Mutex::new(None)),
        analyzer: Arc::new(Mutex::new(analyzer)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}