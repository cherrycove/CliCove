use crate::adapters::{builtin_adapters, config_preview_fields, expand_home, find_adapter};
use crate::models::{
    AdapterManifest, RuntimeDependency, ToolCapabilities, ToolCategory, ToolRecord, ToolSource,
};
use crate::process::background_command;
use crate::service_catalog::find_service;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use uuid::Uuid;
use wait_timeout::ChildExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub struct ToolRegistry {
    adapters: RwLock<Vec<AdapterManifest>>,
    tools: RwLock<HashMap<String, ToolRecord>>,
    login_path: RwLock<OsString>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new(builtin_adapters())
    }
}

impl ToolRegistry {
    pub fn new(adapters: Vec<AdapterManifest>) -> Self {
        Self {
            adapters: RwLock::new(adapters),
            tools: RwLock::new(HashMap::new()),
            login_path: RwLock::new(env::var_os("PATH").unwrap_or_default()),
        }
    }
    pub fn scan(&self, show_system: bool, show_all_executables: bool) -> Vec<ToolRecord> {
        let login_path = resolve_login_path();
        *self.login_path.write() = login_path.clone();
        let homebrew_requested = homebrew_requested_packages();
        let mut grouped: BTreeMap<String, (PathBuf, Vec<String>, bool)> = BTreeMap::new();
        for directory in login_path_directories(&login_path) {
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
                self.make_record(
                    canonical,
                    aliases,
                    is_system,
                    &homebrew_requested,
                    &login_path,
                )
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

    pub fn all(&self) -> Vec<ToolRecord> {
        self.tools.read().values().cloned().collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<ToolRecord> {
        self.tools
            .read()
            .values()
            .find(|tool| tool.aliases.iter().any(|alias| alias == name))
            .cloned()
    }

    pub fn execution_path(&self) -> OsString {
        self.login_path.read().clone()
    }

    pub fn replace_adapters(&self, adapters: Vec<AdapterManifest>) {
        *self.adapters.write() = adapters;
        self.tools.write().clear();
    }

    pub fn resolve_executable(&self, name: &str) -> Option<String> {
        self.find_by_name(name)
            .map(|record| record.executable)
            .or_else(|| {
                find_executable_in_path(name, &self.login_path.read())
                    .map(|path| path.to_string_lossy().to_string())
            })
    }

    pub fn adapter_for(&self, tool: &ToolRecord) -> Option<AdapterManifest> {
        tool.adapter_id.as_ref().and_then(|id| {
            self.adapters
                .read()
                .iter()
                .find(|adapter| &adapter.id == id)
                .cloned()
        })
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
            if let Ok(output) = run_probe(
                &tool.executable,
                &args,
                Duration::from_secs(3),
                &self.login_path.read(),
            ) {
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
        login_path: &OsStr,
    ) -> ToolRecord {
        let mut source = detect_source(&canonical, is_system);
        let adapters = self.adapters.read();
        let adapter = find_adapter(&adapters, &aliases, &canonical, &source);
        let service = find_service(&aliases, &source, adapter.map(|item| item.id.as_str()));
        if service.is_some() && source.kind == "npm" {
            source.package_version = npm_package_version(&canonical);
        }
        let primary_name = adapter
            .and_then(|item| {
                item.names
                    .iter()
                    .find(|name| aliases.iter().any(|alias| alias == *name))
            })
            .cloned()
            .or_else(|| service.and_then(|item| {
                item.commands.iter().find(|name| aliases.contains(name)).cloned()
            }))
            .or_else(|| aliases.first().cloned())
            .unwrap_or_else(|| "unknown".into());
        let category = adapter
            .map(|item| item.category.clone())
            .or_else(|| service.map(|item| item.category.clone()))
            .unwrap_or_else(|| infer_category(&aliases, is_system));
        let executable = canonical.to_string_lossy().to_string();
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, executable.as_bytes()).to_string();
        let config_paths = adapter
            .map(|item| {
                item.config_paths
                    .iter()
                    .map(|path| expand_home(path))
                    .collect()
            })
            .unwrap_or_default();
        let config_preview_available = adapter
            .map(|item| {
                (0..item.config_paths.len())
                    .map(|index| config_preview_fields(&item.id, index).is_some())
                    .collect()
            })
            .unwrap_or_default();
        let runtime_dependencies: Vec<RuntimeDependency> = adapter
            .map(|item| &item.dependencies)
            .or_else(|| service.map(|item| &item.dependencies))
            .map(|dependencies| {
                dependencies
                    .iter()
                    .map(|name| {
                        let dependency = find_executable_in_path(name, login_path);
                        RuntimeDependency {
                            name: name.clone(),
                            executable: dependency
                                .as_ref()
                                .map(|path| path.to_string_lossy().to_string()),
                            available: dependency.is_some(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let dependencies_available = runtime_dependencies
            .iter()
            .all(|dependency| dependency.available);
        let version = adapter.filter(|_| dependencies_available).and_then(|item| {
            item.version_commands.iter().find_map(|args| {
                run_probe(&executable, args, Duration::from_secs(3), login_path)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| first_lines(&value, 3))
            })
        }).or_else(|| service.and(source.package_version.clone()));
        let capabilities = ToolCapabilities {
            terminal: true,
            version_probe: true,
            update: (source.kind != "application"
                && adapter.and_then(|item| item.update.as_ref()).is_some())
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
                .or_else(|| service.map(|item| item.display_name.clone()))
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
            service: service.map(|item| item.identity()),
            capabilities,
            config_paths,
            config_preview_available,
            runtime_dependencies,
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
                if product.adapter_id.is_none() && product.service.is_none() {
                    product.display_name = package_display_name(package, &product.display_name);
                }
            }
            product.id = product_id(&product);
            Some(product)
        })
        .collect()
}

fn product_id(tool: &ToolRecord) -> String {
    let identity = if let Some(adapter) = tool.adapter_id.as_deref() {
        format!("adapter:{adapter}")
    } else if let Some(package) = tool.source.package_name.as_deref() {
        format!(
            "package:{}:{package}:{}",
            tool.source.kind, tool.source.scope
        )
    } else {
        format!("command:{}", tool.canonical_path)
    };
    Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()).to_string()
}

fn product_rank(tool: &ToolRecord, package: Option<&str>) -> (u8, usize, String) {
    let rank = if tool.adapter_id.is_some() || tool.service.is_some() {
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

fn resolve_login_path() -> OsString {
    let inherited = env::var_os("PATH").unwrap_or_default();
    #[cfg(unix)]
    {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        if Path::new(&shell).is_absolute() && Path::new(&shell).exists() {
            background_command(shell)
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
    }
    #[cfg(windows)]
    {
        inherited
    }
}

fn login_path_directories(resolved: &OsStr) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for path in env::split_paths(resolved) {
        if path.is_dir() && !is_transient_directory(&path) && !directories.contains(&path) {
            directories.push(path);
        }
    }
    directories
}

fn find_executable_in_path(name: &str, resolved: &OsStr) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains(['/', '\\'])
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".@+_-".contains(character))
    {
        return None;
    }
    #[cfg(windows)]
    let candidates: Vec<String> = {
        let path = Path::new(name);
        if path.extension().is_some() {
            vec![name.to_string()]
        } else {
            env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(|extension| format!("{name}{}", extension.to_ascii_lowercase()))
                .chain(std::iter::once(name.to_string()))
                .collect()
        }
    };
    #[cfg(not(windows))]
    let candidates = vec![name.to_string()];

    for directory in login_path_directories(resolved) {
        for candidate in &candidates {
            let path = directory.join(candidate);
            if is_executable(&path) {
                return Some(fs::canonicalize(&path).unwrap_or(path));
            }
        }
    }
    None
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
            background_command(path)
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
    if let Some((_, package_name)) = npm_package_root(path) {
        return ToolSource {
            kind: "npm".into(),
            label: "Global npm".into(),
            package_name: Some(package_name),
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

// Use the nearest package boundary: a nested dependency must not inherit its parent's identity.
fn npm_package_root(path: &Path) -> Option<(&Path, String)> {
    for root in path.parent()?.ancestors() {
        let name = root.file_name()?.to_str()?;
        let parent = root.parent()?;
        let parent_name = parent.file_name().and_then(OsStr::to_str).unwrap_or("");
        if name.starts_with('.') || name.starts_with('@') {
            continue;
        }
        if parent_name == "node_modules" {
            return Some((root, name.into()));
        }
        if parent_name.starts_with('@')
            && parent.parent().and_then(Path::file_name) == Some(OsStr::new("node_modules"))
        {
            return Some((root, format!("{parent_name}/{name}")));
        }
    }
    None
}

fn npm_package_version(path: &Path) -> Option<String> {
    let (root, expected_name) = npm_package_root(path)?;
    // Read metadata only, never execute an unadapted CLI during discovery.
    let manifest = root.join("package.json");
    if !fs::metadata(&manifest).ok()?.is_file() {
        return None;
    }
    let file = fs::File::open(manifest).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(128 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 128 * 1024 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    if value.get("name")?.as_str()? != expected_name {
        return None;
    }
    let version = value.get("version")?.as_str()?;
    (!version.is_empty() && version.len() <= 128
        && version.chars().all(|ch| ch.is_ascii_alphanumeric() || ".+-".contains(ch)))
        .then(|| version.to_string())
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
        "gcloud",
        "az",
        "aliyun",
        "hcloud",
        "flyctl",
        "doctl",
        "heroku",
        "supabase",
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

pub(crate) fn run_probe(
    executable: &str,
    args: &[String],
    timeout: Duration,
    login_path: &OsStr,
) -> Result<String, String> {
    let mut child = background_command(executable)
        .args(args)
        .env("PATH", login_path)
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
    match status {
        None => Err("version probe timed out".into()),
        Some(status) if !status.success() => Err(first_lines(&value, 3)),
        Some(_) => Ok(value),
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
            service: None,
            capabilities: ToolCapabilities {
                terminal: true,
                version_probe: true,
                update: false,
                diagnose: false,
                auth: false,
                config: false,
            },
            config_paths: Vec::new(),
            config_preview_available: Vec::new(),
            runtime_dependencies: Vec::new(),
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
    fn service_catalog_discovers_cloud_clis_without_action_adapters() {
        let registry = ToolRegistry::new(Vec::new());
        let requested = HashSet::new();
        let cases = [
            ("wrangler", "wrangler", "wrangler", "Cloudflare Wrangler"),
            ("vercel", "vercel", "vercel", "Vercel CLI"),
            ("netlify", "netlify-cli", "netlify", "Netlify CLI"),
            ("firebase", "firebase-tools", "firebase", "Firebase CLI"),
            ("ve", "@volcengine/cli", "volcengine", "Volcengine CLI"),
            ("stripe", "stripe", "stripe", "Stripe CLI"),
        ];
        for (name, package, id, display_name) in cases {
            let path = if name == "stripe" {
                PathBuf::from("/opt/homebrew/Cellar/stripe/1.2.3/bin/stripe")
            } else {
                PathBuf::from(format!("/test/lib/node_modules/{package}/bin/{name}"))
            };
            let record = registry.make_record(path.clone(), vec![name.into()], false, &requested, OsStr::new(""));
            let helper = registry.make_record(path.with_file_name("helper"), vec!["helper".into()], false, &requested, OsStr::new(""));
            let products: Vec<_> = product_records(vec![helper, record], &requested)
                .into_iter().filter(is_capability_cli).collect();
            assert_eq!(products.len(), 1, "{name}");
            let tool = &products[0];
            assert_eq!(tool.display_name, display_name);
            assert_eq!(tool.name, name);
            assert_eq!(tool.service.as_ref().unwrap().id, id);
            assert!(tool.adapter_id.is_none());
            assert!(tool.capabilities.terminal && tool.capabilities.version_probe);
            assert!(!tool.capabilities.auth && !tool.capabilities.diagnose && !tool.capabilities.config);
            assert!(tool.config_paths.is_empty());
            assert_eq!(tool.related_commands.len(), 2);
            assert_eq!(tool.aliases, vec![name]);
            if name == "ve" {
                assert!(tool.runtime_dependencies.is_empty(), "the ve binary is not a Node runtime");
            }
        }
    }

    #[test]
    fn directly_installed_runtimes_unknown_packages_and_spoofed_names_stay_excluded() {
        let registry = ToolRegistry::new(Vec::new());
        let mut records = Vec::new();
        let requested = ["node", "python@3.13", "openjdk", "helm", "docker", "kubectl"]
            .map(str::to_string).into_iter().collect();
        for (package, name) in [("node", "node"), ("python@3.13", "python3"), ("openjdk", "java"),
            ("helm", "helm"), ("docker", "docker"), ("kubectl", "kubectl")]
        {
            records.push(registry.make_record(format!("/opt/homebrew/Cellar/{package}/1/bin/{name}").into(),
                vec![name.into()], false, &requested, OsStr::new("")));
        }
        for name in ["npm", "npx", "ve", "wrangler", "firebase", "stripe", "unknown-tool"] {
            records.push(registry.make_record(format!("/test/node_modules/unrelated/bin/{name}").into(),
                vec![name.into()], false, &requested, OsStr::new("")));
        }
        let raw_count = records.len();
        assert!(raw_count > 10); // These remain accessible through the advanced raw view.
        assert!(product_records(records, &requested).iter().all(|tool| !is_capability_cli(tool)));
    }

    #[test]
    fn catalog_identity_keeps_existing_product_ids_and_cache_compatibility() {
        let mut tool = test_tool("firebase", "npm", Some("firebase-tools"), ToolCategory::CloudService, None);
        let old_id = product_id(&tool);
        let mut legacy = serde_json::to_value(&tool).unwrap();
        legacy.as_object_mut().unwrap().remove("service");
        let restored: ToolRecord = serde_json::from_value(legacy).unwrap();
        assert!(restored.service.is_none());
        tool.service = find_service(&tool.aliases, &tool.source, None).map(|service| service.identity());
        assert_eq!(old_id, product_id(&tool));
        let restored: ToolRecord = serde_json::from_str(&serde_json::to_string(&tool).unwrap()).unwrap();
        assert_eq!(restored, tool);
    }

    #[test]
    fn npm_identity_uses_nearest_package_not_outer_dependency_owner() {
        let source = detect_source(Path::new("/test/node_modules/@volcengine/cli/node_modules/unrelated/bin/ve"), false);
        assert_eq!(source.package_name.as_deref(), Some("unrelated"));
        assert!(find_service(&["ve".into()], &source, None).is_none());
        assert_eq!(detect_source(Path::new("/test/node_modules/@volcengine/cli/bin/ve"), false)
            .package_name.as_deref(), Some("@volcengine/cli"));
    }

    #[cfg(unix)]
    #[test]
    fn catalog_scan_reads_bounded_version_metadata_without_executing_cli() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("node_modules/@volcengine/cli");
        fs::create_dir_all(root.join("bin")).unwrap();
        let executable = root.join("bin/ve");
        let marker = root.join("executed");
        fs::write(&executable, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(root.join("package.json"), r#"{"name":"@volcengine/cli","version":"1.2.3"}"#).unwrap();
        let registry = ToolRegistry::new(Vec::new());
        let tool = registry.make_record(executable.clone(), vec!["ve".into()], false, &HashSet::new(), OsStr::new("/usr/bin:/bin"));
        assert_eq!(tool.version.as_deref(), Some("1.2.3"));
        assert!(!marker.exists());
        fs::write(root.join("package.json"), r#"{"name":"unrelated","version":"9.9.9"}"#).unwrap();
        assert!(npm_package_version(&executable).is_none());
        fs::write(root.join("package.json"), " ".repeat(128 * 1024 + 1)).unwrap();
        assert!(npm_package_version(&executable).is_none());
        fs::write(root.join("package.json"), "invalid JSON").unwrap();
        assert!(npm_package_version(&executable).is_none());
    }

    #[cfg(unix)]
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
    fn ambiguous_aliases_require_identity_evidence() {
        let adapters = builtin_adapters();
        let source = ToolSource {
            kind: "application".into(),
            label: "Desktop application".into(),
            package_name: None,
            package_version: None,
            scope: "global".into(),
        };

        assert!(find_adapter(
            &adapters,
            &["code".into()],
            Path::new("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"),
            &source,
        )
        .is_none());
        assert_eq!(
            find_adapter(
                &adapters,
                &["code".into()],
                Path::new("/Applications/Cursor.app/Contents/Resources/app/bin/code"),
                &source,
            )
            .map(|adapter| adapter.id.as_str()),
            Some("cursor")
        );
        assert!(find_adapter(
            &adapters,
            &["agent".into()],
            Path::new("/usr/local/bin/agent"),
            &source,
        )
        .is_none());
        assert_eq!(
            find_adapter(
                &adapters,
                &["agent".into()],
                Path::new("/Users/example/.grok/downloads/grok-1.0.0"),
                &source,
            )
            .map(|adapter| adapter.id.as_str()),
            Some("grok")
        );
    }

    #[test]
    fn package_product_id_survives_versioned_path_changes() {
        let mut first = test_tool(
            "aws",
            "homebrew",
            Some("awscli"),
            ToolCategory::CloudService,
            Some("aws"),
        );
        first.canonical_path = "/opt/homebrew/Cellar/awscli/2.35.0/bin/aws".into();
        let mut second = first.clone();
        second.canonical_path = "/opt/homebrew/Cellar/awscli/2.36.0/bin/aws".into();

        assert_eq!(product_id(&first), product_id(&second));
    }

    #[cfg(unix)]
    #[test]
    fn version_probe_uses_resolved_login_path_for_env_shebangs() {
        let temp = TempDir::new().unwrap();
        let node = temp.path().join("node");
        let cli = temp.path().join("example-cli");
        fs::write(&node, "#!/bin/sh\nprintf 'example 1.2.3\\n'\n").unwrap();
        fs::write(&cli, "#!/usr/bin/env node\n").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();

        let output = run_probe(
            cli.to_str().unwrap(),
            &[],
            Duration::from_secs(1),
            temp.path().as_os_str(),
        )
        .unwrap();
        assert_eq!(output.trim(), "example 1.2.3");
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
