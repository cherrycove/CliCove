use crate::adapters::{builtin_adapters, expand_home, find_adapter};
use crate::models::{AdapterManifest, ToolCapabilities, ToolCategory, ToolRecord, ToolSource};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
#[cfg(windows)]
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use uuid::Uuid;
use wait_timeout::ChildExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub struct ToolRegistry {
    adapters: Vec<AdapterManifest>,
    tools: RwLock<HashMap<String, ToolRecord>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new(builtin_adapters())
    }
}

impl ToolRegistry {
    pub fn new(adapters: Vec<AdapterManifest>) -> Self {
        Self {
            adapters,
            tools: RwLock::new(HashMap::new()),
        }
    }
    pub fn scan(&self, show_system: bool) -> Vec<ToolRecord> {
        let homebrew_leaves = homebrew_leaves();
        let mut grouped: BTreeMap<String, (PathBuf, Vec<String>, bool)> = BTreeMap::new();
        for directory in login_path_directories() {
            let system_directory = is_system_directory(&directory);
            if system_directory && !show_system {
                continue;
            }
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !is_executable(&path) {
                    continue;
                }
                let Some(name) = command_name(&path) else {
                    continue;
                };
                let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                let key = canonical.to_string_lossy().to_string();
                let value = grouped
                    .entry(key)
                    .or_insert_with(|| (canonical.clone(), Vec::new(), system_directory));
                if !value.1.contains(&name) {
                    value.1.push(name);
                }
                value.2 &= system_directory;
            }
        }

        let mut records: Vec<_> = grouped
            .into_iter()
            .map(|(_, (canonical, mut aliases, is_system))| {
                aliases.sort();
                self.make_record(canonical, aliases, is_system, &homebrew_leaves)
            })
            .collect();

        records.sort_by(|left, right| {
            category_rank(&left.category)
                .cmp(&category_rank(&right.category))
                .then_with(|| {
                    left.display_name
                        .to_lowercase()
                        .cmp(&right.display_name.to_lowercase())
                })
        });
        let map = records
            .iter()
            .cloned()
            .map(|tool| (tool.id.clone(), tool))
            .collect();
        *self.tools.write() = map;
        records
    }

    pub fn get(&self, id: &str) -> Option<ToolRecord> {
        self.tools.read().get(id).cloned()
    }

    pub fn find_by_name(&self, name: &str) -> Option<ToolRecord> {
        self.tools
            .read()
            .values()
            .find(|tool| tool.aliases.iter().any(|alias| alias == name))
            .cloned()
    }

    pub fn adapter_for(&self, tool: &ToolRecord) -> Option<AdapterManifest> {
        tool.adapter_id
            .as_ref()
            .and_then(|id| self.adapters.iter().find(|adapter| &adapter.id == id))
            .cloned()
    }

    pub fn probe(&self, id: &str) -> Result<String, String> {
        let tool = self
            .get(id)
            .ok_or_else(|| "工具不在当前扫描清单中，请先刷新".to_string())?;
        let commands = self
            .adapter_for(&tool)
            .map(|adapter| adapter.version_commands)
            .unwrap_or_else(|| {
                vec![
                    vec!["--version".into()],
                    vec!["version".into()],
                    vec!["-V".into()],
                ]
            });
        for args in commands {
            if let Ok(output) = run_probe(&tool.executable, &args, Duration::from_secs(3)) {
                if !output.trim().is_empty() {
                    let version = first_lines(&output, 3);
                    if let Some(record) = self.tools.write().get_mut(id) {
                        record.version = Some(version.clone());
                    }
                    return Ok(version);
                }
            }
        }
        Err("无法安全识别该工具的版本".into())
    }

    fn make_record(
        &self,
        canonical: PathBuf,
        aliases: Vec<String>,
        is_system: bool,
        homebrew_leaves: &HashSet<String>,
    ) -> ToolRecord {
        let adapter = find_adapter(&self.adapters, &aliases);
        let primary_name = adapter
            .and_then(|item| {
                item.names
                    .iter()
                    .find(|name| aliases.iter().any(|alias| alias == *name))
            })
            .cloned()
            .or_else(|| aliases.first().cloned())
            .unwrap_or_else(|| "unknown".into());
        let source = detect_source(&canonical, is_system);
        let category = adapter
            .map(|item| item.category.clone())
            .unwrap_or_else(|| infer_category(&aliases, is_system));
        let executable = canonical.to_string_lossy().to_string();
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, executable.as_bytes()).to_string();
        let version = adapter.and_then(|item| {
            item.version_commands.iter().find_map(|args| {
                run_probe(&executable, args, Duration::from_secs(3))
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| first_lines(&value, 3))
            })
        });
        let config_paths = adapter
            .map(|item| {
                item.config_paths
                    .iter()
                    .map(|path| expand_home(path))
                    .collect()
            })
            .unwrap_or_default();
        let capabilities = ToolCapabilities {
            terminal: true,
            version_probe: true,
            update: adapter.and_then(|item| item.update.as_ref()).is_some()
                || source.kind == "npm"
                || (source.kind == "homebrew"
                    && source
                        .package_name
                        .as_ref()
                        .map(|package| homebrew_leaves.contains(package))
                        .unwrap_or(false)),
            diagnose: adapter.and_then(|item| item.diagnose.as_ref()).is_some(),
            auth: adapter.and_then(|item| item.auth.as_ref()).is_some(),
            config: adapter
                .map(|item| !item.config_paths.is_empty() || item.config_command.is_some())
                .unwrap_or(false),
        };
        ToolRecord {
            id,
            name: primary_name,
            display_name: adapter
                .map(|item| item.display_name.clone())
                .unwrap_or_else(|| humanize_name(&aliases[0])),
            executable,
            canonical_path: canonical.to_string_lossy().to_string(),
            aliases,
            version,
            source,
            category,
            is_system,
            adapter_id: adapter.map(|item| item.id.clone()),
            capabilities,
            config_paths,
        }
    }
}

fn login_path_directories() -> Vec<PathBuf> {
    let inherited = env::var_os("PATH").unwrap_or_default();
    #[cfg(unix)]
    let resolved = {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        if Path::new(&shell).is_absolute() && Path::new(&shell).exists() {
            Command::new(shell)
                .args(["-lic", "printf '\\n__CLIHUB_PATH__%s\\n' \"$PATH\""])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|output| {
                    output
                        .lines()
                        .find_map(|line| line.strip_prefix("__CLIHUB_PATH__"))
                        .map(std::ffi::OsString::from)
                })
                .unwrap_or(inherited)
        } else {
            inherited
        }
    };
    #[cfg(windows)]
    let resolved = inherited;

    let mut directories = Vec::new();
    for path in env::split_paths(&resolved) {
        if path.is_dir() && !is_transient_directory(&path) && !directories.contains(&path) {
            directories.push(path);
        }
    }
    directories
}

fn is_transient_directory(path: &Path) -> bool {
    let value = path.to_string_lossy().to_lowercase();
    value.contains("/.cache/codex-runtimes/")
        || value.contains("/.codex/tmp/")
        || value.contains("/codex.system/")
        || value == "/pkg/env/global/bin"
        || value.contains("/chatgpt.app/contents/resources")
}

fn homebrew_leaves() -> HashSet<String> {
    let candidates = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];
    candidates
        .iter()
        .find(|path| Path::new(path).is_file())
        .and_then(|path| {
            Command::new(path)
                .arg("leaves")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
        })
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_system_directory(path: &Path) -> bool {
    let value = path.to_string_lossy().to_lowercase();
    #[cfg(target_os = "macos")]
    return matches!(value.as_str(), "/usr/bin" | "/bin" | "/usr/sbin" | "/sbin")
        || value.contains("/system/cryptexes/")
        || value.contains("/appleinternal/");
    #[cfg(target_os = "windows")]
    return value.contains("\\windows\\system32") || value.contains("\\windows\\syswow64");
    #[cfg(all(unix, not(target_os = "macos")))]
    return matches!(value.as_str(), "/usr/bin" | "/bin" | "/usr/sbin" | "/sbin");
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    return metadata.permissions().mode() & 0o111 != 0;
    #[cfg(windows)]
    return path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "exe" | "cmd" | "bat" | "com"
            )
        })
        .unwrap_or(false);
}

fn command_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    #[cfg(windows)]
    let name = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".cmd"))
        .or_else(|| name.strip_suffix(".bat"))
        .unwrap_or(&name)
        .to_string();
    #[cfg(unix)]
    let name = name.to_string();
    (!name.starts_with('.')).then_some(name)
}

fn detect_source(path: &Path, is_system: bool) -> ToolSource {
    let parts: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    if is_system {
        return ToolSource {
            kind: "system".into(),
            label: "Operating system".into(),
            package_name: None,
            package_version: None,
            scope: "system".into(),
        };
    }
    if let Some(index) = parts.iter().position(|part| *part == "Cellar") {
        return ToolSource {
            kind: "homebrew".into(),
            label: "Homebrew".into(),
            package_name: parts.get(index + 1).map(|value| (*value).into()),
            package_version: parts.get(index + 2).map(|value| (*value).into()),
            scope: "global".into(),
        };
    }
    if let Some(index) = parts.iter().position(|part| *part == "node_modules") {
        let package_name = parts.get(index + 1).map(|value| {
            if value.starts_with('@') {
                parts
                    .get(index + 2)
                    .map(|child| format!("{value}/{child}"))
                    .unwrap_or_else(|| (*value).into())
            } else {
                (*value).into()
            }
        });
        return ToolSource {
            kind: "npm".into(),
            label: "Global npm".into(),
            package_name,
            package_version: None,
            scope: "global".into(),
        };
    }
    let value = path.to_string_lossy();
    if value.contains("/Applications/") || value.contains("\\Program Files\\") {
        return ToolSource {
            kind: "application".into(),
            label: "Desktop application".into(),
            package_name: parts
                .iter()
                .find(|part| part.ends_with(".app"))
                .map(|value| (*value).into()),
            package_version: None,
            scope: "global".into(),
        };
    }
    if value.contains("Android/sdk") {
        return ToolSource {
            kind: "android-sdk".into(),
            label: "Android SDK".into(),
            package_name: None,
            package_version: None,
            scope: "user".into(),
        };
    }
    ToolSource {
        kind: "user".into(),
        label: "User PATH".into(),
        package_name: None,
        package_version: None,
        scope: "user".into(),
    }
}

fn infer_category(names: &[String], is_system: bool) -> ToolCategory {
    if is_system {
        return ToolCategory::System;
    }
    let joined = names.join(" ").to_lowercase();
    if [
        "docker",
        "kubectl",
        "helm",
        "aws",
        "cloudflared",
        "orb",
        "terraform",
    ]
    .iter()
    .any(|name| joined.contains(name))
    {
        ToolCategory::CloudContainer
    } else if [
        "adb",
        "android",
        "sdkmanager",
        "avdmanager",
        "apktool",
        "jadx",
    ]
    .iter()
    .any(|name| joined.contains(name))
    {
        ToolCategory::Android
    } else if [
        "node", "npm", "npx", "python", "pip", "uv", "java", "mvn", "cargo", "go",
    ]
    .iter()
    .any(|name| joined == *name || joined.starts_with(&format!("{name} ")))
    {
        ToolCategory::Runtime
    } else if ["git", "rg", "jq", "starship", "fastfetch", "brew"]
        .iter()
        .any(|name| joined == *name || joined.starts_with(&format!("{name} ")))
    {
        ToolCategory::DeveloperTool
    } else {
        ToolCategory::Other
    }
}

fn humanize_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_probe(executable: &str, args: &[String], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let status = child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?;
    if status.is_none() {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    let mut value = String::from_utf8_lossy(&output.stdout).to_string();
    if value.trim().is_empty() {
        value = String::from_utf8_lossy(&output.stderr).to_string();
    }
    if status.is_none() {
        Err("version probe timed out".into())
    } else {
        Ok(value)
    }
}

fn first_lines(value: &str, limit: usize) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(limit)
        .collect::<Vec<_>>()
        .join(" · ")
}

fn category_rank(category: &ToolCategory) -> u8 {
    match category {
        ToolCategory::AiAgent => 0,
        ToolCategory::Collaboration => 1,
        ToolCategory::CloudContainer => 2,
        ToolCategory::Runtime => 3,
        ToolCategory::Android => 4,
        ToolCategory::DeveloperTool => 5,
        ToolCategory::Other => 6,
        ToolCategory::System => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn source_detection_reads_homebrew_formula() {
        let source = detect_source(
            Path::new("/opt/homebrew/Cellar/ripgrep/15.2.0/bin/rg"),
            false,
        );
        assert_eq!(source.kind, "homebrew");
        assert_eq!(source.package_name.as_deref(), Some("ripgrep"));
        assert_eq!(source.package_version.as_deref(), Some("15.2.0"));
    }

    #[test]
    fn aliases_share_a_canonical_path() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("tool");
        let mut file = fs::File::create(&target).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, temp.path().join("tool-alias")).unwrap();
        assert_eq!(
            fs::canonicalize(&target).unwrap(),
            fs::canonicalize(temp.path().join("tool-alias")).unwrap()
        );
    }

    #[test]
    fn codex_runtime_paths_are_not_global_inventory() {
        assert!(is_transient_directory(Path::new(
            "/Users/example/.cache/codex-runtimes/runtime/bin"
        )));
        assert!(!is_transient_directory(Path::new(
            "/Users/example/.local/bin"
        )));
    }
}
