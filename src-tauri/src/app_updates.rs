use crate::AppState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHECK_INTERVAL_SECONDS: i64 = 6 * 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdatePreferences {
    pub auto_check: bool,
    pub skipped_version: Option<String>,
    pub last_attempt: Option<i64>,
    pub last_checked: Option<i64>,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            auto_check: true,
            skipped_version: None,
            last_attempt: None,
            last_checked: None,
        }
    }
}

impl UpdatePreferences {
    fn should_check(&self, manual: bool, now: i64) -> bool {
        manual
            || (self.auto_check
                && self
                    .last_attempt
                    .is_none_or(|last| now < last || now - last >= CHECK_INTERVAL_SECONDS))
    }

    fn should_offer(&self, version: &str, manual: bool) -> bool {
        manual || self.skipped_version.as_deref() != Some(version)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: String,
    published_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current_version: String,
    preferences: UpdatePreferences,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    checked: bool,
    update: Option<AvailableUpdate>,
    preferences: UpdatePreferences,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Default)]
pub struct AppUpdater {
    pending: tauri::async_runtime::Mutex<Option<Update>>,
}

#[tauri::command]
pub fn app_update_status(app: AppHandle, state: State<'_, AppState>) -> UpdateStatus {
    UpdateStatus {
        current_version: app.package_info().version.to_string(),
        preferences: state.storage.load_update_preferences(),
    }
}

#[tauri::command]
pub fn set_app_update_auto_check(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<UpdatePreferences, String> {
    state
        .storage
        .patch_update_preferences(|prefs| prefs.auto_check = enabled)
}

#[tauri::command]
pub async fn skip_app_update(
    version: String,
    state: State<'_, AppState>,
) -> Result<UpdatePreferences, String> {
    let pending = state.app_updater.pending.lock().await;
    if pending.as_ref().map(|update| &update.version) != Some(&version) {
        return Err("更新信息已变化，请重新检查。".into());
    }
    state
        .storage
        .patch_update_preferences(|prefs| prefs.skipped_version = Some(version))
}

#[tauri::command]
pub async fn check_app_update(
    manual: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CheckResult, String> {
    let mut pending = state.app_updater.pending.lock().await;
    let prefs = state.storage.load_update_preferences();
    let now = chrono::Utc::now().timestamp();
    if !prefs.should_check(manual, now) {
        return Ok(CheckResult {
            checked: false,
            update: None,
            preferences: prefs,
        });
    }
    state
        .storage
        .patch_update_preferences(|prefs| prefs.last_attempt = Some(now))?;
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(update_error)?;
    let update = updater.check().await.map_err(update_error)?;
    if let Some(update) = &update {
        if update.download_url.scheme() != "https" {
            return Err("更新下载地址必须使用 HTTPS。".into());
        }
    }
    let prefs = state
        .storage
        .patch_update_preferences(|prefs| prefs.last_checked = Some(now))?;
    let offered = update
        .as_ref()
        .filter(|update| prefs.should_offer(&update.version, manual))
        .map(|update| AvailableUpdate {
            version: update.version.clone(),
            notes: update.body.clone().unwrap_or_default(),
            published_at: update
                .raw_json
                .get("pub_date")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
        });
    *pending = update;
    Ok(CheckResult {
        checked: true,
        update: offered,
        preferences: prefs,
    })
}

#[tauri::command]
pub async fn install_app_update(
    version: String,
    progress: Channel<UpdateProgress>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Serialize checks/downloads so the confirmed version cannot change during installation.
    let pending = state.app_updater.pending.lock().await;
    let mut update = pending
        .as_ref()
        .filter(|update| update.version == version)
        .cloned()
        .ok_or_else(|| "更新信息已变化，请重新检查。".to_string())?;
    if state.terminal.active_count() > 0 {
        return Err("请先结束正在运行的终端会话，再更新 CliCove。".into());
    }
    update.timeout = Some(Duration::from_secs(15 * 60));
    let mut downloaded = 0;
    let mut last_progress = std::time::Instant::now();
    let _ = progress.send(UpdateProgress {
        phase: "downloading",
        downloaded: 0,
        total: None,
    });
    let bytes = update
        .download(
            |size, total| {
                downloaded += size as u64;
                if last_progress.elapsed() >= Duration::from_millis(100)
                    || total == Some(downloaded)
                {
                    let _ = progress.send(UpdateProgress {
                        phase: "downloading",
                        downloaded,
                        total,
                    });
                    last_progress = std::time::Instant::now();
                }
            },
            || {},
        )
        .await
        .map_err(update_error)?;
    let app_for_install = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_for_install.state::<AppState>();
        // Check again after downloading, holding the same gate used by terminal spawning.
        state.terminal.with_idle_for_app_update(|| {
            let _ = progress.send(UpdateProgress {
                phase: "installing",
                downloaded,
                total: Some(downloaded),
            });
            update.install(bytes).map_err(update_error)?;
            app_for_install.restart();
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn update_error(error: tauri_plugin_updater::Error) -> String {
    match error {
        tauri_plugin_updater::Error::ReleaseNotFound => {
            "暂时无法获取更新信息，请稍后重试，或前往 GitHub Releases 下载。".into()
        }
        tauri_plugin_updater::Error::TargetNotFound(_)
        | tauri_plugin_updater::Error::TargetsNotFound(_) => {
            "此版本暂未提供当前系统的更新包。".into()
        }
        other => format!("CliCove 更新失败：{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_checks_respect_toggle_interval_and_clock_changes() {
        let mut prefs = UpdatePreferences::default();
        assert!(prefs.should_check(false, 100_000));
        prefs.last_attempt = Some(100_000);
        assert!(!prefs.should_check(false, 100_001));
        assert!(prefs.should_check(false, 100_000 + CHECK_INTERVAL_SECONDS));
        assert!(prefs.should_check(false, 99_999));
        prefs.auto_check = false;
        assert!(!prefs.should_check(false, 200_000));
        assert!(prefs.should_check(true, 100_001));
    }

    #[test]
    fn skipped_version_does_not_hide_manual_checks_or_later_releases() {
        let prefs = UpdatePreferences {
            skipped_version: Some("0.6.13".into()),
            ..Default::default()
        };
        assert!(!prefs.should_offer("0.6.13", false));
        assert!(prefs.should_offer("0.6.13", true));
        assert!(prefs.should_offer("0.6.14", false));
    }
}
