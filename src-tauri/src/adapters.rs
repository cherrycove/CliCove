use crate::models::{AdapterManifest, CommandTemplate, ToolCategory};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn own(args: &[&str]) -> CommandTemplate {
    CommandTemplate {
        executable: "{self}".into(),
        args: args.iter().map(|value| (*value).into()).collect(),
    }
}

fn external(executable: &str, args: &[&str]) -> CommandTemplate {
    CommandTemplate {
        executable: executable.into(),
        args: args.iter().map(|value| (*value).into()).collect(),
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
        },
        AdapterManifest {
            id: "meegle".into(),
            display_name: "Meegle CLI".into(),
            names: vec!["meegle".into()],
            category: ToolCategory::Collaboration,
            version_commands: vec![vec!["version".into()], vec!["--version".into()]],
            update: Some(external(
                "npm",
                &["install", "-g", "@lark-project/meegle@latest"],
            )),
            diagnose: Some(own(&["config", "show"])),
            auth: Some(own(&["auth"])),
            config_paths: vec![],
            config_command: Some(own(&["config", "show"])),
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
        },
        AdapterManifest {
            id: "owl".into(),
            display_name: "Owl".into(),
            names: vec!["owl".into()],
            category: ToolCategory::AiAgent,
            version_commands: vec![vec!["version".into()], vec!["--version".into()]],
            update: Some(own(&["update"])),
            diagnose: Some(own(&["list"])),
            auth: Some(own(&["login"])),
            config_paths: vec![],
            config_command: Some(own(&["config", "show"])),
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
        },
    ]
}

pub fn load_adapters(directory: &Path) -> Vec<AdapterManifest> {
    let mut adapters = builtin_adapters();
    let _ = fs::create_dir_all(directory);
    let Ok(entries) = fs::read_dir(directory) else {
        return adapters;
    };
    let mut custom = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 131_072 {
            continue;
        }
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<AdapterManifest>(&content) else {
            continue;
        };
        if validate_manifest(&manifest) {
            custom.push(manifest);
        }
    }
    custom.sort_by(|left, right| left.id.cmp(&right.id));
    for manifest in custom {
        if let Some(index) = adapters.iter().position(|item| item.id == manifest.id) {
            adapters[index] = manifest;
        } else {
            adapters.push(manifest);
        }
    }
    adapters
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
) -> Option<&'a AdapterManifest> {
    adapters.iter().find(|adapter| {
        adapter
            .names
            .iter()
            .any(|candidate| names.iter().any(|name| name == candidate))
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
}
