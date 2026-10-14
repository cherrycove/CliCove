mod adapters;
mod background;
mod commands;
mod health;
mod models;
mod process;
mod resources;
mod scanner;
mod service_catalog;
mod storage;
mod terminal;
mod updates;
mod version_queries;

use crate::models::{AdapterManifest, CustomAdapterInfo, UpdatePlan};
use crate::scanner::ToolRegistry;
use crate::storage::Storage;
use crate::terminal::TerminalManager;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

pub struct AppState {
    registry: ToolRegistry,
    adapters: RwLock<Vec<AdapterManifest>>,
    custom_adapters: RwLock<Vec<CustomAdapterInfo>>,
    adapter_directory: PathBuf,
    storage: Arc<Storage>,
    terminal: TerminalManager,
    pending_updates: Mutex<HashMap<String, UpdatePlan>>,
}

impl AppState {
    fn new(storage: Arc<Storage>, adapter_directory: PathBuf) -> Self {
        let loaded =
            adapters::load_adapters(&adapter_directory, &storage.trusted_adapter_fingerprints());
        Self {
            registry: ToolRegistry::new(loaded.adapters.clone()),
            adapters: RwLock::new(loaded.adapters),
            custom_adapters: RwLock::new(loaded.custom_adapters),
            adapter_directory,
            terminal: TerminalManager::new(storage.clone()),
            storage,
            pending_updates: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn reload_adapters(&self) {
        let loaded = adapters::load_adapters(
            &self.adapter_directory,
            &self.storage.trusted_adapter_fingerprints(),
        );
        self.registry.replace_adapters(loaded.adapters.clone());
        *self.adapters.write() = loaded.adapters;
        *self.custom_adapters.write() = loaded.custom_adapters;
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
    if let Some(tray) = app.tray_by_id("clicove-tray") {
        return tray.set_visible(enabled).map_err(|error| error.to_string());
    }
    if !enabled {
        return Ok(());
    }
    let show = MenuItem::with_id(app, "clicove-show", "显示 CliCove", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, "clicove-quit", "退出", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let menu = Menu::with_items(app, &[&show, &quit]).map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    let icon = tauri::include_image!("icons/tray-template.png");
    #[cfg(not(target_os = "macos"))]
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| "应用图标不可用".to_string())?;
    TrayIconBuilder::with_id("clicove-tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("CliCove")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == "clicove-show" {
                show_main_window(app);
            } else if event.id() == "clicove-quit" {
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
            storage.mark_running_sessions_interrupted()?;
            storage.mark_running_updates_interrupted()?;
            let initial_settings = storage.load_settings();
            app.manage(AppState::new(storage, data_dir.join("adapters")));
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
            health::authentication_statuses,
            health::diagnostic_reports,
            health::check_authentication_status,
            health::run_structured_diagnostic,
            commands::list_adapters,
            resources::list_resource_catalog,
            commands::list_custom_adapters,
            commands::open_adapter_directory,
            commands::trust_custom_adapter,
            commands::revoke_custom_adapter,
            updates::list_install_candidates,
            updates::latest_version_checks,
            updates::check_latest_version,
            updates::check_install_version,
            updates::plan_update,
            updates::plan_install,
            updates::cancel_update_plan,
            updates::execute_update,
            updates::finalize_update,
            updates::list_update_history,
            commands::spawn_terminal,
            commands::run_diagnostic,
            commands::run_auth,
            commands::show_config,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_terminate,
            commands::terminal_close,
            commands::terminal_sessions,
            commands::terminal_rename,
            commands::session_history,
            commands::config_preview,
            commands::open_config,
            commands::get_settings,
            commands::save_settings,
            commands::save_scan_settings,
            commands::force_exit
        ])
        .build(tauri::generate_context!())
        .expect("failed to build CliCove");

    app.run(|_, _| {});
}
