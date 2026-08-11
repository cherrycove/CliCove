use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolCategory {
    AiAgent,
    Collaboration,
    Runtime,
    CloudContainer,
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
    pub capabilities: ToolCapabilities,
    pub config_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandTemplate {
    pub executable: String,
    pub args: Vec<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    pub id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub current_version: Option<String>,
    pub target_version: String,
    pub executable: String,
    pub args: Vec<String>,
    pub display_command: String,
    pub source: String,
    pub requires_elevation: bool,
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
