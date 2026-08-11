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
    pub fn scan(&self, show_system: bool, show_all_executables: bool) -> Vec<ToolRecord> {
        let homebrew_requested = homebrew_requested_packages();
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

        let raw_records: Vec<_> = grouped
            .into_iter()
            .map(|(_, (canonical, mut aliases, is_system))| {
                aliases.sort();
                self.make_record(canonical, aliases, is_system, &homebrew_requested)
            })
            .collect();

        let mut records = if show_all_executables {
            raw_records
        } else {
            product_records(raw_records, &homebrew_requested)
                .into_iter()
                .filter(is_capability_cli)
                .collect()
        };

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
        homebrew_requested: &HashSet<String>,
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
                        .map(|package| homebrew_requested.contains(package))
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
            related_commands: aliases.clone(),
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

fn product_records(
    records: Vec<ToolRecord>,
    homebrew_requested: &HashSet<String>,
) -> Vec<ToolRecord> {
    let mut groups: BTreeMap<String, Vec<ToolRecord>> = BTreeMap::new();
    for record in records {
        let key = match (
            record.adapter_id.as_deref(),
            record.source.kind.as_str(),
            record.source.package_name.as_deref(),
        ) {
            (Some(adapter), _, _) => format!("adapter:{adapter}"),
            (None, "homebrew" | "npm", Some(package)) => {
                format!("{}:{package}", record.source.kind)
            }
            _ => format!("command:{}", record.name.to_lowercase()),
        };
        groups.entry(key).or_default().push(record);
    }

    groups
        .into_values()
        .filter_map(|mut group| {
            let package = group[0].source.package_name.clone();
            let is_package_group = matches!(group[0].source.kind.as_str(), "homebrew" | "npm");
            let is_direct_homebrew = group[0].source.kind == "homebrew"
                && package
                    .as_ref()
                    .map(|name| homebrew_requested.contains(name))
                    .unwrap_or(false);
            let is_known_cli = preferred_command_for_package(package.as_deref()).is_some()
                || group.iter().any(is_primary_command);

            group.sort_by_key(|tool| product_rank(tool, package.as_deref()));
            if is_package_group
                && group[0].source.kind == "homebrew"
                && !is_direct_homebrew
                && !is_known_cli
            {
                return None;
            }
            if !is_package_group && !is_primary_command(&group[0]) {
                return None;
            }

            let mut product = group.remove(0);
            let mut related: Vec<String> = product.aliases.clone();
            for tool in group {
                related.extend(tool.aliases);
            }
            related.sort();
            related.dedup();
            if product.adapter_id.is_some() {
                product.aliases = related.clone();
            }
            product.related_commands = related;
            if let Some(package) = package.as_deref() {
                if product.adapter_id.is_none() {
                    product.display_name = package_display_name(package, &product.display_name);
                }
            }
            Some(product)
        })
        .collect()
}

fn product_rank(tool: &ToolRecord, package: Option<&str>) -> (u8, usize, String) {
    let rank = if tool.adapter_id.is_some() {
        0
    } else if preferred_command_for_package(package)
        .map(|preferred| tool.aliases.iter().any(|alias| alias == preferred))
        .unwrap_or(false)
    {
        1
    } else if package
        .map(|package| tool.aliases.iter().any(|alias| alias == package))
        .unwrap_or(false)
    {
        2
    } else if is_primary_command(tool) {
        10
    } else {
        50
    };
    let shortest = tool
        .aliases
        .iter()
        .map(String::len)
        .min()
        .unwrap_or(usize::MAX);
    (rank, shortest, tool.name.clone())
}

fn preferred_command_for_package(package: Option<&str>) -> Option<&'static str> {
    let package = package?;
    if package.starts_with("openjdk") {
        return Some("java");
    }
    if package.starts_with("python@") {
        return Some("python3");
    }
    match package {
        "awscli" => Some("aws"),
        "docker-buildx" => Some("docker-buildx"),
        "maven" => Some("mvn"),
        "node" => Some("node"),
        "rust" => Some("cargo"),
        "uv" => Some("uv"),
        _ => None,
    }
}

fn is_primary_command(tool: &ToolRecord) -> bool {
    if tool.adapter_id.is_some() || tool.category != ToolCategory::Other {
        return true;
    }
    const PRIMARY: &[&str] = &[
        "brew",
        "node",
        "npm",
        "npx",
        "python3",
        "pip3",
        "uv",
        "uvx",
        "java",
        "javac",
        "mvn",
        "cargo",
        "rustc",
        "git",
        "rg",
        "jq",
        "docker",
        "docker-compose",
        "docker-buildx",
        "kubectl",
        "helm",
        "aws",
        "cloudflared",
        "orb",
        "orbctl",
        "android",
        "adb",
        "sdkmanager",
        "avdmanager",
        "lint",
        "apkanalyzer",
        "retrace",
        "apktool",
        "jadx",
        "starship",
        "fastfetch",
    ];
    tool.aliases
        .iter()
        .any(|alias| PRIMARY.contains(&alias.as_str()))
}

fn is_capability_cli(tool: &ToolRecord) -> bool {
    matches!(
        tool.category,
        ToolCategory::AiAgent
            | ToolCategory::Collaboration
            | ToolCategory::CloudService
            | ToolCategory::Observability
    )
}

fn package_display_name(package: &str, fallback: &str) -> String {
    if package.starts_with("openjdk@") {
        return format!("OpenJDK {}", package.trim_start_matches("openjdk@"));
    }
    if package.starts_with("python@") {
        return format!("Python {}", package.trim_start_matches("python@"));
    }
    match package {
        "awscli" => "AWS CLI".into(),
        "docker-buildx" => "Docker Buildx".into(),
        "maven" => "Apache Maven".into(),
        "rust" => "Rust Toolchain".into(),
        _ => fallback.into(),
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
        || value.ends_with("/node_modules/.bin")
        || value == "/pkg/env/global/bin"
        || value.contains("/chatgpt.app/contents/resources")
}

fn homebrew_requested_packages() -> HashSet<String> {
    let candidates = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];
    candidates
        .iter()
        .find(|path| Path::new(path).is_file())
        .and_then(|path| {
            Command::new(path)
                .args(["info", "--json=v2", "--installed"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
        })
        .filter(|output| output.status.success())
        .map(|output| parse_homebrew_requested_packages(&output.stdout))
        .unwrap_or_default()
}

fn parse_homebrew_requested_packages(json: &[u8]) -> HashSet<String> {
    let mut requested = HashSet::new();
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(json) else {
        return requested;
    };
    let Some(formulae) = value.get("formulae").and_then(serde_json::Value::as_array) else {
        return requested;
    };
    for formula in formulae {
        let installed_on_request = formula
            .get("installed")
            .and_then(serde_json::Value::as_array)
            .map(|installed| {
                installed.iter().any(|entry| {
                    entry
                        .get("installed_on_request")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if installed_on_request {
            if let Some(name) = formula.get("name").and_then(serde_json::Value::as_str) {
                requested.insert(name.to_string());
            }
        }
    }
    requested
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
    let normalized: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
    let has = |candidates: &[&str]| {
        normalized
            .iter()
            .any(|name| candidates.contains(&name.as_str()))
    };
    if has(&[
        "aws",
        "aws-vault",
        "cloudflared",
        "wrangler",
        "gcloud",
        "az",
        "aliyun",
        "hcloud",
        "vercel",
        "netlify",
        "firebase",
        "flyctl",
        "doctl",
        "heroku",
        "supabase",
        "stripe",
    ]) {
        ToolCategory::CloudService
    } else if has(&[
        "sentry-cli",
        "datadog-ci",
        "newrelic",
        "grafana",
        "guance",
        "owl",
    ]) {
        ToolCategory::Observability
    } else if has(&[
        "gh", "glab", "slack", "notion", "lark-cli", "meegle", "linear",
    ]) {
        ToolCategory::Collaboration
    } else if has(&[
        "docker",
        "docker-compose",
        "docker-buildx",
        "docker-credential-osxkeychain",
        "kubectl",
        "helm",
        "orb",
        "orbctl",
        "terraform",
        "tofu",
        "packer",
    ]) {
        ToolCategory::Infrastructure
    } else if has(&[
        "adb",
        "android",
        "sdkmanager",
        "avdmanager",
        "apktool",
        "jadx",
    ]) {
        ToolCategory::Android
    } else if has(&[
        "node", "npm", "npx", "python", "pip", "uv", "java", "mvn", "cargo", "go",
    ]) {
        ToolCategory::Runtime
    } else if has(&["git", "rg", "jq", "starship", "fastfetch", "brew"]) {
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
        ToolCategory::CloudService => 2,
        ToolCategory::Observability => 3,
        ToolCategory::Infrastructure => 4,
        ToolCategory::Runtime => 5,
        ToolCategory::Android => 6,
        ToolCategory::DeveloperTool => 7,
        ToolCategory::Other => 8,
        ToolCategory::System => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_tool(
        name: &str,
        source_kind: &str,
        package_name: Option<&str>,
        category: ToolCategory,
        adapter_id: Option<&str>,
    ) -> ToolRecord {
        ToolRecord {
            id: format!("id-{name}"),
            name: name.into(),
            display_name: humanize_name(name),
            executable: format!("/test/bin/{name}"),
            canonical_path: format!("/test/bin/{name}"),
            aliases: vec![name.into()],
            related_commands: vec![name.into()],
            version: None,
            source: ToolSource {
                kind: source_kind.into(),
                label: source_kind.into(),
                package_name: package_name.map(str::to_string),
                package_version: None,
                scope: "user".into(),
            },
            category,
            is_system: false,
            adapter_id: adapter_id.map(str::to_string),
            capabilities: ToolCapabilities {
                terminal: true,
                version_probe: true,
                update: false,
                diagnose: false,
                auth: false,
                config: false,
            },
            config_paths: Vec::new(),
        }
    }

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
    fn homebrew_request_parser_excludes_dependencies() {
        let packages = parse_homebrew_requested_packages(
            br#"{"formulae":[
                {"name":"helm","installed":[{"installed_on_request":true}]},
                {"name":"gettext","installed":[{"installed_on_request":false}]}
            ]}"#,
        );
        assert!(packages.contains("helm"));
        assert!(!packages.contains("gettext"));
    }

    #[test]
    fn product_inventory_filters_dependencies_and_merges_adapter_commands() {
        let records = vec![
            test_tool(
                "gettext",
                "homebrew",
                Some("gettext"),
                ToolCategory::Other,
                None,
            ),
            test_tool(
                "rg",
                "homebrew",
                Some("ripgrep"),
                ToolCategory::DeveloperTool,
                None,
            ),
            test_tool(
                "cursor",
                "application",
                Some("Cursor.app"),
                ToolCategory::AiAgent,
                Some("cursor"),
            ),
            test_tool(
                "code",
                "application",
                Some("Cursor.app"),
                ToolCategory::AiAgent,
                Some("cursor"),
            ),
        ];

        let products = product_records(records, &HashSet::new());
        assert!(!products.iter().any(|tool| tool.name == "gettext"));
        assert!(products.iter().any(|tool| tool.name == "rg"));
        let cursor = products
            .iter()
            .find(|tool| tool.adapter_id.as_deref() == Some("cursor"))
            .unwrap();
        assert_eq!(cursor.related_commands, vec!["code", "cursor"]);
        assert_eq!(cursor.aliases, cursor.related_commands);
    }

    #[test]
    fn capability_inventory_excludes_runtimes_and_infrastructure() {
        let capability_tools = [
            test_tool(
                "claude",
                "user",
                None,
                ToolCategory::AiAgent,
                Some("claude"),
            ),
            test_tool(
                "aws",
                "homebrew",
                Some("awscli"),
                ToolCategory::CloudService,
                Some("aws"),
            ),
        ];
        let excluded_tools = [
            test_tool(
                "docker",
                "application",
                Some("Docker.app"),
                ToolCategory::Infrastructure,
                None,
            ),
            test_tool(
                "node",
                "homebrew",
                Some("node"),
                ToolCategory::Runtime,
                None,
            ),
        ];

        assert!(capability_tools.iter().all(is_capability_cli));
        assert!(excluded_tools.iter().all(|tool| !is_capability_cli(tool)));
    }

    #[test]
    fn codex_runtime_paths_are_not_global_inventory() {
        assert!(is_transient_directory(Path::new(
            "/Users/example/.cache/codex-runtimes/runtime/bin"
        )));
        assert!(is_transient_directory(Path::new(
            "/Users/example/project/node_modules/.bin"
        )));
        assert!(!is_transient_directory(Path::new(
            "/Users/example/.local/bin"
        )));
    }
}
