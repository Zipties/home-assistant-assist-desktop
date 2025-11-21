// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use opener::open_browser;
use serde::{Deserialize, Serialize};
use std::fs::File;
use tauri::GlobalShortcutManager;
use tauri::Manager;
use tauri::SystemTray;
use tauri::{CustomMenuItem, SystemTrayMenu, SystemTrayMenuItem};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::LogTarget;
use url::Url;

// No longer needed - imports moved into the setup closure

// Define settings
#[derive(Serialize, Deserialize)]
struct HomeAssistantSettings {
    access_token: String,
    host: String,
    port: u16,
    ssl: bool,
}

#[derive(Serialize, Deserialize)]
struct TraySettings {
    double_click_action: String,
}

#[derive(Serialize, Deserialize)]
struct Settings {
    autostart: bool,
    home_assistant: HomeAssistantSettings,
    tray: Option<TraySettings>,
}

#[derive(Debug, Serialize)]
struct CommandError {
    message: String,
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        CommandError {
            message: error.to_string(),
        }
    }
}

fn show_window_app(window: tauri::Window) {
    log::info!("Showing window...");
    let url = window.url().to_string();
    if url.contains("settings") {
        open_app(window);
        return;
    }
    window.show().expect("failed to show the window");
    window.set_focus().expect("failed to focus the window");
    window
        .emit("focus", {})
        .expect("failed to emit focus event");
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn open_app(window: tauri::Window) {
    println!("Opening app...");

    let current_url: String = window.url().to_string();
    let mut url = Url::parse(&current_url).expect("failed to parse URL");

    window.show().expect("failed to show the window");
    window.set_focus().expect("failed to focus the window");

    url.set_path("/");
    println!("Navigating to {}", url);

    window
        .eval(&format!("window.location.href = '{}';", url))
        .unwrap();
}

#[tauri::command]
fn open_settings(window: tauri::Window) {
    println!("Opening settings...");

    let current_url: String = window.url().to_string();
    let mut url = Url::parse(&current_url).expect("failed to parse URL");

    window.show().expect("failed to show the window");
    window.set_focus().expect("failed to focus the window");

    url.set_path("/settings");
    println!("Navigating to {}", url);

    window
        .eval(&format!("window.location.href = '{}';", url))
        .unwrap();
}

#[tauri::command]
fn load_settings(app_handle: tauri::AppHandle) -> Result<Settings, CommandError> {
    let settings_path: String = app_handle
        .path_resolver()
        .app_config_dir()
        .unwrap()
        .join("settings.json")
        .to_str()
        .unwrap()
        .to_string();

    println!("Loading settings from {}...", settings_path);

    // If the directory doesn't exist, create it.
    if !std::path::Path::new(&settings_path)
        .parent()
        .unwrap()
        .exists()
    {
        std::fs::create_dir_all(std::path::Path::new(&settings_path).parent().unwrap()).unwrap();
    }

    // Convert the settings path to a Path.
    let path: &std::path::Path = std::path::Path::new(&settings_path);

    // Check if the file exists.
    if !path.exists() {
        // Create the file if it doesn't exist.
        let file: File = File::create(path).unwrap();
        // Create a new Settings struct.
        let settings: Settings = Settings {
            autostart: false,
            home_assistant: HomeAssistantSettings {
                access_token: "".to_string(),
                host: "homeassistant.local".to_string(),
                port: 8123,
                ssl: false,
            },
            tray: Some(TraySettings {
                double_click_action: "toggle_window".to_string(),
            }),
        };
        // Serialize the Settings struct into JSON.
        serde_json::to_writer_pretty(file, &settings).unwrap();
    }
    // Open the file in read-only mode.
    let file: File = File::open(path).unwrap();
    // Read the JSON contents of the file as an instance of `Settings`.
    let mut settings: Settings = serde_json::from_reader(file)?;

    if settings.tray.is_none() {
        settings.tray = Some(TraySettings {
            double_click_action: "toggle_window".to_string(),
        });
    }

    Ok(settings)
}

#[tauri::command]
fn update_settings(app_handle: tauri::AppHandle, settings: Settings) -> Result<(), CommandError> {
    let settings_path: String = app_handle
        .path_resolver()
        .app_config_dir()
        .unwrap()
        .join("settings.json")
        .to_str()
        .unwrap()
        .to_string();

    println!("Updating settings at {}...", settings_path);

    // Open the file in write-only mode.
    let file: File = File::create(settings_path).unwrap();
    // Serialize the Settings struct into JSON.
    serde_json::to_writer_pretty(file, &settings).unwrap();

    Ok(())
}

#[tauri::command]
fn toggle_window(window: tauri::Window) {
    let window_visible = window
        .is_visible()
        .expect("failed to check if the window is visible");
    println!("Window visible: {}", window_visible);
    if window_visible {
        println!("Hiding window...");
        window.hide().expect("failed to hide the window");
    } else {
        show_window_app(window.clone());
    }
}

#[tauri::command]
fn trigger_voice_pipeline(window: tauri::Window) {
    if !window
        .is_visible()
        .expect("failed to check if the window is visible")
    {
        show_window_app(window.clone());
    }

    log::info!("Triggering voice pipeline...");
    window
        .emit("trigger-voice-pipeline", {})
        .expect("failed to emit trigger-voice-pipeline event");
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    window.hide().expect("failed to hide the window");
}

#[tauri::command]
fn open_logs_directory(app_handle: tauri::AppHandle) {
    let path: String = app_handle
        .path_resolver()
        .app_log_dir()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    println!("Opening logs directory at {}...", path);

    // Open file with default application
    opener::open(path).unwrap();
}

#[tauri::command]
fn quit_application(window: tauri::Window) {
    window.close().expect("failed to close the window");
    std::process::exit(0);
}

fn main() {
    // Linux/Wayland: Fix for audio device access crash (Error 71 - Protocol error)
    // CRITICAL: These environment variables MUST be set before GTK/webkit initialization
    //
    // Root cause: On Wayland + KDE Plasma + webkit2gtk, getUserMedia() was causing
    // "Error 71 (Protocol error) dispatching to Wayland display" and hard crashing.
    //
    // Solution: Disable webkit's hardware compositing and DMA-BUF rendering, which
    // cause Wayland protocol errors when accessing PipeWire audio devices.
    // GTK_USE_PORTAL=1 forces proper XDG portal usage for media access.
    //
    // Fixed on: 2025-11-21 (Nobara 42, KDE Plasma 6.2, Wayland, webkit2gtk 0.18)
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("GTK_USE_PORTAL", "1");
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    // Check for CLI arguments (for triggering actions via KDE shortcuts on Wayland)
    let args: Vec<String> = std::env::args().collect();
    let trigger_voice = args.iter().any(|arg| arg == "--trigger-voice");

    let tray_menu: SystemTrayMenu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new(
            "toggle_window".to_string(),
            "Show/Hide Window (Ctrl+Alt+A)",
        ))
        .add_item(CustomMenuItem::new(
            "trigger_voice_pipeline".to_string(),
            "Trigger Voice Pipeline (Ctrl+Shift+A)",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("open_settings".to_string(), "Settings"))
        .add_item(CustomMenuItem::new(
            "open_logs_directory".to_string(),
            "Open Logs",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(
            "check_for_updates".to_string(),
            format!("Check for Updates ({})", env!("CARGO_PKG_VERSION")),
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit_application".to_string(), "Quit"));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            log::info!("Single instance triggered with args: {:?}", argv);

            // Check if --trigger-voice flag is present
            if argv.iter().any(|arg| arg == "--trigger-voice") {
                let window = app.get_window("main").unwrap();
                trigger_voice_pipeline(window);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([LogTarget::LogDir, LogTarget::Stdout, LogTarget::Webview])
                .build(),
        )
        .on_window_event(|event: tauri::GlobalWindowEvent| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .system_tray(SystemTray::new().with_menu(tray_menu))
        .on_system_tray_event(
            |app: &tauri::AppHandle, event: tauri::SystemTrayEvent| match event {
                tauri::SystemTrayEvent::DoubleClick { .. } => {
                    let settings = load_settings(app.clone()).unwrap();

                    let action = if settings.tray.is_some() {
                        settings.tray.unwrap().double_click_action
                    } else {
                        "toggle_window".to_string()
                    };

                    let window: tauri::Window = app.get_window("main").unwrap();
                    match action.as_str() {
                        "toggle_window" => {
                            toggle_window(window);
                        }
                        "trigger_voice_pipeline" => {
                            trigger_voice_pipeline(window);
                        }
                        _ => {}
                    }
                }
                tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
                    let window: tauri::Window = app.get_window("main").unwrap();
                    match id.as_str() {
                        "toggle_window" => toggle_window(window),
                        "trigger_voice_pipeline" => trigger_voice_pipeline(window),
                        "open_settings" => open_settings(window),
                        "open_logs_directory" => open_logs_directory(app.clone()),
                        "check_for_updates" => open_browser(
                            "https://github.com/timmo001/home-assistant-assist-desktop/releases",
                        )
                        .unwrap(),
                        "quit_application" => quit_application(window),
                        _ => {}
                    }
                }
                _ => {}
            },
        )
        .invoke_handler(tauri::generate_handler![
            open_app,
            open_settings,
            load_settings,
            update_settings,
            toggle_window,
            trigger_voice_pipeline,
            hide_window,
            open_logs_directory,
            quit_application
        ])
        .setup(move |app: &mut tauri::App| {
            // Linux: Log environment status (env vars already set in main())
            #[cfg(target_os = "linux")]
            {
                match std::env::var("XDG_RUNTIME_DIR") {
                    Ok(dir) => log::info!("XDG_RUNTIME_DIR: {}", dir),
                    Err(_) => log::warn!("XDG_RUNTIME_DIR not set - PipeWire access may fail"),
                }

                log::info!("Webkit Wayland compatibility flags enabled (GTK_USE_PORTAL=1)");
            }

            let window = app.get_window("main").unwrap();

            // If --trigger-voice flag was passed, trigger the voice pipeline
            if trigger_voice {
                log::info!("CLI: Triggering voice pipeline from --trigger-voice flag");
                trigger_voice_pipeline(window.clone());
            }

            // Linux: Auto-grant microphone/camera permissions
            // With the webkit environment variables set at startup, auto-granting now works
            // without causing Wayland protocol errors. This allows getUserMedia() to succeed.
            #[cfg(target_os = "linux")]
            {
                use webkit2gtk::WebViewExt;
                use webkit2gtk::SettingsExt;

                window.with_webview(|webview| {
                    let wv = webview.inner();

                    // Enable autoplay for TTS audio responses
                    if let Some(settings) = wv.settings() {
                        settings.set_enable_media_stream(true);
                        settings.set_enable_webaudio(true);
                        settings.set_allow_modal_dialogs(true);
                        log::info!("Enabled webkit media stream and webaudio");
                    }

                    wv.connect_permission_request(|_webview, request| {
                        use webkit2gtk::glib::Cast;
                        use webkit2gtk::UserMediaPermissionRequest;
                        use webkit2gtk::PermissionRequestExt;

                        if let Some(media_request) = request.downcast_ref::<UserMediaPermissionRequest>() {
                            log::info!("Auto-granting microphone/camera permission request");
                            media_request.allow();
                            return true;
                        }
                        false
                    });
                }).unwrap();
            }

            // Try to register global shortcuts, but don't panic if they fail
            // (might already be in use by another app)
            if let Err(e) = app.global_shortcut_manager()
                .register("Ctrl+Alt+A", move || {
                    toggle_window(window.clone());
                })
            {
                log::warn!("Could not register Ctrl+Alt+A shortcut: {}", e);
            }

            let window = app.get_window("main").unwrap();
            if let Err(e) = app.global_shortcut_manager()
                .register("Ctrl+Shift+A", move || {
                    trigger_voice_pipeline(window.clone());
                })
            {
                log::warn!("Could not register Ctrl+Shift+A shortcut: {}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
