mod adapters;
mod commands;
mod models;
mod scanner;
mod storage;
mod terminal;

use crate::models::{AdapterManifest, UpdatePlan};
use crate::scanner::ToolRegistry;
use crate::storage::Storage;
use crate::terminal::TerminalManager;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

pub struct AppState {
    registry: ToolRegistry,
    adapters: Vec<AdapterManifest>,
    storage: Arc<Storage>,
    terminal: TerminalManager,
    pending_updates: Mutex<HashMap<String, UpdatePlan>>,
}

impl AppState {
    fn new(storage: Arc<Storage>, adapters: Vec<AdapterManifest>) -> Self {
        Self {
            registry: ToolRegistry::new(adapters.clone()),
            adapters,
            terminal: TerminalManager::new(storage.clone()),
            storage,
            pending_updates: Mutex::new(HashMap::new()),
        }
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub(crate) fn set_tray_enabled(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("clihub-tray") {
        return tray.set_visible(enabled).map_err(|error| error.to_string());
    }
    if !enabled {
        return Ok(());
    }
    let show = MenuItem::with_id(app, "clihub-show", "显示 CLIHub", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, "clihub-quit", "退出", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let menu = Menu::with_items(app, &[&show, &quit]).map_err(|error| error.to_string())?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| "应用图标不可用".to_string())?;
    TrayIconBuilder::with_id("clihub-tray")
        .icon(icon)
        .tooltip("CLIHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == "clihub-show" {
                show_main_window(app);
            } else if event.id() == "clihub-quit" {
                let state = app.state::<AppState>();
                if state.terminal.active_count() > 0 {
                    show_main_window(app);
                    let _ = app.emit("exit-confirmation-requested", ());
                } else {
                    app.exit(0);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            let storage = Arc::new(Storage::new(&data_dir.join("clihub.db"))?);
            let adapters = adapters::load_adapters(&data_dir.join("adapters"));
            let initial_settings = storage.load_settings();
            app.manage(AppState::new(storage, adapters));
            set_tray_enabled(app.handle(), initial_settings.tray_enabled)?;

            let app_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let state = app_handle.state::<AppState>();
                        if state.storage.load_settings().tray_enabled {
                            api.prevent_close();
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                            return;
                        }
                        if state.terminal.active_count() > 0 {
                            api.prevent_close();
                            let _ = app_handle.emit("exit-confirmation-requested", ());
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_tools,
            commands::probe_version,
            commands::list_adapters,
            commands::plan_update,
            commands::execute_update,
            commands::spawn_terminal,
            commands::run_diagnostic,
            commands::run_auth,
            commands::show_config,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_terminate,
            commands::terminal_close,
            commands::terminal_sessions,
            commands::config_preview,
            commands::open_config,
            commands::get_settings,
            commands::save_settings,
            commands::force_exit
        ])
        .build(tauri::generate_context!())
        .expect("failed to build CLIHub");

    app.run(|_, _| {});
}
