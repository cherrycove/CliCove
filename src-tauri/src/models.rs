use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolCategory {
    AiAgent,
    Collaboration,
    CloudService,
    Observability,
    #[serde(rename = "infrastructure", alias = "cloud-container")]
    Infrastructure,
    Runtime,
    Android,
    DeveloperTool,
    System,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSource {
    pub kind: String,
    pub label: String,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub scope: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    pub terminal: bool,
    pub version_probe: bool,
    pub update: bool,
    pub diagnose: bool,
    pub auth: bool,
    pub config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDependency {
    pub name: String,
    pub executable: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliInformation {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub introduction: String,
    pub documentation_url: String,
    pub category: ToolCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceIdentity {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub executable: String,
    pub canonical_path: String,
    pub aliases: Vec<String>,
    pub related_commands: Vec<String>,
    pub version: Option<String>,
    pub source: ToolSource,
    pub category: ToolCategory,
    pub is_system: bool,
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub service: Option<ServiceIdentity>,
    pub capabilities: ToolCapabilities,
    pub config_paths: Vec<String>,
    #[serde(default)]
    pub config_preview_available: Vec<bool>,
    #[serde(default)]
    pub runtime_dependencies: Vec<RuntimeDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub category: ToolCategory,
    pub auth_owner: String,
    pub website: Option<String>,
    pub installation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: String,
    pub provider_id: String,
    pub tool_id: String,
    pub display_name: String,
    pub command: String,
    pub executable: String,
    pub aliases: Vec<String>,
    pub version: Option<String>,
    pub source: ToolSource,
    pub adapter_id: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub id: String,
    pub provider_id: String,
    pub installation_id: String,
    pub key: String,
    pub display_name: String,
    pub kind: String,
    pub description: String,
    pub risk_level: String,
    pub agent_callable: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCatalog {
    pub providers: Vec<Provider>,
    pub installations: Vec<Installation>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandTemplate {
    pub executable: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterMatcher {
    pub name: String,
    #[serde(default)]
    pub path_contains: Vec<String>,
    #[serde(default)]
    pub package_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterManifest {
    pub id: String,
    pub display_name: String,
    pub names: Vec<String>,
    pub category: ToolCategory,
    pub version_commands: Vec<Vec<String>>,
    pub update: Option<CommandTemplate>,
    pub diagnose: Option<CommandTemplate>,
    pub auth: Option<CommandTemplate>,
    pub config_paths: Vec<String>,
    pub config_command: Option<CommandTemplate>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub matchers: Vec<AdapterMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapterAction {
    pub name: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapterInfo {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub fingerprint: String,
    pub status: String,
    pub message: String,
    pub has_stored_trust: bool,
    pub actions: Vec<CustomAdapterAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    pub id: String,
    pub operation: String,
    pub tool_id: String,
    pub adapter_id: Option<String>,
    pub tool_name: String,
    pub current_version: Option<String>,
    pub target_version: String,
    pub executable: String,
    pub args: Vec<String>,
    pub display_command: String,
    pub source: String,
    pub requires_elevation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersionInfo {
    pub target_id: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: Option<bool>,
    pub checked_at: String,
    pub source: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallCandidate {
    pub id: String,
    pub adapter_id: String,
    pub display_name: String,
    pub description: String,
    pub introduction: String,
    pub documentation_url: String,
    pub category: ToolCategory,
    pub package_manager: String,
    pub package_name: String,
    pub source: String,
    pub manager_available: bool,
    pub installed: bool,
    pub installed_tool_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHistoryRecord {
    pub id: String,
    pub operation: String,
    pub tool_id: String,
    pub tool_name: String,
    pub source: String,
    pub display_command: String,
    pub current_version: Option<String>,
    pub target_version: String,
    pub observed_version: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub verified_at: Option<String>,
    pub exit_code: Option<i32>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSpawnRequest {
    pub tool_id: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    pub id: String,
    pub tool_id: String,
    pub title: String,
    pub executable: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub started_at: String,
    pub status: String,
    pub kind: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryRecord {
    pub id: String,
    pub tool_id: String,
    pub title: String,
    pub kind: String,
    pub executable: String,
    pub cwd: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusField {
    pub label: String,
    pub value: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationStatus {
    pub tool_id: String,
    #[serde(default)]
    pub installation: Option<HealthInstallation>,
    pub status: String,
    pub method: Option<String>,
    pub expires_in_minutes: Option<i64>,
    pub checked_at: String,
    pub source: String,
    pub message: String,
    pub fields: Vec<StatusField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub tool_id: String,
    #[serde(default)]
    pub installation: Option<HealthInstallation>,
    pub status: String,
    pub checked_at: String,
    pub duration_ms: u64,
    pub summary: String,
    pub checks: Vec<DiagnosticCheck>,
}

/// Identifies the installation inspected by a cached health result, without credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HealthInstallation {
    pub executable: String,
    pub canonical_path: String,
    pub version: Option<String>,
    pub source: ToolSource,
    pub runtime_dependencies: Vec<RuntimeDependency>,
}

impl From<&ToolRecord> for HealthInstallation {
    fn from(tool: &ToolRecord) -> Self {
        Self {
            executable: tool.executable.clone(),
            canonical_path: tool.canonical_path.clone(),
            version: tool.version.clone(),
            source: tool.source.clone(),
            runtime_dependencies: tool.runtime_dependencies.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutput {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExit {
    pub session_id: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreview {
    pub path: String,
    pub exists: bool,
    pub content: String,
    pub truncated: bool,
    pub notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub show_system_tools: bool,
    pub show_all_executables: bool,
    pub tray_enabled: bool,
    pub default_working_directory: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_system_tools: false,
            show_all_executables: false,
            tray_enabled: false,
            default_working_directory: dirs::home_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        }
    }
}
