use crate::models::{
    AdapterManifest, AppSettings, CommandTemplate, ConfigPreview, TerminalSession,
    TerminalSpawnRequest, ToolRecord, UpdatePlan,
};
use crate::AppState;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use uuid::Uuid;

#[tauri::command]
pub fn scan_tools(
    show_system: bool,
    state: State<'_, AppState>,
) -> Result<Vec<ToolRecord>, String> {
    let tools = state.registry.scan(show_system);
    state.storage.cache_tools(&tools)?;
    Ok(tools)
}

#[tauri::command]
pub fn probe_version(tool_id: String, state: State<'_, AppState>) -> Result<String, String> {
    state.registry.probe(&tool_id)
}

#[tauri::command]
pub fn list_adapters(state: State<'_, AppState>) -> Vec<AdapterManifest> {
    state.adapters.clone()
}

#[tauri::command]
pub fn plan_update(tool_id: String, state: State<'_, AppState>) -> Result<UpdatePlan, String> {
    let tool = require_tool(&state, &tool_id)?;
    if !tool.capabilities.update {
        return Err("该工具不是用户直接安装的包，不能单独更新".into());
    }
    let template = state
        .registry
        .adapter_for(&tool)
        .and_then(|adapter| adapter.update)
        .or_else(|| package_manager_update(&tool))
        .ok_or_else(|| "该工具没有可验证的官方更新方式".to_string())?;
    let (executable, args) = resolve_template(&state, &tool, &template)?;
    validate_args(&args)?;
    let plan = UpdatePlan {
        id: Uuid::new_v4().to_string(),
        tool_id: tool.id.clone(),
        tool_name: tool.display_name.clone(),
        current_version: tool.version.clone().or(tool.source.package_version.clone()),
        target_version: "latest".into(),
        display_command: display_command(&executable, &args),
        executable,
        args,
        source: tool.source.label.clone(),
        requires_elevation: false,
    };
    state
        .pending_updates
        .lock()
        .insert(plan.id.clone(), plan.clone());
    Ok(plan)
}

#[tauri::command]
pub fn execute_update(
    plan_id: String,
    cwd: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    let plan = state
        .pending_updates
        .lock()
        .remove(&plan_id)
        .ok_or_else(|| "更新计划已失效，请重新生成并确认".to_string())?;
    let directory = resolve_cwd(cwd, &state.storage.load_settings())?;
    state.terminal.spawn(
        app,
        plan.tool_id,
        format!("更新 {}", plan.tool_name),
        plan.executable,
        plan.args,
        directory,
        110,
        32,
    )
}

#[tauri::command]
pub fn spawn_terminal(
    request: TerminalSpawnRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TerminalSession, String> {
    let tool = require_tool(&state, &request.tool_id)?;
    validate_args(&request.args)?;
    let directory = resolve_cwd(request.cwd, &state.storage.load_settings())?;
    state.terminal.spawn(
        app,
        tool.id,
        tool.display_name,
        tool.executable,
        request.args,
        directory,
        request.cols.unwrap_or(110),
        request.rows.unwrap_or(32),
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
    preview_file(Path::new(path))
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
        app,
        tool.id,
        format!("{} · {}", tool.display_name, action),
        executable,
        args,
        directory,
        110,
        32,
    )
}

fn require_tool(state: &State<'_, AppState>, id: &str) -> Result<ToolRecord, String> {
    state
        .registry
        .get(id)
        .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".into())
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
            .find_by_name(&template.executable)
            .map(|record| record.executable)
            .ok_or_else(|| format!("找不到更新依赖：{}", template.executable))?
    };
    Ok((executable, template.args.clone()))
}

fn package_manager_update(tool: &ToolRecord) -> Option<CommandTemplate> {
    let package = tool.source.package_name.as_ref()?;
    if !safe_package_name(package) {
        return None;
    }
    match tool.source.kind.as_str() {
        "homebrew" => Some(CommandTemplate {
            executable: "brew".into(),
            args: vec!["upgrade".into(), package.clone()],
        }),
        "npm" => Some(CommandTemplate {
            executable: "npm".into(),
            args: vec!["install".into(), "-g".into(), format!("{package}@latest")],
        }),
        _ => None,
    }
}

fn safe_package_name(package: &str) -> bool {
    !package.is_empty()
        && package.len() < 180
        && package
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@+._-/".contains(character))
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

fn display_command(executable: &str, args: &[String]) -> String {
    std::iter::once(executable)
        .chain(args.iter().map(String::as_str))
        .map(quote_display)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_display(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._@:+-=\\".contains(character))
    {
        value.into()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
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

fn preview_file(path: &Path) -> Result<ConfigPreview, String> {
    if !path.exists() {
        return Ok(ConfigPreview {
            path: path.to_string_lossy().to_string(),
            exists: false,
            content: String::new(),
            truncated: false,
        });
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let truncated = bytes.len() > 262_144;
    let content = String::from_utf8_lossy(&bytes[..bytes.len().min(262_144)]).to_string();
    Ok(ConfigPreview {
        path: path.to_string_lossy().to_string(),
        exists: true,
        content: redact_config(&content),
        truncated,
    })
}

fn redact_config(content: &str) -> String {
    let pattern = Regex::new(
        r#"(?im)((?:\"|')?(?:api[_-]?key|token|secret|password|credential|authorization)(?:\"|')?\s*[:=]\s*)(\"[^\"]*\"|'[^']*'|[^,\s}\]]+)"#,
    )
    .expect("redaction regex is valid");
    pattern.replace_all(content, "${1}\"***\"").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_reject_shell_metacharacters() {
        assert!(safe_package_name("@scope/tool"));
        assert!(!safe_package_name("tool; rm -rf /"));
    }

    #[test]
    fn config_redaction_covers_json_and_toml() {
        let input = "api_key = \"secret\"\n{\"token\":\"abc\",\"model\":\"x\"}";
        let output = redact_config(input);
        assert!(!output.contains("secret"));
        assert!(!output.contains("abc"));
        assert!(output.contains("model"));
    }
}
