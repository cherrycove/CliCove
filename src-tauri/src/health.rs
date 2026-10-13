use crate::models::{
    AuthenticationStatus, DiagnosticCheck, DiagnosticReport, StatusField, ToolRecord,
};
use crate::AppState;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use wait_timeout::ChildExt;

const STATUS_TIMEOUT: Duration = Duration::from_secs(8);
const OUTPUT_LIMIT: usize = 262_144;

struct StatusCommand {
    args: &'static [&'static str],
    source: &'static str,
}

struct CommandResult {
    success: bool,
    output: String,
}

#[tauri::command]
pub fn authentication_statuses(
    state: State<'_, AppState>,
) -> Result<Vec<AuthenticationStatus>, String> {
    state.storage.authentication_statuses()
}

#[tauri::command]
pub fn diagnostic_reports(state: State<'_, AppState>) -> Result<Vec<DiagnosticReport>, String> {
    state.storage.diagnostic_reports()
}

#[tauri::command]
pub async fn check_authentication_status(
    tool_id: String,
    app: AppHandle,
) -> Result<AuthenticationStatus, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let tool = require_tool(&state, &tool_id)?;
        let status = probe_authentication(&tool, &state.registry.execution_path())?;
        state.storage.save_authentication_status(&status)?;
        Ok(status)
    })
    .await
}

#[tauri::command]
pub async fn run_structured_diagnostic(
    tool_id: String,
    app: AppHandle,
) -> Result<DiagnosticReport, String> {
    crate::background::run(move || {
        let state = app.state::<AppState>();
        let started = Instant::now();
        let mut tool = require_tool(&state, &tool_id)?;
        let mut checks = Vec::new();

        checks.push(DiagnosticCheck {
            id: "executable".into(),
            label: "可执行文件".into(),
            status: if Path::new(&tool.executable).is_file() {
                "pass"
            } else {
                "fail"
            }
            .into(),
            message: if Path::new(&tool.executable).is_file() {
                "可执行文件存在，使用下方记录的启动路径".into()
            } else {
                "扫描后可执行文件已消失，请重新扫描".into()
            },
        });

        let missing: Vec<_> = tool
            .runtime_dependencies
            .iter()
            .filter(|dependency| !dependency.available)
            .map(|dependency| dependency.name.as_str())
            .collect();
        checks.push(DiagnosticCheck {
            id: "dependencies".into(),
            label: "运行依赖".into(),
            status: if missing.is_empty() { "pass" } else { "fail" }.into(),
            message: if missing.is_empty() {
                if tool.runtime_dependencies.is_empty() {
                    "没有额外运行时依赖".into()
                } else {
                    format!("{} 项依赖均可用", tool.runtime_dependencies.len())
                }
            } else {
                format!("缺少：{}", missing.join(", "))
            },
        });

        let version_result = state.registry.probe(&tool.id);
        if let Ok(version) = &version_result {
            tool.version = Some(version.clone());
        }
        checks.push(DiagnosticCheck {
            id: "version".into(),
            label: "版本响应".into(),
            status: if version_result.is_ok() {
                "pass"
            } else {
                "warn"
            }
            .into(),
            message: version_result
                .map(|version| format!("CLI 可响应版本探测：{version}"))
                .unwrap_or_else(|error| format!("无法确认版本：{}", safe_message(&error))),
        });

        if tool.capabilities.auth {
            let auth = probe_authentication(&tool, &state.registry.execution_path())?;
            state.storage.save_authentication_status(&auth)?;
            let check_status = match auth.status.as_str() {
                "authenticated" => "pass",
                "needs-refresh" | "unknown" | "unsupported" => "warn",
                _ => "fail",
            };
            checks.push(DiagnosticCheck {
                id: "authentication".into(),
                label: "认证状态".into(),
                status: check_status.into(),
                message: auth.message,
            });
        }

        if tool.capabilities.config {
            let existing = tool
                .config_paths
                .iter()
                .filter(|path| Path::new(path).exists())
                .count();
            checks.push(DiagnosticCheck {
                id: "configuration".into(),
                label: "配置入口".into(),
                status: if tool.config_paths.is_empty() || existing > 0 {
                    "pass"
                } else {
                    "info"
                }
                .into(),
                message: if tool.config_paths.is_empty() {
                    "配置由 CLI 官方命令管理".into()
                } else if existing > 0 {
                    format!("找到 {existing} 个已声明配置文件")
                } else {
                    "配置文件尚未创建，首次运行时可能自动生成".into()
                },
            });
        }

        let failed = checks.iter().filter(|check| check.status == "fail").count();
        let warnings = checks
            .iter()
            .filter(|check| matches!(check.status.as_str(), "warn" | "info"))
            .count();
        let status = if failed > 0 {
            "error"
        } else if warnings > 0 {
            "warning"
        } else {
            "healthy"
        };
        let summary = if failed > 0 {
            format!("发现 {failed} 项异常，{warnings} 项提醒")
        } else if warnings > 0 {
            format!("基础检查通过，有 {warnings} 项提醒")
        } else {
            format!("{} 项检查全部通过", checks.len())
        };
        let report = DiagnosticReport {
            installation: Some((&tool).into()),
            tool_id: tool.id,
            status: status.into(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            summary,
            checks,
        };
        state.storage.save_diagnostic_report(&report)?;
        Ok(report)
    })
    .await
}

fn require_tool(state: &State<'_, AppState>, id: &str) -> Result<ToolRecord, String> {
    state
        .registry
        .get(id)
        .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".into())
}

fn probe_authentication(
    tool: &ToolRecord,
    login_path: &OsStr,
) -> Result<AuthenticationStatus, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let Some(adapter_id) = tool.adapter_id.as_deref() else {
        return Ok(unsupported_auth(tool, now));
    };
    let Some(spec) = status_command(adapter_id) else {
        return Ok(unsupported_auth(tool, now));
    };
    let args: Vec<String> = spec.args.iter().map(|value| (*value).into()).collect();
    let result = match run_status_command(&tool.executable, &args, login_path) {
        Ok(result) => result,
        Err(error) => {
            return Ok(AuthenticationStatus {
                tool_id: tool.id.clone(),
                installation: Some(tool.into()),
                status: "unknown".into(),
                method: None,
                expires_in_minutes: None,
                checked_at: now,
                source: spec.source.into(),
                message: format!("认证状态查询未完成：{}", safe_message(&error)),
                fields: Vec::new(),
            });
        }
    };
    Ok(parse_authentication(
        tool,
        adapter_id,
        spec.source,
        result,
        now,
    ))
}

fn unsupported_auth(tool: &ToolRecord, checked_at: String) -> AuthenticationStatus {
    AuthenticationStatus {
        tool_id: tool.id.clone(),
        installation: Some(tool.into()),
        status: "unsupported".into(),
        method: None,
        expires_in_minutes: None,
        checked_at,
        source: "adapter".into(),
        message: "暂不支持自动检查此 CLI 的认证状态，请通过 CLI 确认登录".into(),
        fields: Vec::new(),
    }
}

fn status_command(adapter_id: &str) -> Option<StatusCommand> {
    let (args, source) = match adapter_id {
        "claude" => (&["auth", "status"][..], "claude auth status"),
        "codex" => (&["login", "status"][..], "codex login status"),
        "grok" => (&["whoami"][..], "grok whoami"),
        "lark-cli" => (&["auth", "status"][..], "lark-cli auth status"),
        "meegle" => (&["auth", "status"][..], "meegle auth status"),
        "opencode" => (&["auth", "list"][..], "opencode auth list"),
        "aws" => (
            &["sts", "get-caller-identity", "--output", "json"][..],
            "aws sts get-caller-identity",
        ),
        "cloudflared" => (
            &["tunnel", "list", "--output", "json"][..],
            "cloudflared tunnel list",
        ),
        _ => return None,
    };
    Some(StatusCommand { args, source })
}

fn parse_authentication(
    tool: &ToolRecord,
    adapter_id: &str,
    source: &str,
    result: CommandResult,
    checked_at: String,
) -> AuthenticationStatus {
    let mut status = "unknown".to_string();
    let mut method = None;
    let mut expires_in_minutes = None;
    let mut fields = Vec::new();
    let lower = result.output.to_ascii_lowercase();
    let mut message = if result.success {
        "认证状态命令已完成，但没有可识别的结构化状态".to_string()
    } else {
        "认证状态命令返回失败；原始输出未保存".to_string()
    };

    match adapter_id {
        "claude" => {
            if let Ok(json) = serde_json::from_str::<Value>(&result.output) {
                let logged_in = json
                    .get("loggedIn")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                status = if logged_in {
                    "authenticated"
                } else {
                    "unauthenticated"
                }
                .into();
                method = json
                    .get("authMethod")
                    .and_then(Value::as_str)
                    .map(humanize_token);
                message = if logged_in {
                    "Claude Code 已登录"
                } else {
                    "Claude Code 尚未登录"
                }
                .into();
            }
        }
        "codex" => {
            if lower.contains("logged in") {
                status = "authenticated".into();
                method = if lower.contains("chatgpt") {
                    Some("ChatGPT".into())
                } else {
                    None
                };
                message = "Codex 已登录".into();
            } else if lower.contains("not logged") {
                status = "unauthenticated".into();
                message = "Codex 尚未登录".into();
            }
        }
        "grok" => {
            if result.success {
                status = "authenticated".into();
                message = "Grok CLI 已配置身份".into();
            } else if lower.contains("not configured") || lower.contains("device not configured") {
                status = "unauthenticated".into();
                message = "Grok CLI 尚未完成设备配置".into();
            }
        }
        "lark-cli" => {
            if let Ok(json) = serde_json::from_str::<Value>(&result.output) {
                let identities = json.get("identities").and_then(Value::as_object);
                let mut ready = 0;
                let mut refresh = 0;
                for (key, label) in [("bot", "Bot 身份"), ("user", "用户身份")] {
                    if let Some(identity) = identities.and_then(|items| items.get(key)) {
                        let value = identity
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        ready += usize::from(value == "ready");
                        refresh += usize::from(value == "needs_refresh");
                        fields.push(StatusField {
                            label: label.into(),
                            value: humanize_token(value),
                            tone: if value == "ready" {
                                "success"
                            } else {
                                "warning"
                            }
                            .into(),
                        });
                    }
                }
                status = if ready > 0 && refresh == 0 {
                    "authenticated"
                } else if ready > 0 || refresh > 0 {
                    "needs-refresh"
                } else {
                    "unauthenticated"
                }
                .into();
                message = if status == "authenticated" {
                    "Lark CLI 身份可用"
                } else if status == "needs-refresh" {
                    "部分身份需要刷新，CLI 会在下次请求时处理"
                } else {
                    "Lark CLI 没有可用身份"
                }
                .into();
            }
        }
        "meegle" => {
            if let Ok(json) = serde_json::from_str::<Value>(&result.output) {
                let authenticated = json
                    .get("authenticated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                expires_in_minutes = json.get("expires_in_minutes").and_then(Value::as_i64);
                status = if authenticated {
                    "authenticated"
                } else {
                    "unauthenticated"
                }
                .into();
                message = if authenticated {
                    "Meegle CLI 已认证"
                } else {
                    "Meegle CLI 尚未认证"
                }
                .into();
            }
        }
        "opencode" => {
            let zero = lower.contains("0 credentials");
            status = if result.success && !zero {
                "authenticated"
            } else {
                "unauthenticated"
            }
            .into();
            message = if zero {
                "OpenCode 未配置 Provider 凭据"
            } else {
                "OpenCode 已配置 Provider 凭据"
            }
            .into();
        }
        "aws" => {
            if result.success {
                status = "authenticated".into();
                message = "AWS CLI 当前凭据有效".into();
            } else if lower.contains("expired") || lower.contains("reauthenticate") {
                status = "expired".into();
                message = "AWS CLI 会话已过期，需要重新登录".into();
            } else if lower.contains("unable to locate credentials") {
                status = "unauthenticated".into();
                message = "AWS CLI 未找到凭据".into();
            }
        }
        "cloudflared" => {
            if result.success {
                status = "authenticated".into();
                let count = serde_json::from_str::<Value>(&result.output)
                    .ok()
                    .and_then(|value| value.as_array().map(Vec::len));
                message = count
                    .map(|count| format!("Cloudflare 认证有效，可访问 {count} 个 Tunnel"))
                    .unwrap_or_else(|| "Cloudflare 认证有效".into());
            } else if lower.contains("cert.pem") || lower.contains("login") {
                status = "unauthenticated".into();
                message = "Cloudflare Tunnel 尚未登录或证书不可用".into();
            }
        }
        _ => {}
    }

    AuthenticationStatus {
        tool_id: tool.id.clone(),
        installation: Some(tool.into()),
        status,
        method,
        expires_in_minutes,
        checked_at,
        source: source.into(),
        message,
        fields,
    }
}

fn run_status_command(
    executable: &str,
    args: &[String],
    login_path: &OsStr,
) -> Result<CommandResult, String> {
    let mut child = Command::new(executable)
        .args(args)
        .env("PATH", login_path)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let status = child
        .wait_timeout(STATUS_TIMEOUT)
        .map_err(|error| error.to_string())?;
    if status.is_none() {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if status.is_none() {
        return Err("认证状态查询超过 8 秒，已安全终止".into());
    }
    let mut bytes = output.stdout;
    bytes.extend_from_slice(&output.stderr);
    if bytes.len() > OUTPUT_LIMIT {
        bytes.truncate(OUTPUT_LIMIT);
    }
    Ok(CommandResult {
        success: status.is_some_and(|status| status.success()),
        output: strip_terminal_controls(&String::from_utf8_lossy(&bytes)),
    })
}

fn strip_terminal_controls(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else if ch == '\n' || ch == '\r' || ch == '\t' || !ch.is_control() {
            result.push(ch);
        }
    }
    result
}

fn safe_message(value: &str) -> String {
    let message = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("认证状态查询失败");
    let lower = message.to_ascii_lowercase();
    if lower.contains("token") && (lower.contains('=') || lower.contains(':')) {
        "认证状态查询失败；详细信息可能包含敏感字段，已隐藏".into()
    } else {
        message.chars().take(240).collect()
    }
}

fn humanize_token(value: &str) -> String {
    value.replace(['_', '-'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeDependency, ToolCapabilities, ToolCategory, ToolSource};

    fn tool(adapter_id: &str) -> ToolRecord {
        ToolRecord {
            id: "tool".into(),
            name: adapter_id.into(),
            display_name: adapter_id.into(),
            executable: "/bin/tool".into(),
            canonical_path: "/bin/tool".into(),
            aliases: vec![adapter_id.into()],
            related_commands: vec![adapter_id.into()],
            version: Some("1.0.0".into()),
            source: ToolSource {
                kind: "user".into(),
                label: "User PATH".into(),
                package_name: None,
                package_version: None,
                scope: "user".into(),
            },
            category: ToolCategory::AiAgent,
            is_system: false,
            adapter_id: Some(adapter_id.into()),
            service: None,
            capabilities: ToolCapabilities::default(),
            config_paths: Vec::new(),
            config_preview_available: Vec::new(),
            runtime_dependencies: Vec::<RuntimeDependency>::new(),
        }
    }

    #[test]
    fn parses_claude_without_persisting_raw_json() {
        let status = parse_authentication(
            &tool("claude"),
            "claude",
            "test",
            CommandResult {
                success: true,
                output: r#"{"loggedIn":true,"authMethod":"oauth_token","secret":"hidden"}"#.into(),
            },
            "now".into(),
        );
        assert_eq!(status.status, "authenticated");
        assert_eq!(status.method.as_deref(), Some("oauth token"));
        assert!(!serde_json::to_string(&status).unwrap().contains("hidden"));
    }

    #[test]
    fn classifies_expired_aws_session() {
        let status = parse_authentication(
            &tool("aws"),
            "aws",
            "test",
            CommandResult {
                success: false,
                output: "Your session has expired. Please reauthenticate".into(),
            },
            "now".into(),
        );
        assert_eq!(status.status, "expired");
    }

    #[test]
    fn strips_ansi_sequences_from_status_output() {
        assert_eq!(strip_terminal_controls("\u{1b}[31merror\u{1b}[0m"), "error");
    }
}
