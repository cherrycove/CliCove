use crate::models::{
    Capability, Installation, Provider, ResourceCatalog, ToolCategory, ToolRecord,
};
use crate::AppState;
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

pub(crate) struct DiscoveredResources {
    pub providers: Vec<Provider>,
    pub installations: Vec<Installation>,
    pub capabilities: Vec<Capability>,
}

struct CapabilitySpec<'a> {
    key: &'a str,
    display_name: &'a str,
    kind: &'a str,
    description: &'a str,
    risk_level: &'a str,
    agent_callable: bool,
    available: bool,
}

pub(crate) fn discover(tools: &[ToolRecord]) -> DiscoveredResources {
    let now = chrono::Utc::now().to_rfc3339();
    let mut providers = BTreeMap::<String, Provider>::new();
    let mut installations = Vec::new();
    let mut capabilities = Vec::new();

    for tool in tools {
        if !matches!(
            tool.category,
            ToolCategory::AiAgent
                | ToolCategory::Collaboration
                | ToolCategory::CloudService
                | ToolCategory::Observability
        ) || tool.is_system
        {
            continue;
        }
        let mut provider = provider_for(tool);
        provider.installation_count = 0;
        providers.entry(provider.id.clone()).or_insert(provider);
        let provider_id = provider_id_for(tool);
        installations.push(Installation {
            id: tool.id.clone(),
            provider_id: provider_id.clone(),
            tool_id: tool.id.clone(),
            display_name: tool.display_name.clone(),
            command: tool.name.clone(),
            executable: tool.executable.clone(),
            aliases: tool.aliases.clone(),
            version: tool.version.clone(),
            source: tool.source.clone(),
            adapter_id: tool.adapter_id.clone(),
            status: if tool
                .runtime_dependencies
                .iter()
                .all(|dependency| dependency.available)
            {
                "ready".into()
            } else {
                "degraded".into()
            },
            last_seen: now.clone(),
            active: true,
        });

        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "invoke",
                display_name: "CLI 调用",
                kind: "invoke",
                description: "通过受校验的可执行文件与参数数组调用该 CLI",
                risk_level: "controlled",
                agent_callable: true,
                available: tool.capabilities.terminal,
            },
        );
        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "version",
                display_name: "版本探测",
                kind: "inspection",
                description: "只读获取本机安装版本",
                risk_level: "read-only",
                agent_callable: false,
                available: tool.capabilities.version_probe,
            },
        );
        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "diagnose",
                display_name: "诊断",
                kind: "diagnostic",
                description: "运行适配器审核过的诊断命令",
                risk_level: "read-only",
                agent_callable: true,
                available: tool.capabilities.diagnose,
            },
        );
        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "auth",
                display_name: "登录与授权",
                kind: "authentication",
                description: "由官方 CLI 自行完成登录和凭据管理",
                risk_level: "interactive",
                agent_callable: false,
                available: tool.capabilities.auth,
            },
        );
        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "config",
                display_name: "配置入口",
                kind: "configuration",
                description: "访问审核过的配置入口或脱敏预览",
                risk_level: "controlled",
                agent_callable: false,
                available: tool.capabilities.config,
            },
        );
        push_capability(
            &mut capabilities,
            tool,
            &provider_id,
            CapabilitySpec {
                key: "update",
                display_name: "安装与更新",
                kind: "lifecycle",
                description: "生成需人工确认的一次性安装或更新计划",
                risk_level: "confirmation-required",
                agent_callable: false,
                available: tool.capabilities.update,
            },
        );
    }

    for installation in &installations {
        if let Some(provider) = providers.get_mut(&installation.provider_id) {
            provider.installation_count += 1;
        }
    }

    let mut providers: Vec<_> = providers.into_values().collect();
    providers.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    installations.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    capabilities.sort_by(|left, right| {
        left.installation_id
            .cmp(&right.installation_id)
            .then(left.key.cmp(&right.key))
    });
    DiscoveredResources {
        providers,
        installations,
        capabilities,
    }
}

fn push_capability(
    capabilities: &mut Vec<Capability>,
    tool: &ToolRecord,
    provider_id: &str,
    spec: CapabilitySpec<'_>,
) {
    if !spec.available {
        return;
    }
    let identity = format!("capability:{}:{}", tool.id, spec.key);
    capabilities.push(Capability {
        id: Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()).to_string(),
        provider_id: provider_id.into(),
        installation_id: tool.id.clone(),
        key: spec.key.into(),
        display_name: spec.display_name.into(),
        kind: spec.kind.into(),
        description: spec.description.into(),
        risk_level: spec.risk_level.into(),
        agent_callable: spec.agent_callable,
        available: true,
    });
}

fn provider_id_for(tool: &ToolRecord) -> String {
    let stable = match tool.adapter_id.as_deref() {
        Some("claude") => "anthropic",
        Some("codex") => "openai",
        Some("grok") => "xai",
        Some("lark-cli" | "meegle") => "lark",
        Some("opencode") => "opencode",
        Some("trellis") => "trellis",
        Some("owl") => "guance",
        Some("cursor") => "cursor",
        Some("aws") => "aws",
        Some("cloudflared") => "cloudflare",
        Some("gh") => "github",
        Some("glab") => "gitlab",
        Some("gcloud") => "google-cloud",
        Some("az") => "azure",
        Some("kubectl" | "helm") => "kubernetes",
        Some("docker") => "docker",
        Some("terraform") => "hashicorp",
        Some("vercel") => "vercel",
        Some("netlify") => "netlify",
        Some("railway") => "railway",
        Some("fly") => "fly-io",
        Some("slack") => "slack",
        Some("linear") => "linear",
        Some(adapter) => return format!("adapter:{adapter}"),
        None => {
            let product = tool
                .source
                .package_name
                .as_deref()
                .unwrap_or(tool.name.as_str());
            let identity = format!("{}:{}:{product}", tool.source.kind, tool.source.scope);
            return format!(
                "local:{}",
                Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes())
            );
        }
    };
    stable.into()
}

fn provider_for(tool: &ToolRecord) -> Provider {
    let id = provider_id_for(tool);
    let (display_name, kind, website) = match id.as_str() {
        "anthropic" => (
            "Anthropic",
            "first-party",
            Some("https://www.anthropic.com"),
        ),
        "openai" => ("OpenAI", "first-party", Some("https://openai.com")),
        "xai" => ("xAI", "first-party", Some("https://x.ai")),
        "lark" => (
            "Lark / 飞书",
            "first-party",
            Some("https://www.larksuite.com"),
        ),
        "opencode" => ("OpenCode", "community", Some("https://opencode.ai")),
        "trellis" => ("Trellis", "community", None),
        "guance" => (
            "Guance / 观测云",
            "first-party",
            Some("https://www.guance.com"),
        ),
        "cursor" => ("Cursor", "first-party", Some("https://www.cursor.com")),
        "aws" => (
            "Amazon Web Services",
            "first-party",
            Some("https://aws.amazon.com"),
        ),
        "cloudflare" => (
            "Cloudflare",
            "first-party",
            Some("https://www.cloudflare.com"),
        ),
        "github" => ("GitHub", "first-party", Some("https://github.com")),
        "gitlab" => ("GitLab", "first-party", Some("https://gitlab.com")),
        "google-cloud" => (
            "Google Cloud",
            "first-party",
            Some("https://cloud.google.com"),
        ),
        "azure" => (
            "Microsoft Azure",
            "first-party",
            Some("https://azure.microsoft.com"),
        ),
        "kubernetes" => ("Kubernetes", "community", Some("https://kubernetes.io")),
        "docker" => ("Docker", "first-party", Some("https://www.docker.com")),
        "hashicorp" => (
            "HashiCorp",
            "first-party",
            Some("https://www.hashicorp.com"),
        ),
        "vercel" => ("Vercel", "first-party", Some("https://vercel.com")),
        "netlify" => ("Netlify", "first-party", Some("https://www.netlify.com")),
        "railway" => ("Railway", "first-party", Some("https://railway.app")),
        "fly-io" => ("Fly.io", "first-party", Some("https://fly.io")),
        "slack" => ("Slack", "first-party", Some("https://slack.com")),
        "linear" => ("Linear", "first-party", Some("https://linear.app")),
        _ if id.starts_with("adapter:") => (tool.display_name.as_str(), "custom-adapter", None),
        _ if tool.service.is_some() => (tool.display_name.as_str(), "local", None),
        _ => (
            tool.source
                .package_name
                .as_deref()
                .unwrap_or(tool.display_name.as_str()),
            "local",
            None,
        ),
    };
    Provider {
        id,
        display_name: display_name.into(),
        kind: kind.into(),
        category: tool.category.clone(),
        auth_owner: "external-cli".into(),
        website: website.map(str::to_string),
        installation_count: 0,
    }
}

#[tauri::command]
pub fn list_resource_catalog(state: State<'_, AppState>) -> Result<ResourceCatalog, String> {
    state.storage.resource_catalog()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeDependency, ToolCapabilities, ToolSource};

    fn tool(adapter: &str, name: &str) -> ToolRecord {
        ToolRecord {
            id: format!("tool-{adapter}"),
            name: name.into(),
            display_name: name.into(),
            executable: format!("/usr/local/bin/{name}"),
            canonical_path: format!("/usr/local/bin/{name}"),
            aliases: vec![name.into()],
            related_commands: vec![name.into()],
            version: Some("1.0.0".into()),
            source: ToolSource {
                kind: "npm".into(),
                label: "Global npm".into(),
                package_name: Some(name.into()),
                package_version: Some("1.0.0".into()),
                scope: "user".into(),
            },
            category: ToolCategory::Collaboration,
            is_system: false,
            adapter_id: Some(adapter.into()),
            service: None,
            capabilities: ToolCapabilities {
                terminal: true,
                version_probe: true,
                update: true,
                diagnose: false,
                auth: true,
                config: false,
            },
            config_paths: Vec::new(),
            config_preview_available: Vec::new(),
            runtime_dependencies: Vec::<RuntimeDependency>::new(),
        }
    }

    #[test]
    fn lark_products_share_one_provider_but_keep_installations() {
        let resources = discover(&[tool("lark-cli", "lark-cli"), tool("meegle", "meegle")]);
        assert_eq!(resources.providers.len(), 1);
        assert_eq!(resources.providers[0].id, "lark");
        assert_eq!(resources.providers[0].installation_count, 2);
        assert_eq!(resources.installations.len(), 2);
    }

    #[test]
    fn catalog_only_service_exposes_invocation_without_inventing_adapter_actions() {
        let mut cli = tool("volcengine", "ve");
        cli.adapter_id = None;
        cli.source.package_name = Some("@volcengine/cli".into());
        cli.category = ToolCategory::CloudService;
        cli.capabilities.auth = false;
        cli.service = crate::service_catalog::find_service(&cli.aliases, &cli.source, None)
            .map(|service| service.identity());
        let resources = discover(&[cli]);
        assert_eq!(resources.installations.len(), 1);
        assert!(resources.installations[0].adapter_id.is_none());
        assert!(resources
            .capabilities
            .iter()
            .any(|cap| cap.key == "invoke" && cap.agent_callable));
        assert!(resources
            .capabilities
            .iter()
            .all(|cap| !["auth", "diagnose", "config"].contains(&cap.key.as_str())));
        assert!(resources
            .capabilities
            .iter()
            .filter(|cap| cap.key != "invoke")
            .all(|cap| !cap.agent_callable));
    }

    #[test]
    fn lifecycle_and_auth_capabilities_are_not_agent_callable() {
        let resources = discover(&[tool("lark-cli", "lark-cli")]);
        assert!(
            resources
                .capabilities
                .iter()
                .find(|capability| capability.key == "invoke")
                .unwrap()
                .agent_callable
        );
        assert!(resources
            .capabilities
            .iter()
            .filter(|capability| capability.key == "update" || capability.key == "auth")
            .all(|capability| !capability.agent_callable));
    }

    #[test]
    fn runtime_and_infrastructure_tools_never_enter_agent_resources() {
        let mut runtime = tool("node", "node");
        runtime.category = ToolCategory::Runtime;
        let mut infrastructure = tool("docker", "docker");
        infrastructure.category = ToolCategory::Infrastructure;
        let resources = discover(&[runtime, infrastructure]);
        assert!(resources.providers.is_empty());
        assert!(resources.installations.is_empty());
        assert!(resources.capabilities.is_empty());
    }
}
