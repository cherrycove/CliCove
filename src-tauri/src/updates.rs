use crate::models::{
    InstallCandidate, LatestVersionInfo, ToolCategory, ToolRecord, UpdateHistoryRecord, UpdatePlan,
};
use crate::scanner::run_probe;
use crate::version_queries::{self, QueryKind};
use crate::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[derive(Clone)]
struct InstallRecipe {
    id: &'static str,
    adapter_id: &'static str,
    display_name: &'static str,
    category: ToolCategory,
    manager: &'static str,
    package: &'static str,
    cask: bool,
}

#[tauri::command]
pub fn list_install_candidates(state: State<'_, AppState>) -> Vec<InstallCandidate> {
    install_candidates(&state)
}

#[tauri::command]
pub fn latest_version_checks(state: State<'_, AppState>) -> Result<Vec<LatestVersionInfo>, String> {
    // Do not let the old blanket source restriction survive a resolver upgrade.
    Ok(state.storage.latest_checks()?.into_iter().filter(|check| {
        check.status != "unsupported" || check.message != "该安装来源没有经过审核的只读最新版查询方式"
    }).collect())
}

#[tauri::command]
pub async fn check_latest_version(
    tool_id: String,
    app: AppHandle,
) -> Result<LatestVersionInfo, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let tool = state
            .registry
            .get(&tool_id)
            .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".to_string())?;
        let check = latest_for_tool(&state, &tool);
        state.storage.save_latest_check(&check)?;
        Ok(check)
    })
    .await
}

#[tauri::command]
pub async fn check_install_version(
    candidate_id: String,
    app: AppHandle,
) -> Result<LatestVersionInfo, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let recipe = require_recipe(&candidate_id)?;
        let check = latest_for_recipe(&state, &recipe);
        state.storage.save_latest_check(&check)?;
        Ok(check)
    })
    .await
}

#[tauri::command]
pub async fn plan_update(tool_id: String, app: AppHandle) -> Result<UpdatePlan, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let tool = state
            .registry
            .get(&tool_id)
            .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".to_string())?;
        require_runtime_dependencies(&tool)?;
        if !tool.capabilities.update {
            return Err("该工具不是用户直接安装的包，不能单独更新".into());
        }

        let check = latest_for_tool(&state, &tool);
        state.storage.save_latest_check(&check)?;
        if check.update_available == Some(false) {
            return Err(format!("{} 已是最新版本", tool.display_name));
        }
        let target = check
            .latest_version
            .clone()
            .unwrap_or_else(|| "由官方更新器决定".into());
        let (executable, args) = update_command(&state, &tool, check.latest_version.as_deref())?;
        validate_args(&args)?;
        let plan = UpdatePlan {
            id: Uuid::new_v4().to_string(),
            operation: "update".into(),
            tool_id: tool.id.clone(),
            adapter_id: tool.adapter_id.clone(),
            tool_name: tool.display_name.clone(),
            current_version: current_version(&tool),
            target_version: target,
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
    })
    .await
}

#[tauri::command]
pub async fn plan_install(
    candidate_id: String,
    app: AppHandle,
) -> Result<UpdatePlan, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let recipe = require_recipe(&candidate_id)?;
        let candidate = install_candidates(&state)
            .into_iter()
            .find(|candidate| candidate.id == candidate_id)
            .ok_or_else(|| "安装项不可用".to_string())?;
        if candidate.installed {
            return Err(format!("{} 已经安装", candidate.display_name));
        }
        if !candidate.manager_available {
            return Err(format!("找不到安装依赖：{}", candidate.package_manager));
        }
        let check = latest_for_recipe(&state, &recipe);
        state.storage.save_latest_check(&check)?;
        let target = check.latest_version.clone().ok_or_else(|| {
            format!(
                "无法确认 {} 的最新版本：{}",
                recipe.display_name, check.message
            )
        })?;
        let executable = state
            .registry
            .resolve_executable(recipe.manager)
            .ok_or_else(|| format!("找不到安装依赖：{}", recipe.manager))?;
        let args = install_args(&recipe, &target);
        validate_args(&args)?;
        let plan = UpdatePlan {
            id: Uuid::new_v4().to_string(),
            operation: "install".into(),
            tool_id: format!("install:{}", recipe.id),
            adapter_id: Some(recipe.adapter_id.into()),
            tool_name: recipe.display_name.into(),
            current_version: None,
            target_version: target,
            display_command: display_command(&executable, &args),
            executable,
            args,
            source: candidate.source,
            requires_elevation: false,
        };
        state
            .pending_updates
            .lock()
            .insert(plan.id.clone(), plan.clone());
        Ok(plan)
    })
    .await
}

#[tauri::command]
pub fn cancel_update_plan(plan_id: String, state: State<'_, AppState>) -> bool {
    state.pending_updates.lock().remove(&plan_id).is_some()
}

#[tauri::command]
pub fn execute_update(
    plan_id: String,
    cwd: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::models::TerminalSession, String> {
    let plan = state
        .pending_updates
        .lock()
        .remove(&plan_id)
        .ok_or_else(|| "操作计划已失效，请重新生成并确认".to_string())?;
    let directory = resolve_cwd(
        cwd,
        &state.storage.load_settings().default_working_directory,
    )?;
    let session_id = Uuid::new_v4().to_string();
    let mut history_plan = plan.clone();
    let is_custom_adapter = plan.adapter_id.as_ref().is_some_and(|adapter_id| {
        state
            .custom_adapters
            .read()
            .iter()
            .any(|adapter| adapter.id == *adapter_id && adapter.status == "trusted")
    });
    if is_custom_adapter {
        history_plan.display_command = "受信自定义适配器命令（可执行文件与参数未写入历史）".into();
    }
    state.storage.update_started(&history_plan, &session_id)?;
    let session = state.terminal.spawn(
        Some(session_id),
        app,
        plan.tool_id.clone(),
        format!(
            "{} {}",
            if plan.operation == "install" {
                "安装"
            } else {
                "更新"
            },
            plan.tool_name
        ),
        plan.executable.clone(),
        plan.args.clone(),
        directory,
        110,
        32,
        state.registry.execution_path(),
        "management".into(),
    );
    let session = match session {
        Ok(session) => session,
        Err(error) => {
            state.storage.finish_update(
                &plan.id,
                None,
                None,
                "failed",
                &format!("无法启动管理命令：{error}"),
            )?;
            return Err(error);
        }
    };
    Ok(session)
}

#[tauri::command]
pub async fn finalize_update(
    session_id: String,
    app: AppHandle,
) -> Result<UpdateHistoryRecord, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let context = state
            .storage
            .update_context(&session_id)?
            .ok_or_else(|| "该终端不是安装或更新会话".to_string())?;
        if context.status != "running" {
            return state
                .storage
                .update_history(500)?
                .into_iter()
                .find(|record| record.id == context.id)
                .ok_or_else(|| "找不到操作历史".to_string());
        }
        let exit_code = state
            .storage
            .session_exit_code(&session_id)?
            .ok_or_else(|| "终端尚未退出，暂时不能验证".to_string())?;
        if exit_code != Some(0) {
            let message = match exit_code {
                Some(code) => format!("命令退出码为 {code}"),
                None => "无法取得命令退出码".into(),
            };
            state
                .storage
                .finish_update(&context.id, exit_code, None, "failed", &message)?;
            return history_by_id(&state, &context.id);
        }

        let settings = state.storage.load_settings();
        let tools = state
            .registry
            .scan(settings.show_system_tools, settings.show_all_executables);
        state.storage.cache_tools(&tools)?;
        let observed_tool = if context.operation == "install" {
            let recipe_id = context.tool_id.trim_start_matches("install:");
            install_recipes()
                .into_iter()
                .find(|recipe| recipe.id == recipe_id)
                .and_then(|recipe| find_recipe_tool(&tools, &recipe))
        } else {
            tools
                .iter()
                .find(|tool| tool.id == context.tool_id)
                .cloned()
                .or_else(|| {
                    context.adapter_id.as_ref().and_then(|adapter_id| {
                        tools
                            .iter()
                            .find(|tool| tool.adapter_id.as_ref() == Some(adapter_id))
                            .cloned()
                    })
                })
        };
        let Some(tool) = observed_tool else {
            state.storage.finish_update(
                &context.id,
                exit_code,
                None,
                "verification-failed",
                "命令成功退出，但重新扫描后未发现目标 CLI",
            )?;
            return history_by_id(&state, &context.id);
        };
        let observed = tool
            .version
            .clone()
            .or_else(|| state.registry.probe(&tool.id).ok());
        let Some(observed_version) = observed else {
            state.storage.finish_update(
                &context.id,
                exit_code,
                None,
                "verification-failed",
                "已发现目标 CLI，但无法重新探测版本",
            )?;
            return history_by_id(&state, &context.id);
        };
        let post_check = latest_for_tool(&state, &tool);
        state.storage.save_latest_check(&post_check)?;

        let target_is_exact = version_token(&context.target_version).is_some();
        let verified = if target_is_exact {
            version_at_least(&observed_version, &context.target_version)
        } else {
            true
        };
        let changed = context
            .current_version
            .as_ref()
            .is_none_or(|current| !versions_equal(current, &observed_version));
        let (status, message) = if verified {
            (
                "succeeded",
                if changed {
                    format!("重新扫描并验证版本：{}", compact_version(&observed_version))
                } else {
                    format!(
                        "官方更新器执行成功，重新探测后版本仍为 {}（已是最新）",
                        compact_version(&observed_version)
                    )
                },
            )
        } else {
            (
                "verification-failed",
                format!(
                    "命令成功退出，但探测版本 {} 未达到目标 {}",
                    compact_version(&observed_version),
                    context.target_version
                ),
            )
        };
        state.storage.finish_update(
            &context.id,
            exit_code,
            Some(&observed_version),
            status,
            &message,
        )?;
        history_by_id(&state, &context.id)
    })
    .await
}

#[tauri::command]
pub fn list_update_history(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<UpdateHistoryRecord>, String> {
    state.storage.update_history(limit.unwrap_or(100))
}

fn history_by_id(state: &State<'_, AppState>, id: &str) -> Result<UpdateHistoryRecord, String> {
    state
        .storage
        .update_history(500)?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| "找不到操作历史".to_string())
}

fn install_candidates(state: &State<'_, AppState>) -> Vec<InstallCandidate> {
    let tools = state.registry.all();
    install_recipes()
        .into_iter()
        .map(|recipe| {
            let installed_tool = find_recipe_tool(&tools, &recipe);
            InstallCandidate {
                id: recipe.id.into(),
                adapter_id: recipe.adapter_id.into(),
                display_name: recipe.display_name.into(),
                category: recipe.category.clone(),
                package_manager: recipe.manager.into(),
                package_name: recipe.package.into(),
                source: package_source(&recipe),
                manager_available: state.registry.resolve_executable(recipe.manager).is_some(),
                installed: installed_tool.is_some(),
                installed_tool_id: installed_tool.map(|tool| tool.id),
            }
        })
        .collect()
}

fn find_recipe_tool(tools: &[ToolRecord], recipe: &InstallRecipe) -> Option<ToolRecord> {
    tools
        .iter()
        .find(|tool| {
            tool.adapter_id.as_deref() == Some(recipe.adapter_id)
                || tool.service.as_ref().is_some_and(|service| service.id == recipe.id)
        })
        .cloned()
}

fn require_recipe(id: &str) -> Result<InstallRecipe, String> {
    install_recipes()
        .into_iter()
        .find(|recipe| recipe.id == id)
        .ok_or_else(|| "安装项不在内置可信目录中".to_string())
}

fn latest_for_tool(state: &State<'_, AppState>, tool: &ToolRecord) -> LatestVersionInfo {
    let current = current_version(tool);
    let queries = match version_queries::plan(tool) {
        Ok(queries) if queries.is_empty() => {
            let mut check = latest_result(tool.id.clone(), current, tool.source.label.clone(),
                Err("未找到此安装方式的只读更新检查或可靠发布源".into()));
            check.status = "unsupported".into();
            return check;
        }
        Ok(queries) => queries,
        Err(error) => return latest_result(tool.id.clone(), current, tool.source.label.clone(), Err(error)),
    };
    match version_queries::try_queries(&queries, |query| match &query.kind {
        QueryKind::Npm(package) => query_npm(state, package),
        QueryKind::Brew(package) => query_brew(state, package, false),
        _ => version_queries::execute(query, &state.registry, tool),
    }) {
        Ok((version, source, fallback)) => {
            let mut check = latest_result(tool.id.clone(), current, source, Ok(version));
            if fallback { check.message.push_str("；主查询失败，已使用备用发布源"); }
            if tool.source.kind == "application" {
                check.update_available = None;
                check.message.push_str("；应用随附的 CLI，仅供参考，请通过所属应用更新");
            }
            check
        }
        Err(error) => latest_result(tool.id.clone(), current,
            queries.iter().map(|query| query.label.as_str()).collect::<Vec<_>>().join(" → "), Err(error)),
    }
}

fn latest_for_recipe(state: &State<'_, AppState>, recipe: &InstallRecipe) -> LatestVersionInfo {
    let result = if recipe.manager == "npm" {
        query_npm(state, recipe.package)
    } else if recipe.manager == "brew" {
        query_brew(state, recipe.package, recipe.cask)
    } else {
        Err("该包管理器暂不支持结构化最新版查询".into())
    };
    latest_result(
        format!("install:{}", recipe.id),
        None,
        package_source(recipe),
        result,
    )
}

fn latest_result(
    target_id: String,
    current_version: Option<String>,
    source: String,
    result: Result<String, String>,
) -> LatestVersionInfo {
    let checked_at = chrono::Utc::now().to_rfc3339();
    match result {
        Ok(latest) => {
            let update_available = current_version
                .as_ref()
                .and_then(|current| compare_versions(current, &latest))
                .map(|ordering| ordering == std::cmp::Ordering::Less);
            let message = if current_version.as_ref().and_then(|current| compare_versions(current, &latest))
                == Some(std::cmp::Ordering::Greater) {
                format!("渠道版本 {latest}；本机版本更高，无需降级")
            } else {
                format!("最新版 {latest}")
            };
            LatestVersionInfo {
                target_id,
                current_version,
                latest_version: Some(latest.clone()),
                update_available,
                checked_at,
                source,
                status: "checked".into(),
                message,
            }
        }
        Err(message) => LatestVersionInfo {
            target_id,
            current_version,
            latest_version: None,
            update_available: None,
            checked_at,
            source,
            status: "error".into(),
            message,
        },
    }
}

fn query_npm(state: &State<'_, AppState>, package: &str) -> Result<String, String> {
    safe_package_name(package)?;
    let npm = state
        .registry
        .resolve_executable("npm")
        .ok_or_else(|| "找不到 npm".to_string())?;
    let output = run_probe(
        &npm,
        &[
            "view".into(),
            package.into(),
            "version".into(),
            "--json".into(),
        ],
        Duration::from_secs(20),
        &state.registry.execution_path(),
    )?;
    serde_json::from_str::<String>(&output)
        .or_else(|_| Ok(output.trim().trim_matches('"').to_string()))
        .and_then(non_empty_version)
}

fn query_brew(state: &State<'_, AppState>, package: &str, cask: bool) -> Result<String, String> {
    safe_package_name(package)?;
    let brew = state
        .registry
        .resolve_executable("brew")
        .ok_or_else(|| "找不到 Homebrew".to_string())?;
    let mut args = vec!["info".into(), "--json=v2".into()];
    if cask {
        args.push("--cask".into());
    }
    args.push(package.into());
    let output = run_probe(
        &brew,
        &args,
        Duration::from_secs(20),
        &state.registry.execution_path(),
    )?;
    let value: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| format!("Homebrew 返回了无效 JSON：{error}"))?;
    let version = if cask {
        value.pointer("/casks/0/version")
    } else {
        value.pointer("/formulae/0/versions/stable")
    };
    version
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Homebrew 未返回稳定版本".into())
        .and_then(non_empty_version)
}


fn update_command(
    state: &State<'_, AppState>,
    tool: &ToolRecord,
    target: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    if let Some(package) = tool.source.package_name.as_deref() {
        safe_package_name(package)?;
        match tool.source.kind.as_str() {
            "npm" => {
                let npm = state
                    .registry
                    .resolve_executable("npm")
                    .ok_or_else(|| "找不到 npm".to_string())?;
                let selector = target
                    .map(|version| format!("{package}@{version}"))
                    .unwrap_or_else(|| format!("{package}@latest"));
                return Ok((npm, vec!["install".into(), "-g".into(), selector]));
            }
            "homebrew" => {
                let brew = state
                    .registry
                    .resolve_executable("brew")
                    .ok_or_else(|| "找不到 Homebrew".to_string())?;
                return Ok((brew, vec!["upgrade".into(), package.into()]));
            }
            _ => {}
        }
    }
    let adapter = state
        .registry
        .adapter_for(tool)
        .and_then(|adapter| adapter.update)
        .ok_or_else(|| "该工具没有可验证的官方更新方式".to_string())?;
    let executable = if adapter.executable == "{self}" {
        tool.executable.clone()
    } else {
        state
            .registry
            .resolve_executable(&adapter.executable)
            .ok_or_else(|| format!("找不到更新依赖：{}", adapter.executable))?
    };
    Ok((executable, adapter.args))
}

fn install_args(recipe: &InstallRecipe, target: &str) -> Vec<String> {
    if recipe.manager == "npm" {
        vec![
            "install".into(),
            "-g".into(),
            format!("{}@{target}", recipe.package),
        ]
    } else {
        let mut args = vec!["install".into()];
        if recipe.cask {
            args.push("--cask".into());
        }
        args.push(recipe.package.into());
        args
    }
}

fn package_source(recipe: &InstallRecipe) -> String {
    if recipe.manager == "npm" {
        format!("npm registry · {}", recipe.package)
    } else if recipe.cask {
        format!("Homebrew Cask · {}", recipe.package)
    } else {
        format!("Homebrew · {}", recipe.package)
    }
}

fn current_version(tool: &ToolRecord) -> Option<String> {
    tool.version
        .clone()
        .or_else(|| tool.source.package_version.clone())
}

fn compact_version(value: &str) -> String {
    version_token(value).unwrap_or_else(|| value.lines().next().unwrap_or(value).to_string())
}

fn non_empty_version(value: String) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err("最新版为空".into())
    } else {
        Ok(value)
    }
}

fn versions_equal(left: &str, right: &str) -> bool {
    match (version_token(left), version_token(right)) {
        (Some(left), Some(right)) => left.trim_start_matches('v') == right.trim_start_matches('v'),
        _ => left.trim() == right.trim(),
    }
}

fn compare_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = version_token(left)?;
    let right = version_token(right)?;
    let left = semver::Version::parse(left.trim_start_matches('v')).ok()?;
    let right = semver::Version::parse(right.trim_start_matches('v')).ok()?;
    Some(left.cmp_precedence(&right))
}

fn version_at_least(observed: &str, target: &str) -> bool {
    if versions_equal(observed, target) {
        return true;
    }
    let Some(observed) = version_numbers(observed) else {
        return false;
    };
    let Some(target) = version_numbers(target) else {
        return false;
    };
    observed >= target
}

fn version_numbers(value: &str) -> Option<Vec<u64>> {
    version_token(value)?
        .trim_start_matches('v')
        .split(['.', '-'])
        .take_while(|part| part.chars().all(|character| character.is_ascii_digit()))
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|parts| !parts.is_empty())
}

fn version_token(value: &str) -> Option<String> {
    let chars: Vec<char> = value.chars().collect();
    for index in 0..chars.len() {
        let starts_with_v =
            chars[index] == 'v' && chars.get(index + 1).is_some_and(char::is_ascii_digit);
        if !chars[index].is_ascii_digit() && !starts_with_v {
            continue;
        }
        let start = index;
        let mut end = index + usize::from(starts_with_v);
        while end < chars.len()
            && (chars[end].is_ascii_alphanumeric() || ".+-_".contains(chars[end]))
        {
            end += 1;
        }
        let token: String = chars[start..end].iter().collect();
        if token.contains('.') {
            return Some(token.trim_end_matches(['.', '-', '+', '_']).to_string());
        }
    }
    None
}

fn safe_package_name(package: &str) -> Result<(), String> {
    if !package.is_empty()
        && package.len() < 180
        && package
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@+._-/".contains(character))
    {
        Ok(())
    } else {
        Err("包名称未通过安全校验".into())
    }
}

fn validate_args(args: &[String]) -> Result<(), String> {
    if args.len() > 128
        || args
            .iter()
            .any(|arg| arg.contains('\0') || arg.len() > 8192)
    {
        Err("管理命令参数包含无效内容".into())
    } else {
        Ok(())
    }
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
            "{} 缺少运行依赖：{}",
            tool.display_name,
            missing.join(", ")
        ))
    }
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

fn resolve_cwd(cwd: Option<String>, default: &str) -> Result<PathBuf, String> {
    let directory = PathBuf::from(cwd.unwrap_or_else(|| default.to_string()));
    if directory.is_dir() {
        Ok(directory)
    } else {
        Err(format!("工作目录不存在：{}", directory.display()))
    }
}

fn install_recipes() -> Vec<InstallRecipe> {
    crate::service_catalog::services()
        .iter()
        .flat_map(|service| {
            service.packages.iter().filter_map(move |package| {
                if !package.install {
                    return None;
                }
                let manager = match package.source.as_str() {
                    "npm" => "npm",
                    "homebrew" if cfg!(target_os = "macos") => "brew",
                    _ => return None,
                };
                Some(InstallRecipe {
                    id: &service.id,
                    adapter_id: &service.id,
                    display_name: &service.display_name,
                    category: service.category.clone(),
                    manager,
                    package: &package.name,
                    cask: package.cask,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_versions_from_cli_output() {
        assert_eq!(
            version_token("codex-cli 0.147.0-alpha.6"),
            Some("0.147.0-alpha.6".into())
        );
        assert_eq!(
            version_token("aws-cli/2.36.0 Python/3.13"),
            Some("2.36.0".into())
        );
        assert!(versions_equal("grok 1.0.0 (abc)", "1.0.0"));
    }

    #[test]
    fn version_verification_accepts_newer_patch() {
        assert!(version_at_least("tool 1.18.16", "1.18.15"));
        assert!(!version_at_least("tool 1.18.5", "1.18.15"));
    }

    #[test]
    fn latest_comparison_respects_channels_and_prereleases() {
        let result = |current: &str, latest: &str| latest_result("tool".into(), Some(current.into()), "test channel".into(), Ok(latest.into()));
        assert_eq!(result("2.1.260", "2.1.236").update_available, Some(false));
        assert_eq!(result("1.0.0-alpha.2", "1.0.0").update_available, Some(true));
        assert_eq!(result("1.0.0", "1.0.0-alpha.2").update_available, Some(false));
        assert_eq!(result("v1.0.0+abc", "1.0.0+def").update_available, Some(false));
        assert_eq!(result("unknown", "1.0.0").update_available, None);
        assert!(result("2.1.260", "2.1.236").message.contains("无需降级"));
    }

    #[test]
    fn network_and_parse_failures_are_retryable_not_unsupported() {
        let result = latest_result("tool".into(), None, "official".into(), Err("连接失败".into()));
        assert_eq!(result.status, "error");
        assert_eq!(result.update_available, None);
    }

    #[test]
    fn install_arguments_are_arrays_with_exact_version() {
        let recipe = require_recipe("volcengine").unwrap();
        assert_eq!(
            install_args(&recipe, "1.2.3"),
            vec!["install", "-g", "@volcengine/cli@1.2.3"]
        );
        assert!(safe_package_name("bad;command").is_err());
    }

    #[test]
    fn install_catalog_and_discovery_share_identity_without_requiring_an_adapter() {
        for recipe in install_recipes() {
            assert!(safe_package_name(recipe.package).is_ok());
            let service = crate::service_catalog::services().iter()
                .find(|service| service.id == recipe.id).unwrap();
            let mut tool: ToolRecord = serde_json::from_value(serde_json::json!({
                "id": "test-installation", "name": service.commands[0],
                "displayName": service.display_name, "executable": "/test/cli",
                "canonicalPath": "/test/cli", "aliases": service.commands,
                "relatedCommands": service.commands, "version": "1.2.3",
                "source": {
                    "kind": if recipe.manager == "brew" { "homebrew" } else { "npm" },
                    "label": recipe.manager, "packageName": recipe.package, "scope": "global"
                },
                "category": service.category, "isSystem": false, "adapterId": null,
                "capabilities": { "terminal": true, "versionProbe": true, "update": true,
                    "diagnose": false, "auth": false, "config": false },
                "configPaths": []
            })).unwrap();
            tool.service = crate::service_catalog::find_service(&tool.aliases, &tool.source, None)
                .map(|entry| entry.identity());
            assert_eq!(find_recipe_tool(&[tool.clone()], &recipe).unwrap().id, tool.id);
            // A matching command alone must not hide an uninstalled product.
            tool.service = None;
            tool.source.package_name = Some("unrelated-package".into());
            assert!(find_recipe_tool(&[tool], &recipe).is_none());
        }
    }
}
