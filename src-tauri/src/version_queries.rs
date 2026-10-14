//! Reviewed read-only release queries. Never infer `update --check` from a name:
//! some updaters ignore unknown flags or install while checking.
use crate::models::ToolRecord;
use crate::process::background_command;
use crate::scanner::ToolRegistry;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

const LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const CLAUDE_RELEASES: &str = "https://downloads.claude.ai/claude-code-releases";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parser {
    TextVersion,
    JsonVersion(&'static str),
    SelfUpdateJson,
    GcloudSdk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryKind {
    Npm(String),
    Brew(String),
    Native(Vec<&'static str>, Parser),
    Http(String, Parser),
}

#[derive(Debug, Clone)]
pub struct Query {
    pub label: String,
    pub kind: QueryKind,
}

fn http(label: impl Into<String>, url: impl Into<String>, parser: Parser) -> Query {
    Query {
        label: label.into(),
        kind: QueryKind::Http(url.into(), parser),
    }
}

fn native(label: &str, args: &[&'static str], parser: Parser) -> Query {
    Query {
        label: label.into(),
        kind: QueryKind::Native(args.to_vec(), parser),
    }
}

pub fn plan(tool: &ToolRecord) -> Result<Vec<Query>, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法确定用户目录".to_string())?;
    plan_with_config(tool, &home, managed_claude_settings().as_deref())
}

fn plan_with_config(
    tool: &ToolRecord,
    home: &Path,
    managed: Option<&Path>,
) -> Result<Vec<Query>, String> {
    // Package-managed installations must compare against their own channel, not
    // a possibly newer upstream release that the package manager cannot install.
    if let Some(package) = tool.source.package_name.as_ref() {
        match tool.source.kind.as_str() {
            "npm" => {
                return Ok(vec![Query {
                    label: format!("npm registry · {package}"),
                    kind: QueryKind::Npm(package.clone()),
                }])
            }
            "homebrew" => {
                return Ok(vec![Query {
                    label: format!("Homebrew · {package}"),
                    kind: QueryKind::Brew(package.clone()),
                }])
            }
            _ => {}
        }
    }
    let identity = tool
        .adapter_id
        .as_deref()
        .or_else(|| tool.service.as_ref().map(|service| service.id.as_str()));
    Ok(match identity {
        Some("grok") => vec![native(
            "Grok 官方更新检查",
            &["update", "--check", "--json"],
            Parser::SelfUpdateJson,
        )],
        Some("gcloud") => vec![native(
            "Google Cloud 官方组件检查",
            &[
                "components",
                "list",
                "--show-versions",
                "--filter=id:core",
                "--format=json",
            ],
            Parser::GcloudSdk,
        )],
        Some("claude") => {
            let channel = claude_channel(home, managed)?;
            vec![http(
                format!("Claude 官方发布渠道 · {channel}"),
                format!("{CLAUDE_RELEASES}/{channel}"),
                Parser::TextVersion,
            )]
        }
        Some("owl") => {
            let url = owl_manifest(home)?;
            vec![http(
                "Owl 安装渠道发布清单",
                url,
                Parser::JsonVersion("/version"),
            )]
        }
        Some("codex") => vec![http(
            if tool.source.kind == "application" {
                "Codex CLI 官方包发布版本（应用随附，仅供参考）"
            } else {
                "Codex CLI 官方包发布版本"
            },
            "https://registry.npmjs.org/@openai%2Fcodex/latest",
            Parser::JsonVersion("/version"),
        )],
        _ => Vec::new(),
    })
}

fn managed_claude_settings() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return Some(PathBuf::from(
        "/Library/Application Support/ClaudeCode/managed-settings.json",
    ));
    #[cfg(target_os = "windows")]
    return std::env::var_os("ProgramData")
        .map(|path| PathBuf::from(path).join("ClaudeCode/managed-settings.json"));
    #[cfg(all(unix, not(target_os = "macos")))]
    return Some(PathBuf::from("/etc/claude-code/managed-settings.json"));
}

fn read_config(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取更新渠道配置".into()),
        Ok(metadata) if !metadata.is_file() || metadata.len() > LIMIT as u64 => {
            return Err("更新渠道配置不是普通文件或超过大小限制".into())
        }
        _ => {}
    }
    let file = fs::File::open(path).map_err(|_| "无法读取更新渠道配置".to_string())?;
    read_bounded(file).map(Some)
}

fn claude_channel(home: &Path, managed: Option<&Path>) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Settings {
        auto_updates_channel: Option<String>,
    }
    let mut channel = "latest".to_string();
    let user = home.join(".claude/settings.json");
    for path in std::iter::once(user.as_path()).chain(managed) {
        if let Some(bytes) = read_config(path)? {
            let settings: Settings = serde_json::from_slice(&bytes)
                .map_err(|_| "Claude 更新渠道配置格式无效".to_string())?;
            if let Some(value) = settings.auto_updates_channel {
                channel = value;
            }
        }
    }
    match channel.as_str() {
        "latest" | "stable" => Ok(channel),
        _ => Err("Claude 更新渠道无法识别；未改用其他渠道".into()),
    }
}

fn owl_manifest(home: &Path) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Config {
        update: Option<Update>,
    }
    #[derive(Deserialize)]
    struct Update {
        base_url: Option<String>,
    }
    let bytes = read_config(&home.join(".owl/config.yaml"))?
        .ok_or_else(|| "Owl 缺少安装渠道配置，无法确定对应发布源".to_string())?;
    let config: Config =
        serde_yaml_ng::from_slice(&bytes).map_err(|_| "Owl 更新渠道配置格式无效".to_string())?;
    let base = config
        .update
        .and_then(|update| update.base_url)
        .ok_or_else(|| "Owl 未配置 update.base_url，无法确定对应发布源".to_string())?;
    // Do not issue arbitrary requests to config-supplied URLs, or switch a
    // regional/private installation to another distribution's channel.
    match base.trim_end_matches('/') {
        "https://static.guance.com/owl" | "https://static.truewatch.com/owl" => {
            Ok(format!("{}/version", base.trim_end_matches('/')))
        }
        _ => Err("Owl 使用自定义安装渠道，尚未验证其只读版本清单；未切换渠道".into()),
    }
}

pub fn execute(
    query: &Query,
    registry: &ToolRegistry,
    tool: &ToolRecord,
) -> Result<String, String> {
    match &query.kind {
        QueryKind::Native(args, parser) => {
            let (stdout, stderr) =
                command_output(&tool.executable, args, &registry.execution_path(), TIMEOUT)?;
            parse(parser, &stdout, &stderr)
        }
        QueryKind::Http(url, parser) => {
            let body = get_release(url)?;
            parse(parser, &body, "")
        }
        _ => Err("包管理器查询应交由安装来源处理".into()),
    }
}

fn get_release(url: &str) -> Result<String, String> {
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(8))
        .timeout(TIMEOUT)
        .user_agent(concat!("CliCove/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "无法初始化版本查询连接".to_string())?;
    let response = client.get(url).send().map_err(|error| {
        if error.is_timeout() {
            "版本查询超时".to_string()
        } else {
            "无法连接发布源，请检查网络或代理".to_string()
        }
    })?;
    if !response.status().is_success() {
        return Err(format!("发布源返回 HTTP {}", response.status().as_u16()));
    }
    let bytes = read_bounded(response)?;
    String::from_utf8(bytes).map_err(|_| "发布源返回了无效文本".to_string())
}

fn read_bounded(mut reader: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "读取版本数据失败".to_string())?;
    if bytes.len() > LIMIT {
        return Err("版本数据超过大小限制".into());
    }
    Ok(bytes)
}

// Drain both pipes concurrently: gcloud writes the SDK version to stderr and
// component versions to stdout. Waiting before draining can deadlock on a full pipe.
fn command_output(
    executable: &str,
    args: &[&str],
    path: &OsStr,
    timeout: Duration,
) -> Result<(String, String), String> {
    let deadline = Instant::now() + timeout;
    let mut child = background_command(executable)
        .args(args)
        .env("PATH", path)
        .env("LC_ALL", "C")
        .env("NO_COLOR", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("OWL_NO_UPDATE_CHECK", "1")
        .env("CLOUDSDK_CORE_DISABLE_USAGE_REPORTING", "true")
        // The SDK's component-list command can offer a self-reinstall on a
        // future manifest schema. Its reinstallation guard makes that path fail
        // before downloading or replacing components, even with quiet enabled.
        .env("CLOUDSDK_REINSTALL_COMPONENTS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "无法启动官方版本检查命令".to_string())?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let copy = sender.clone();
    std::thread::spawn(move || {
        let _ = copy.send((true, read_bounded(stdout)));
    });
    std::thread::spawn(move || {
        let _ = sender.send((false, read_bounded(stderr)));
    });
    let status = child
        .wait_timeout(timeout)
        .map_err(|_| "无法等待版本检查命令".to_string());
    let status = match status {
        Ok(Some(status)) => status,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("官方版本检查超时".into());
        }
    };
    if !status.success() {
        return Err(format!(
            "官方版本检查退出异常（{}）",
            status.code().unwrap_or(-1)
        ));
    }
    let mut output = (String::new(), String::new());
    for _ in 0..2 {
        let (stdout, bytes) = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| "读取官方版本结果超时".to_string())?;
        let text = String::from_utf8(bytes?).map_err(|_| "官方版本结果不是有效文本".to_string())?;
        if stdout {
            output.0 = text;
        } else {
            output.1 = text;
        }
    }
    Ok(output)
}

pub fn parse(parser: &Parser, stdout: &str, stderr: &str) -> Result<String, String> {
    let candidate = match parser {
        Parser::TextVersion => stdout.trim().to_string(),
        Parser::JsonVersion(pointer) => {
            let json: serde_json::Value =
                serde_json::from_str(stdout).map_err(|_| "发布源没有返回有效 JSON".to_string())?;
            json.pointer(pointer)
                .and_then(|value| value.as_str())
                .ok_or_else(|| "发布源缺少版本字段".to_string())?
                .to_string()
        }
        Parser::SelfUpdateJson => {
            let json: serde_json::Value = serde_json::from_str(stdout)
                .map_err(|_| "官方检查命令没有返回有效 JSON".to_string())?;
            json.get("latestVersion")
                .or_else(|| json.get("latest_version"))
                .and_then(|value| value.as_str())
                .ok_or_else(|| "官方检查命令缺少最新版字段".to_string())?
                .to_string()
        }
        Parser::GcloudSdk => {
            // A component date (e.g. 2026.08.31) is not the SDK release (583.0.0).
            stdout
                .lines()
                .chain(stderr.lines())
                .find_map(|line| {
                    line.trim()
                        .strip_prefix("The latest available version is: ")
                })
                .ok_or_else(|| "组件结果未提供 SDK 最新版本（可能固定了安装版本）".to_string())?
                .trim()
                .to_string()
        }
    };
    validate_version(&candidate)
}

fn validate_version(value: &str) -> Result<String, String> {
    if value.len() > 128 || semver::Version::parse(value.trim_start_matches('v')).is_err() {
        return Err("发布源版本格式无效".into());
    }
    Ok(value.to_string())
}

pub fn try_queries(
    queries: &[Query],
    mut run: impl FnMut(&Query) -> Result<String, String>,
) -> Result<(String, String, bool), String> {
    let mut errors = Vec::new();
    for (index, query) in queries.iter().enumerate() {
        match run(query) {
            Ok(version) => return Ok((version, query.label.clone(), index > 0)),
            Err(error) => errors.push(format!("{}：{error}", query.label)),
        }
    }
    Err(errors.join("；"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tool(id: &str, kind: &str, package: Option<&str>) -> ToolRecord {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "displayName": id, "executable": "/test/cli",
            "canonicalPath": "/test/cli", "aliases": [id], "relatedCommands": [id],
            "source": { "kind": kind, "label": kind, "packageName": package, "scope": "global" },
            "category": "cloud-service", "isSystem": false, "adapterId": id,
            "capabilities": { "terminal": true, "versionProbe": true, "update": true,
                "diagnose": false, "auth": false, "config": false }, "configPaths": []
        }))
        .unwrap()
    }

    #[test]
    fn native_plans_only_use_reviewed_read_only_commands() {
        let temp = TempDir::new().unwrap();
        let grok = plan_with_config(&tool("grok", "user", None), temp.path(), None).unwrap();
        assert_eq!(
            grok[0].kind,
            QueryKind::Native(vec!["update", "--check", "--json"], Parser::SelfUpdateJson)
        );
        let gcloud = plan_with_config(&tool("gcloud", "user", None), temp.path(), None).unwrap();
        assert!(
            matches!(&gcloud[0].kind, QueryKind::Native(args, _) if args[0..2] == ["components", "list"])
        );
        let claude = plan_with_config(&tool("claude", "user", None), temp.path(), None).unwrap();
        assert!(matches!(&claude[0].kind, QueryKind::Http(url, _) if url.ends_with("/latest")));
        assert!(
            plan_with_config(&tool("unknown", "user", None), temp.path(), None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn package_channel_takes_precedence_over_native_upstream() {
        let temp = TempDir::new().unwrap();
        assert_eq!(
            plan_with_config(
                &tool("claude", "npm", Some("@anthropic-ai/claude-code")),
                temp.path(),
                None
            )
            .unwrap()[0]
                .kind,
            QueryKind::Npm("@anthropic-ai/claude-code".into())
        );
        assert_eq!(
            plan_with_config(&tool("grok", "homebrew", Some("grok")), temp.path(), None).unwrap()
                [0]
            .kind,
            QueryKind::Brew("grok".into())
        );
    }

    #[test]
    fn claude_respects_user_and_managed_release_channels_without_returning_secrets() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join(".claude")).unwrap();
        let user = temp.path().join(".claude/settings.json");
        fs::write(
            &user,
            r#"{"autoUpdatesChannel":"stable","env":{"API_KEY":"secret"}}"#,
        )
        .unwrap();
        assert_eq!(claude_channel(temp.path(), None).unwrap(), "stable");
        let managed = temp.path().join("managed.json");
        fs::write(&managed, r#"{"autoUpdatesChannel":"latest"}"#).unwrap();
        assert_eq!(
            claude_channel(temp.path(), Some(&managed)).unwrap(),
            "latest"
        );
        fs::write(&user, r#"{"autoUpdatesChannel":"unknown-secret-channel"}"#).unwrap();
        let error = claude_channel(temp.path(), None).unwrap_err();
        assert!(!error.contains("secret"));
    }

    #[test]
    fn owl_uses_only_the_configured_reviewed_distribution() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join(".owl")).unwrap();
        let config = temp.path().join(".owl/config.yaml");
        for host in ["static.guance.com", "static.truewatch.com"] {
            fs::write(
                &config,
                format!(
                    "token: secret\nupdate:\n    base_url: https://{host}/owl/\n    check: false\n"
                ),
            )
            .unwrap();
            assert_eq!(
                owl_manifest(temp.path()).unwrap(),
                format!("https://{host}/owl/version")
            );
        }
        for base in [
            "http://127.0.0.1/private",
            "https://static.guance.com.evil/owl",
            "https://static.guance.com/owl?token=secret",
        ] {
            fs::write(&config, format!("update:\n  base_url: '{base}'\n")).unwrap();
            let error = owl_manifest(temp.path()).unwrap_err();
            assert!(!error.contains(base));
        }
    }

    #[test]
    fn gcloud_parser_uses_sdk_release_not_component_date_or_installed_version() {
        assert_eq!(parse(&Parser::GcloudSdk, r#"[{"latest_version_string":"2026.08.31"}]"#,
            "Your current Google Cloud CLI version is: 580.0.0\nThe latest available version is: 583.0.0\n").unwrap(), "583.0.0");
        assert!(parse(
            &Parser::GcloudSdk,
            r#"[{"latest_version_string":"2026.08.31"}]"#,
            "Your current Google Cloud CLI version is: 580.0.0"
        )
        .is_err());
    }

    #[test]
    fn release_parsers_reject_missing_malformed_and_nonversion_responses() {
        assert_eq!(
            parse(
                &Parser::SelfUpdateJson,
                r#"{"latest_version":"v1.2.3"}"#,
                ""
            )
            .unwrap(),
            "v1.2.3"
        );
        assert_eq!(
            parse(
                &Parser::JsonVersion("/version"),
                r#"{"version":"1.2.3"}"#,
                ""
            )
            .unwrap(),
            "1.2.3"
        );
        for response in ["<html>1.2.3</html>", "version 1.2.3", "", "1.2.3;command"] {
            assert!(parse(&Parser::TextVersion, response, "").is_err());
        }
        assert!(parse(&Parser::SelfUpdateJson, r#"{"currentVersion":"1.2.3"}"#, "").is_err());
        assert!(parse(
            &Parser::JsonVersion("/version"),
            r#"{"error":"unauthorized"}"#,
            ""
        )
        .is_err());
        assert!(read_bounded(vec![0; LIMIT + 1].as_slice()).is_err());
    }

    #[test]
    fn query_chain_tries_supported_fallback_and_records_actual_source() {
        let queries = vec![
            http("primary", "https://example.com/a", Parser::TextVersion),
            http("fallback", "https://example.com/b", Parser::TextVersion),
        ];
        let mut attempts = 0;
        let result = try_queries(&queries, |_| {
            attempts += 1;
            if attempts == 1 {
                Err("timeout".into())
            } else {
                Ok("1.2.3".into())
            }
        })
        .unwrap();
        assert_eq!(result, ("1.2.3".into(), "fallback".into(), true));
        let error = try_queries(&queries, |_| Err("offline".into())).unwrap_err();
        assert!(error.contains("primary") && error.contains("fallback"));
    }

    #[cfg(unix)]
    #[test]
    fn command_capture_drains_both_pipes_and_times_out() {
        let (guard, _) = command_output(
            "/bin/sh",
            &["-c", "printf '%s' \"$CLOUDSDK_REINSTALL_COMPONENTS\""],
            OsStr::new("/usr/bin:/bin"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(guard, "1");
        let (stdout, stderr) = command_output(
            "/bin/sh",
            &["-c", "printf '{\"ok\":true}'; printf 'SDK 1.2.3' >&2"],
            OsStr::new("/usr/bin:/bin"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(stdout, r#"{"ok":true}"#);
        assert_eq!(stderr, "SDK 1.2.3");
        let large = command_output(
            "/bin/sh",
            &[
                "-c",
                "head -c 100000 /dev/zero; head -c 100000 /dev/zero >&2",
            ],
            OsStr::new("/usr/bin:/bin"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(large.0.len(), 100000);
        assert_eq!(large.1.len(), 100000);
        assert!(command_output(
            "/bin/sleep",
            &["2"],
            OsStr::new("/usr/bin:/bin"),
            Duration::from_millis(30)
        )
        .is_err());
    }

    #[test]
    #[ignore = "explicit live, read-only release-source verification"]
    fn live_installed_release_queries() {
        let registry = ToolRegistry::default();
        let home = dirs::home_dir().unwrap();
        for id in ["claude", "owl", "codex", "gcloud"] {
            let mut tool = tool(id, if id == "codex" { "application" } else { "user" }, None);
            if id == "gcloud" {
                tool.executable = std::env::var("CLIHUB_TEST_GCLOUD_PATH")
                    .expect("set CLIHUB_TEST_GCLOUD_PATH to the installed gcloud entry");
            }
            let queries =
                plan_with_config(&tool, &home, managed_claude_settings().as_deref()).unwrap();
            let result = try_queries(&queries, |query| execute(query, &registry, &tool)).unwrap();
            println!("{id}: {} · {}", result.0, result.1);
        }
    }
}
