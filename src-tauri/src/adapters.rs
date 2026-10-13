use crate::models::{
    AdapterManifest, AdapterMatcher, CommandTemplate, CustomAdapterAction, CustomAdapterInfo,
    ToolCategory, ToolSource,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

fn own(args: &[&str]) -> CommandTemplate {
    CommandTemplate {
        executable: "{self}".into(),
        args: args.iter().map(|value| (*value).into()).collect(),
    }
}

fn matcher(name: &str) -> AdapterMatcher {
    AdapterMatcher {
        name: name.into(),
        path_contains: Vec::new(),
        package_names: Vec::new(),
    }
}

fn path_matcher(name: &str, path_contains: &[&str]) -> AdapterMatcher {
    AdapterMatcher {
        name: name.into(),
        path_contains: path_contains.iter().map(|value| (*value).into()).collect(),
        package_names: Vec::new(),
    }
}

pub fn config_preview_fields(
    adapter_id: &str,
    path_index: usize,
) -> Option<&'static [&'static str]> {
    if path_index != 0 {
        return None;
    }
    match adapter_id {
        "claude" => Some(&[
            "model",
            "effortLevel",
            "language",
            "autoUpdatesChannel",
            "cleanupPeriodDays",
            "permissions.defaultMode",
            "permissions.disableBypassPermissionsMode",
        ]),
        "codex" => Some(&[
            "model",
            "model_provider",
            "model_reasoning_effort",
            "model_verbosity",
            "approval_policy",
            "sandbox_mode",
            "web_search",
            "personality",
            "plan_mode_reasoning_effort",
        ]),
        "grok" => Some(&[
            "model",
            "reasoning_effort",
            "permission_mode",
            "sandbox",
            "ui.screen_mode",
            "terminal.alt_screen",
            "updates.channel",
        ]),
        "opencode" => Some(&[
            "model",
            "small_model",
            "theme",
            "default_agent",
            "autoupdate",
        ]),
        _ => None,
    }
}

pub fn builtin_adapters() -> Vec<AdapterManifest> {
    vec![
        AdapterManifest {
            id: "claude".into(),
            display_name: "Claude Code".into(),
            names: vec!["claude".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["doctor"])),
            auth: Some(own(&["auth"])),
            config_paths: vec!["~/.claude/settings.json".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "codex".into(),
            display_name: "Codex".into(),
            names: vec!["codex".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["doctor"])),
            auth: Some(own(&["login"])),
            config_paths: vec!["~/.codex/config.toml".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "grok".into(),
            display_name: "Grok CLI".into(),
            names: vec!["grok".into(), "agent".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["version".into()], vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["inspect"])),
            auth: Some(own(&["login"])),
            config_paths: vec!["~/.grok/config.toml".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![
                matcher("grok"),
                path_matcher("agent", &["/.grok/", "/grok-", "\\grok-"]),
            ],
        },
        AdapterManifest {
            id: "lark-cli".into(),
            display_name: "Lark CLI".into(),
            names: vec!["lark-cli".into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["doctor"])),
            auth: Some(own(&["auth"])),
            config_paths: vec![],
            config_command: Some(own(&["config", "show"])),
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
        AdapterManifest {
            id: "meegle".into(),
            display_name: "Meegle CLI".into(),
            names: vec!["meegle".into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["version".into()], vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["config", "show"])),
            auth: Some(own(&["auth"])),
            config_paths: vec![],
            config_command: Some(own(&["config", "show"])),
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
        AdapterManifest {
            id: "opencode".into(),
            display_name: "OpenCode".into(),
            names: vec!["opencode".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["upgrade"])),
            diagnose: Some(own(&["debug", "paths"])),
            auth: Some(own(&["providers"])),
            config_paths: vec!["~/.config/opencode/opencode.json".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "trellis".into(),
            display_name: "Trellis".into(),
            names: vec!["trellis".into(), "tl".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["upgrade"])),
            diagnose: None,
            auth: None,
            config_paths: vec![],
            config_command: None,
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
        AdapterManifest {
            id: "owl".into(),
            display_name: "Owl".into(),
            names: vec!["owl".into()],
            category: ToolCategory::Observability,
            version_commands: vec![vec!["version".into()], vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["list"])),
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: Some(own(&["config", "show"])),
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            names: vec!["cursor".into(), "code".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: None,
            auth: None,
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![
                matcher("cursor"),
                path_matcher(
                    "code",
                    &["/cursor.app/", "/programs/cursor/", "/cursor/resources/"],
                ),
            ],
        },
        AdapterManifest {
            id: "aws".into(),
            display_name: "AWS CLI".into(),
            names: vec!["aws".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: None,
            auth: Some(own(&["configure"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "cloudflared".into(),
            display_name: "Cloudflare Tunnel".into(),
            names: vec!["cloudflared".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: None,
            auth: Some(own(&["tunnel", "login"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "gh".into(),
            display_name: "GitHub CLI".into(),
            names: vec!["gh".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["auth", "status"])),
            auth: Some(own(&["auth", "login"])),
            config_paths: vec!["~/.config/gh/config.yml".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "glab".into(),
            display_name: "GitLab CLI".into(),
            names: vec!["glab".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["auth", "status"])),
            auth: Some(own(&["auth", "login"])),
            config_paths: vec!["~/.config/glab-cli/config.yml".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "gcloud".into(),
            display_name: "Google Cloud CLI".into(),
            names: vec!["gcloud".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["version".into()]],
            update: Some(own(&["components", "update"])),
            diagnose: Some(own(&["info"])),
            auth: Some(own(&["auth", "login"])),
            config_paths: vec!["~/.config/gcloud/configurations/config_default".into()],
            config_command: Some(own(&["config", "list"])),
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "az".into(),
            display_name: "Azure CLI".into(),
            names: vec!["az".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["upgrade"])),
            diagnose: Some(own(&["account", "show"])),
            auth: Some(own(&["login"])),
            config_paths: vec!["~/.azure/config".into()],
            config_command: Some(own(&["config", "get"])),
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "kubectl".into(),
            display_name: "Kubernetes CLI".into(),
            names: vec!["kubectl".into()],
            category: ToolCategory::Infrastructure,
            version_commands: vec![vec!["version".into(), "--client".into()]],
            update: None,
            diagnose: Some(own(&["cluster-info"])),
            auth: None,
            config_paths: vec!["~/.kube/config".into()],
            config_command: Some(own(&["config", "view"])),
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "docker".into(),
            display_name: "Docker CLI".into(),
            names: vec!["docker".into()],
            category: ToolCategory::Infrastructure,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["info"])),
            auth: Some(own(&["login"])),
            config_paths: vec!["~/.docker/config.json".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "terraform".into(),
            display_name: "Terraform".into(),
            names: vec!["terraform".into()],
            category: ToolCategory::Infrastructure,
            version_commands: vec![vec!["version".into()]],
            update: None,
            diagnose: None,
            auth: None,
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "helm".into(),
            display_name: "Helm".into(),
            names: vec!["helm".into()],
            category: ToolCategory::Infrastructure,
            version_commands: vec![vec!["version".into()]],
            update: None,
            diagnose: None,
            auth: None,
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "vercel".into(),
            display_name: "Vercel CLI".into(),
            names: vec!["vercel".into(), "vc".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["whoami"])),
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
        AdapterManifest {
            id: "netlify".into(),
            display_name: "Netlify CLI".into(),
            names: vec!["netlify".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["status"])),
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
        AdapterManifest {
            id: "railway".into(),
            display_name: "Railway CLI".into(),
            names: vec!["railway".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["whoami"])),
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "fly".into(),
            display_name: "Fly.io CLI".into(),
            names: vec!["fly".into(), "flyctl".into()],
            category: ToolCategory::CloudService,
            version_commands: vec![vec!["version".into()]],
            update: None,
            diagnose: Some(own(&["status"])),
            auth: Some(own(&["auth", "login"])),
            config_paths: vec!["~/.fly/config.yml".into()],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "slack".into(),
            display_name: "Slack CLI".into(),
            names: vec!["slack".into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: None,
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec![],
            matchers: vec![],
        },
        AdapterManifest {
            id: "linear".into(),
            display_name: "Linear CLI".into(),
            names: vec!["linear".into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["--version".into()]],
            update: None,
            diagnose: Some(own(&["whoami"])),
            auth: Some(own(&["init"])),
            config_paths: vec![],
            config_command: None,
            dependencies: vec!["node".into()],
            matchers: vec![],
        },
    ]
}

pub struct AdapterLoadResult {
    pub adapters: Vec<AdapterManifest>,
    pub custom_adapters: Vec<CustomAdapterInfo>,
}

struct CustomAdapterCandidate {
    path: String,
    fingerprint: String,
    manifest: Option<AdapterManifest>,
    error: Option<String>,
}

pub fn load_adapters(
    directory: &Path,
    trusted_fingerprints: &HashMap<String, String>,
) -> AdapterLoadResult {
    let builtins = builtin_adapters();
    let builtin_ids: HashSet<_> = builtins.iter().map(|adapter| adapter.id.clone()).collect();
    let builtin_names: HashSet<_> = builtins
        .iter()
        .flat_map(|adapter| adapter.names.iter().cloned())
        .collect();
    let _ = fs::create_dir_all(directory);
    let Ok(entries) = fs::read_dir(directory) else {
        return AdapterLoadResult {
            adapters: builtins,
            custom_adapters: Vec::new(),
        };
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.path());
    let mut candidates = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let display_path = fs::canonicalize(&path)
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 131_072 {
            candidates.push(CustomAdapterCandidate {
                path: display_path,
                fingerprint: String::new(),
                manifest: None,
                error: Some("文件不是普通 JSON 文件或超过 128 KiB 安全上限".into()),
            });
            continue;
        }
        let Ok(content) = fs::read(&path) else {
            continue;
        };
        let fingerprint = sha256_hex(&content);
        let parsed = std::str::from_utf8(&content)
            .map_err(|_| "文件不是有效 UTF-8".to_string())
            .and_then(|json| {
                serde_json::from_str::<AdapterManifest>(json)
                    .map_err(|error| format!("JSON 无效：{error}"))
            })
            .and_then(|manifest| {
                validate_manifest(&manifest)
                    .then_some(manifest)
                    .ok_or_else(|| "适配器字段未通过安全校验".to_string())
            });
        match parsed {
            Ok(manifest) => candidates.push(CustomAdapterCandidate {
                path: display_path,
                fingerprint,
                manifest: Some(manifest),
                error: None,
            }),
            Err(error) => candidates.push(CustomAdapterCandidate {
                path: display_path,
                fingerprint,
                manifest: None,
                error: Some(error),
            }),
        }
    }

    let mut id_counts: HashMap<String, usize> = HashMap::new();
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for manifest in candidates
        .iter()
        .filter_map(|candidate| candidate.manifest.as_ref())
    {
        *id_counts.entry(manifest.id.clone()).or_default() += 1;
        for name in &manifest.names {
            *name_counts.entry(name.clone()).or_default() += 1;
        }
    }

    let mut adapters = builtins;
    let mut custom_adapters = Vec::new();
    for candidate in candidates {
        let Some(manifest) = candidate.manifest else {
            let has_stored_trust = trusted_fingerprints.contains_key(&candidate.path);
            custom_adapters.push(CustomAdapterInfo {
                id: "invalid".into(),
                display_name: Path::new(&candidate.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("无效适配器")
                    .into(),
                path: candidate.path,
                fingerprint: candidate.fingerprint,
                status: "invalid".into(),
                message: candidate.error.unwrap_or_else(|| "文件无效".into()),
                has_stored_trust,
                actions: Vec::new(),
            });
            continue;
        };
        let conflicts_with_builtin = builtin_ids.contains(manifest.id.as_str())
            || manifest
                .names
                .iter()
                .any(|name| builtin_names.contains(name.as_str()));
        let conflicts_with_custom = id_counts.get(manifest.id.as_str()).copied().unwrap_or(0) > 1
            || manifest
                .names
                .iter()
                .any(|name| name_counts.get(name.as_str()).copied().unwrap_or(0) > 1);
        let actions = adapter_actions(&manifest);
        let (status, message) = if conflicts_with_builtin {
            ("blocked", "ID 或命令名与内置适配器冲突，不能覆盖内置定义")
        } else if conflicts_with_custom {
            ("blocked", "ID 或命令名与另一个自定义适配器冲突")
        } else if trusted_fingerprints
            .get(&candidate.path)
            .map(|fingerprint| fingerprint == &candidate.fingerprint)
            .unwrap_or(false)
        {
            adapters.push(manifest.clone());
            ("trusted", "内容指纹已信任，适配器已启用")
        } else if trusted_fingerprints.contains_key(&candidate.path) {
            ("pending", "文件内容在上次信任后发生变化，需要重新审查")
        } else {
            ("pending", "尚未信任，不会加载或执行任何适配器动作")
        };
        let has_stored_trust = trusted_fingerprints.contains_key(&candidate.path);
        custom_adapters.push(CustomAdapterInfo {
            id: manifest.id,
            display_name: manifest.display_name,
            path: candidate.path,
            fingerprint: candidate.fingerprint,
            status: status.into(),
            message: message.into(),
            has_stored_trust,
            actions,
        });
    }
    AdapterLoadResult {
        adapters,
        custom_adapters,
    }
}

fn sha256_hex(content: &[u8]) -> String {
    Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn adapter_actions(manifest: &AdapterManifest) -> Vec<CustomAdapterAction> {
    let mut actions = Vec::new();
    for args in &manifest.version_commands {
        actions.push(CustomAdapterAction {
            name: "版本探测".into(),
            command: display_command("{self}", args),
        });
    }
    for (name, command) in [
        ("更新", manifest.update.as_ref()),
        ("诊断", manifest.diagnose.as_ref()),
        ("登录", manifest.auth.as_ref()),
        ("显示配置", manifest.config_command.as_ref()),
    ] {
        if let Some(command) = command {
            actions.push(CustomAdapterAction {
                name: name.into(),
                command: display_command(&command.executable, &command.args),
            });
        }
    }
    actions
}

fn display_command(executable: &str, args: &[String]) -> String {
    std::iter::once(executable)
        .chain(args.iter().map(String::as_str))
        .map(|value| {
            if value.chars().all(|character| {
                character.is_ascii_alphanumeric() || "/._@:+-={}\\".contains(character)
            }) {
                value.to_string()
            } else {
                format!("\"{}\"", value.replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_manifest(manifest: &AdapterManifest) -> bool {
    let valid_name = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".@+_-".contains(character))
    };
    if !valid_name(&manifest.id)
        || manifest.display_name.trim().is_empty()
        || manifest.names.is_empty()
        || !manifest.names.iter().all(|name| valid_name(name))
    {
        return false;
    }
    let unique: HashSet<_> = manifest.names.iter().collect();
    if unique.len() != manifest.names.len() {
        return false;
    }
    if !manifest.dependencies.iter().all(|name| valid_name(name))
        || !manifest.matchers.iter().all(|matcher| {
            valid_name(&matcher.name)
                && manifest.names.contains(&matcher.name)
                && matcher.path_contains.len() <= 16
                && matcher.package_names.len() <= 16
                && matcher
                    .path_contains
                    .iter()
                    .all(|value| !value.is_empty() && value.len() <= 512 && !value.contains('\0'))
                && matcher.package_names.iter().all(|value| valid_name(value))
        })
    {
        return false;
    }
    manifest
        .update
        .iter()
        .chain(manifest.diagnose.iter())
        .chain(manifest.auth.iter())
        .chain(manifest.config_command.iter())
        .all(valid_command)
        && manifest
            .version_commands
            .iter()
            .all(|args| valid_args(args))
}

fn valid_command(command: &CommandTemplate) -> bool {
    !command.executable.is_empty()
        && !command.executable.contains('\0')
        && command.executable.len() <= 1024
        && valid_args(&command.args)
}

fn valid_args(args: &[String]) -> bool {
    args.len() <= 128
        && args
            .iter()
            .all(|arg| !arg.contains('\0') && arg.len() <= 8192)
}

pub fn find_adapter<'a>(
    adapters: &'a [AdapterManifest],
    names: &[String],
    executable: &Path,
    source: &ToolSource,
) -> Option<&'a AdapterManifest> {
    let normalized_path = executable
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    adapters.iter().find(|adapter| {
        if adapter.matchers.is_empty() {
            return adapter
                .names
                .iter()
                .any(|candidate| names.iter().any(|name| name == candidate));
        }
        adapter.matchers.iter().any(|matcher| {
            if !names.iter().any(|name| name == &matcher.name) {
                return false;
            }
            let path_matches = matcher.path_contains.is_empty()
                || matcher.path_contains.iter().any(|hint| {
                    normalized_path.contains(&hint.replace('\\', "/").to_ascii_lowercase())
                });
            let package_matches = matcher.package_names.is_empty()
                || source
                    .package_name
                    .as_ref()
                    .map(|package| {
                        matcher
                            .package_names
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(package))
                    })
                    .unwrap_or(false);
            path_matches && package_matches
        })
    })
}

pub fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn custom_manifest(id: &str, name: &str) -> AdapterManifest {
        AdapterManifest {
            id: id.into(),
            display_name: "Custom Tool".into(),
            names: vec![name.into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["--version".into()]],
            update: Some(CommandTemplate {
                executable: "{self}".into(),
                args: vec!["update".into()],
            }),
            diagnose: None,
            auth: None,
            config_paths: vec!["~/.config/custom/config.json".into()],
            config_command: None,
            dependencies: Vec::new(),
            matchers: Vec::new(),
        }
    }

    #[test]
    fn rejects_shell_like_adapter_names_and_nul_arguments() {
        let mut manifest = builtin_adapters().remove(0);
        manifest.id = "bad;adapter".into();
        assert!(!validate_manifest(&manifest));
        manifest.id = "safe-adapter".into();
        manifest.update = Some(CommandTemplate {
            executable: "npm".into(),
            args: vec!["bad\0arg".into()],
        });
        assert!(!validate_manifest(&manifest));
    }

    #[test]
    fn custom_adapter_requires_exact_fingerprint_trust() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("custom.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&custom_manifest("custom-tool", "custom-tool")).unwrap(),
        )
        .unwrap();

        let pending = load_adapters(temp.path(), &HashMap::new());
        let info = pending.custom_adapters.first().unwrap();
        assert_eq!(info.status, "pending");
        assert!(!pending
            .adapters
            .iter()
            .any(|adapter| adapter.id == "custom-tool"));

        let trusted = HashMap::from([(info.path.clone(), info.fingerprint.clone())]);
        let loaded = load_adapters(temp.path(), &trusted);
        assert_eq!(loaded.custom_adapters[0].status, "trusted");
        assert!(loaded
            .adapters
            .iter()
            .any(|adapter| adapter.id == "custom-tool"));

        let mut changed = custom_manifest("custom-tool", "custom-tool");
        changed.display_name = "Changed Tool".into();
        fs::write(&path, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
        let changed = load_adapters(temp.path(), &trusted);
        assert_eq!(changed.custom_adapters[0].status, "pending");
        assert!(!changed
            .adapters
            .iter()
            .any(|adapter| adapter.id == "custom-tool"));
    }

    #[test]
    fn custom_adapter_cannot_shadow_builtin_id_or_command() {
        let temp = TempDir::new().unwrap();
        for (file, manifest) in [
            ("id.json", custom_manifest("claude", "custom-claude")),
            ("name.json", custom_manifest("custom-claude", "claude")),
        ] {
            fs::write(
                temp.path().join(file),
                serde_json::to_vec_pretty(&manifest).unwrap(),
            )
            .unwrap();
        }
        let loaded = load_adapters(temp.path(), &HashMap::new());
        assert_eq!(loaded.custom_adapters.len(), 2);
        assert!(loaded
            .custom_adapters
            .iter()
            .all(|adapter| adapter.status == "blocked"));
        assert_eq!(
            loaded
                .adapters
                .iter()
                .filter(|adapter| adapter.id == "claude")
                .count(),
            1
        );
    }

    #[test]
    fn infrastructure_adapters_never_enter_the_default_agent_inventory() {
        let adapters = builtin_adapters();
        for id in ["kubectl", "docker", "terraform", "helm"] {
            let adapter = adapters
                .iter()
                .find(|adapter| adapter.id == id)
                .unwrap_or_else(|| panic!("missing built-in adapter {id}"));
            assert_eq!(adapter.category, ToolCategory::Infrastructure, "{id}");
        }
    }

    #[test]
    fn package_manager_updates_are_inferred_from_the_installation_source() {
        let adapters = builtin_adapters();
        for id in ["meegle", "gh", "glab", "vercel", "netlify", "linear"] {
            let adapter = adapters
                .iter()
                .find(|adapter| adapter.id == id)
                .unwrap_or_else(|| panic!("missing built-in adapter {id}"));
            assert!(adapter.update.is_none(), "{id} hardcodes a package manager");
        }
    }

    #[test]
    fn credential_only_files_are_not_declared_as_configuration() {
        for adapter in builtin_adapters() {
            assert!(adapter.config_paths.iter().all(|path| {
                let path = path.to_ascii_lowercase();
                !path.ends_with("auth.json")
                    && !path.ends_with("credentials")
                    && !path.ends_with("token.json")
            }));
        }
    }
}
