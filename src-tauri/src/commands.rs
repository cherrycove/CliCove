use crate::adapters::config_preview_fields;
use crate::models::{
    AdapterManifest, AppSettings, CommandTemplate, ConfigPreview, CustomAdapterInfo,
    SessionHistoryRecord, TerminalSession, TerminalSpawnRequest, ToolRecord,
};
use crate::AppState;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn scan_tools(
    show_system: bool,
    show_all_executables: bool,
    app: AppHandle,
) -> Result<Vec<ToolRecord>, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let tools = state.registry.scan(show_system, show_all_executables);
        state.storage.cache_tools(&tools)?;
        let resources = crate::resources::discover(&tools);
        state.storage.sync_discovered_resources(&resources)?;
        Ok(tools)
    })
    .await
}

#[tauri::command]
pub async fn probe_version(tool_id: String, app: AppHandle) -> Result<String, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let tool = require_tool(&state, &tool_id)?;
        require_runtime_dependencies(&tool)?;
        state.registry.probe(&tool_id)
    })
    .await
}

#[tauri::command]
pub fn list_adapters(state: State<'_, AppState>) -> Vec<AdapterManifest> {
    state.adapters.read().clone()
}

#[tauri::command]
pub fn list_custom_adapters(state: State<'_, AppState>) -> Vec<CustomAdapterInfo> {
    state.reload_adapters();
    state.custom_adapters.read().clone()
}

#[tauri::command]
pub fn open_adapter_directory(state: State<'_, AppState>) -> Result<(), String> {
    open::that(&state.adapter_directory).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn trust_custom_adapter(
    path: String,
    fingerprint: String,
    state: State<'_, AppState>,
) -> Result<Vec<CustomAdapterInfo>, String> {
    state.reload_adapters();
    let adapter = state
        .custom_adapters
        .read()
        .iter()
        .find(|adapter| adapter.path == path && adapter.fingerprint == fingerprint)
        .cloned()
        .ok_or_else(|| "适配器文件已变化，请刷新后重新审查".to_string())?;
    if adapter.status != "pending" {
        return Err("只有待审查且无冲突的适配器可以被信任".into());
    }
    state
        .storage
        .trust_adapter(&adapter.path, &adapter.fingerprint)?;
    state.reload_adapters();
    Ok(state.custom_adapters.read().clone())
}

#[tauri::command]
pub fn revoke_custom_adapter(
    path: String,
    fingerprint: String,
    state: State<'_, AppState>,
) -> Result<Vec<CustomAdapterInfo>, String> {
    state.reload_adapters();
    let adapter = state
        .custom_adapters
        .read()
        .iter()
        .find(|adapter| adapter.path == path && adapter.fingerprint == fingerprint)
        .cloned()
        .ok_or_else(|| "适配器文件已变化，请刷新后重试".to_string())?;
    if !adapter.has_stored_trust {
        return Err("该适配器没有已保存的信任记录".into());
    }
    state.storage.revoke_adapter(&adapter.path)?;
    state.reload_adapters();
    Ok(state.custom_adapters.read().clone())
}

#[tauri::command]
pub fn spawn_terminal(
    request: TerminalSpawnRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    let tool = require_tool(&state, &request.tool_id)?;
    require_runtime_dependencies(&tool)?;
    validate_args(&request.args)?;
    let directory = resolve_cwd(request.cwd, &state.storage.load_settings())?;
    state.terminal.spawn(
        None,
        app,
        tool.id,
        tool.display_name,
        tool.executable,
        request.args,
        directory,
        request.cols.unwrap_or(110),
        request.rows.unwrap_or(32),
        state.registry.execution_path(),
        "interactive".into(),
    )
}

#[tauri::command]
pub fn run_diagnostic(
    tool_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    run_adapter_action("diagnostic", &tool_id, app, &state)
}

#[tauri::command]
pub fn run_auth(
    tool_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    run_adapter_action("auth", &tool_id, app, &state)
}

#[tauri::command]
pub fn show_config(
    tool_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    run_adapter_action("config", &tool_id, app, &state)
}

#[tauri::command]
pub fn terminal_write(
    session_id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.terminal.write(&session_id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    session_id: String,
    cols: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.terminal.resize(&session_id, cols, rows)
}

#[tauri::command]
pub fn terminal_terminate(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.terminal.terminate(&session_id)
}

#[tauri::command]
pub fn terminal_close(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.terminal.close(&session_id)
}

#[tauri::command]
pub fn terminal_sessions(state: State<'_, AppState>) -> Vec<TerminalSession> {
    state.terminal.list()
}

#[tauri::command]
pub fn terminal_rename(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    state.terminal.rename(&session_id, &title)
}

#[tauri::command]
pub fn session_history(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionHistoryRecord>, String> {
    state.storage.session_history(limit.unwrap_or(50))
}

#[tauri::command]
pub fn config_preview(
    tool_id: String,
    path_index: usize,
    state: State<'_, AppState>,
) -> Result<ConfigPreview, String> {
    let tool = require_tool(&state, &tool_id)?;
    let path = tool
        .config_paths
        .get(path_index)
        .ok_or_else(|| "配置路径不在适配器白名单中".to_string())?;
    let adapter_id = tool
        .adapter_id
        .as_deref()
        .ok_or_else(|| "只有内置适配器支持安全配置预览".to_string())?;
    let fields = config_preview_fields(adapter_id, path_index)
        .ok_or_else(|| "该配置没有经过审核的安全预览规则，请使用外部编辑器打开".to_string())?;
    preview_file(Path::new(path), fields)
}

#[tauri::command]
pub fn open_config(
    tool_id: String,
    path_index: usize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let tool = require_tool(&state, &tool_id)?;
    let path = tool
        .config_paths
        .get(path_index)
        .ok_or_else(|| "配置路径不在适配器白名单中".to_string())?;
    let target = if Path::new(path).exists() {
        PathBuf::from(path)
    } else {
        Path::new(path)
            .parent()
            .filter(|parent| parent.exists())
            .map(Path::to_path_buf)
            .ok_or_else(|| "配置文件及其目录均不存在".to_string())?
    };
    open::that(target).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.storage.load_settings()
}

#[tauri::command]
pub fn save_scan_settings(
    show_system: bool,
    show_all_executables: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .storage
        .save_scan_settings(show_system, show_all_executables)
}

#[tauri::command]
pub fn save_settings(
    settings: AppSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = Path::new(&settings.default_working_directory);
    if !path.is_dir() {
        return Err("默认工作目录不存在".into());
    }
    let previous = state.storage.load_settings();
    state.storage.save_settings(&settings)?;
    if previous.tray_enabled != settings.tray_enabled {
        crate::set_tray_enabled(&app, settings.tray_enabled)?;
    }
    Ok(())
}

#[tauri::command]
pub fn force_exit(app: AppHandle, state: State<'_, AppState>) {
    state.terminal.terminate_all();
    app.exit(0);
}

fn run_adapter_action(
    action: &str,
    tool_id: &str,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<TerminalSession, String> {
    let tool = require_tool(state, tool_id)?;
    require_runtime_dependencies(&tool)?;
    let adapter = state
        .registry
        .adapter_for(&tool)
        .ok_or_else(|| "该工具没有专用适配器".to_string())?;
    let template = match action {
        "diagnostic" => adapter.diagnose,
        "auth" => adapter.auth,
        "config" => adapter.config_command,
        _ => None,
    }
    .ok_or_else(|| format!("该工具不支持 {action} 操作"))?;
    let (executable, args) = resolve_template(state, &tool, &template)?;
    let directory = resolve_cwd(None, &state.storage.load_settings())?;
    state.terminal.spawn(
        None,
        app,
        tool.id,
        format!("{} · {}", tool.display_name, action),
        executable,
        args,
        directory,
        110,
        32,
        state.registry.execution_path(),
        action.into(),
    )
}

fn require_tool(state: &State<'_, AppState>, id: &str) -> Result<ToolRecord, String> {
    state
        .registry
        .get(id)
        .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".into())
}

fn require_runtime_dependencies(tool: &ToolRecord) -> Result<(), String> {
    let missing: Vec<_> = tool
        .runtime_dependencies
        .iter()
        .filter(|dependency| !dependency.available)
        .map(|dependency| dependency.name.as_str())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} 缺少运行依赖：{}。请安装依赖后重新扫描。",
            tool.display_name,
            missing.join(", ")
        ))
    }
}

fn resolve_template(
    state: &State<'_, AppState>,
    tool: &ToolRecord,
    template: &CommandTemplate,
) -> Result<(String, Vec<String>), String> {
    let executable = if template.executable == "{self}" {
        tool.executable.clone()
    } else {
        state
            .registry
            .resolve_executable(&template.executable)
            .ok_or_else(|| format!("找不到更新依赖：{}", template.executable))?
    };
    Ok((executable, template.args.clone()))
}

fn validate_args(args: &[String]) -> Result<(), String> {
    if args.len() > 128 {
        return Err("参数数量超过安全上限".into());
    }
    if args
        .iter()
        .any(|arg| arg.contains('\0') || arg.len() > 8192)
    {
        return Err("参数包含无效内容".into());
    }
    Ok(())
}

fn resolve_cwd(cwd: Option<String>, settings: &AppSettings) -> Result<PathBuf, String> {
    let directory =
        PathBuf::from(cwd.unwrap_or_else(|| settings.default_working_directory.clone()));
    if directory.is_dir() {
        Ok(directory)
    } else {
        Err(format!("工作目录不存在：{}", directory.display()))
    }
}

fn preview_file(path: &Path, allowed_fields: &[&str]) -> Result<ConfigPreview, String> {
    if !path.exists() {
        return Ok(ConfigPreview {
            path: path.to_string_lossy().to_string(),
            exists: false,
            content: String::new(),
            truncated: false,
            notice: "配置文件尚未创建".into(),
        });
    }
    const LIMIT: u64 = 262_144;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("配置文件超过 256 KiB 安全预览上限，请使用外部编辑器打开".into());
    }
    let content = std::str::from_utf8(&bytes).map_err(|_| "配置文件不是有效 UTF-8".to_string())?;
    let parsed = parse_structured_config(path, content)?;
    let filtered = select_allowed_fields(&parsed, allowed_fields);
    let has_fields = filtered
        .as_object()
        .map(|object| !object.is_empty())
        .unwrap_or(false);
    Ok(ConfigPreview {
        path: path.to_string_lossy().to_string(),
        exists: true,
        content: serde_json::to_string_pretty(&filtered).map_err(|error| error.to_string())?,
        truncated: false,
        notice: if has_fields {
            "仅显示内置适配器审核通过的非敏感字段；凭据、环境变量和未知字段均已省略。".into()
        } else {
            "未找到可安全预览的非敏感字段；文件内容未显示。".into()
        },
    })
}

fn parse_structured_config(path: &Path, content: &str) -> Result<serde_json::Value, String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => {
            serde_json::from_str(content).map_err(|error| format!("配置 JSON 无法解析：{error}"))
        }
        Some("toml") => toml::from_str::<toml::Value>(content)
            .map_err(|error| format!("配置 TOML 无法解析：{error}"))
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string())),
        _ => Err("只有经过审核的 JSON 或 TOML 配置支持安全预览".into()),
    }
}

fn select_allowed_fields(root: &serde_json::Value, allowed_fields: &[&str]) -> serde_json::Value {
    let mut selected = serde_json::Map::new();
    for field in allowed_fields {
        let parts: Vec<_> = field.split('.').collect();
        let Some(value) = value_at_path(root, &parts) else {
            continue;
        };
        if !matches!(
            value,
            serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_)
        ) {
            continue;
        }
        insert_value_at_path(&mut selected, &parts, value.clone());
    }
    serde_json::Value::Object(selected)
}

fn value_at_path<'a>(root: &'a serde_json::Value, parts: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = root;
    for part in parts {
        current = current.as_object()?.get(*part)?;
    }
    Some(current)
}

fn insert_value_at_path(
    target: &mut serde_json::Map<String, serde_json::Value>,
    parts: &[&str],
    value: serde_json::Value,
) {
    let Some((first, rest)) = parts.split_first() else {
        return;
    };
    if rest.is_empty() {
        target.insert((*first).into(), value);
        return;
    }
    let entry = target
        .entry((*first).to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(object) = entry.as_object_mut() {
        insert_value_at_path(object, rest, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_preview_only_copies_explicit_scalar_fields() {
        let input = serde_json::json!({
            "model": "safe-model",
            "api_key": "secret",
            "env": {"TOKEN": "secret"},
            "permissions": {
                "defaultMode": "manual",
                "allow": ["Bash(*)"]
            }
        });
        let output = select_allowed_fields(
            &input,
            &["model", "permissions.defaultMode", "permissions.allow"],
        );
        let rendered = serde_json::to_string(&output).unwrap();
        assert!(rendered.contains("safe-model"));
        assert!(rendered.contains("manual"));
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("TOKEN"));
        assert!(!rendered.contains("Bash"));
    }

    #[test]
    fn structured_preview_parses_toml_without_exposing_unknown_fields() {
        let parsed = parse_structured_config(
            Path::new("config.toml"),
            "model = \"gpt-safe\"\napi_key = \"secret\"\n[permissions]\ndefaultMode = \"manual\"",
        )
        .unwrap();
        let output = select_allowed_fields(&parsed, &["model", "permissions.defaultMode"]);
        let rendered = serde_json::to_string(&output).unwrap();
        assert!(rendered.contains("gpt-safe"));
        assert!(!rendered.contains("secret"));
    }
}
