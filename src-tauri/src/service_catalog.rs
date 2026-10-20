//! Product identity is data, not an authorization to run management commands.
//! Installation recipes and scanning share this catalog; richer actions stay in adapters.
use crate::models::{CliInformation, ServiceIdentity, ToolCategory, ToolSource};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub introduction: String,
    pub documentation_url: String,
    pub category: ToolCategory,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub standalone_commands: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub packages: Vec<ServicePackage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServicePackage {
    pub source: String,
    pub name: String,
    #[serde(default)]
    pub install: bool,
    #[serde(default)]
    pub cask: bool,
}

impl ServiceDefinition {
    fn validate_information(&self) -> Result<(), String> {
        if self.commands.is_empty()
            && (!self.packages.is_empty()
                || !self.standalone_commands.is_empty()
                || !self.dependencies.is_empty())
        {
            return Err(format!(
                "{} information-only entry cannot declare installation or discovery rules",
                self.id
            ));
        }
        if self.description.trim().is_empty() || self.introduction.trim().is_empty() {
            return Err(format!(
                "{} must include a description and introduction",
                self.id
            ));
        }
        let url = reqwest::Url::parse(&self.documentation_url)
            .map_err(|_| format!("{} must include a valid documentation URL", self.id))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(format!(
                "{} documentation must use an HTTPS URL without credentials",
                self.id
            ));
        }
        Ok(())
    }

    pub fn identity(&self) -> ServiceIdentity {
        ServiceIdentity {
            id: self.id.clone(),
            description: self.description.clone(),
        }
    }

    fn information(&self) -> CliInformation {
        CliInformation {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            description: self.description.clone(),
            introduction: self.introduction.clone(),
            documentation_url: self.documentation_url.clone(),
            category: self.category.clone(),
        }
    }
}

pub fn services() -> &'static [ServiceDefinition] {
    static CATALOG: OnceLock<Vec<ServiceDefinition>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: Vec<ServiceDefinition> =
            serde_json::from_str(include_str!("../service-catalog.json"))
                .expect("the bundled service catalog must be valid");
        for service in &catalog {
            service
                .validate_information()
                .expect("CLI information must be complete");
        }
        catalog
    })
}

fn documentation_url(service_id: &str) -> Result<&'static str, String> {
    services()
        .iter()
        .find(|service| service.id == service_id)
        .map(|service| service.documentation_url.as_str())
        .ok_or_else(|| "此 CLI 不在内置资料目录中".to_string())
}

#[tauri::command]
pub fn list_cli_information() -> Vec<CliInformation> {
    // Display information is available independently of installation recipes and OS.
    services()
        .iter()
        .map(ServiceDefinition::information)
        .collect()
}

#[tauri::command]
pub fn open_cli_documentation(service_id: String) -> Result<(), String> {
    // Resolve the reviewed URL from the bundled catalog; do not accept a URL from the UI.
    open::that(documentation_url(&service_id)?).map_err(|error| error.to_string())
}

pub fn find_service(
    aliases: &[String],
    source: &ToolSource,
    adapter_id: Option<&str>,
) -> Option<&'static ServiceDefinition> {
    if source.kind == "system" {
        return None;
    }
    services().iter().find(|service| {
        // Display-only entries never add discovery or executable permissions.
        if service.commands.is_empty() {
            return false;
        }
        // Existing trusted adapters retain their identity and precedence.
        if adapter_id == Some(service.id.as_str()) {
            return true;
        }
        if !service.commands.iter().any(|name| aliases.contains(name)) {
            return false;
        }
        if let Some(package) = source.package_name.as_deref() {
            return service
                .packages
                .iter()
                .any(|expected| expected.source == source.kind && expected.name == package);
        }
        // Only explicitly reviewed, distinctive names allow standalone discovery.
        // A short name such as `ve` requires package evidence, never name guessing.
        source.kind == "user"
            && service
                .standalone_commands
                .iter()
                .any(|name| aliases.contains(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn catalog_fixture() -> serde_json::Value {
        serde_json::from_str::<Vec<serde_json::Value>>(include_str!("../service-catalog.json"))
            .unwrap()
            .remove(0)
    }

    #[test]
    fn new_catalog_entries_require_information_fields() {
        for field in ["description", "introduction", "documentationUrl"] {
            let mut entry = catalog_fixture();
            entry.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<ServiceDefinition>(entry).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn information_rejects_blank_text_and_unsafe_documentation_links() {
        for field in ["description", "introduction"] {
            let mut entry = catalog_fixture();
            entry[field] = serde_json::json!(" \n\t ");
            let service: ServiceDefinition = serde_json::from_value(entry).unwrap();
            assert!(service.validate_information().is_err(), "{field}");
        }
        for url in [
            "",
            "not-a-url",
            "http://example.com",
            "file:///tmp/help",
            "javascript:alert(1)",
            "https://user:password@example.com",
        ] {
            let mut entry = catalog_fixture();
            entry["documentationUrl"] = serde_json::json!(url);
            let service: ServiceDefinition = serde_json::from_value(entry).unwrap();
            assert!(service.validate_information().is_err(), "{url}");
        }
    }

    #[test]
    fn documentation_opens_only_a_known_catalog_entry() {
        for service in services() {
            assert!(service.validate_information().is_ok());
            assert_eq!(
                documentation_url(&service.id).unwrap(),
                service.documentation_url
            );
        }
        assert!(documentation_url("https://example.com").is_err());
        assert!(documentation_url("unknown-cli").is_err());
    }

    #[test]
    fn information_covers_every_builtin_adapter_even_without_install_recipes() {
        let information = list_cli_information();
        for adapter in crate::adapters::builtin_adapters() {
            assert!(
                information.iter().any(|item| item.id == adapter.id),
                "{} needs CLI information",
                adapter.id
            );
        }
        for command in ["aliyun", "hcloud"] {
            assert!(information.iter().any(|item| item.id == command));
        }
    }

    #[test]
    fn information_only_entries_do_not_change_discovery_or_installation() {
        let source = ToolSource {
            kind: "user".into(),
            label: "User PATH".into(),
            package_name: None,
            package_version: None,
            scope: "user".into(),
        };
        for service in services().iter().filter(|entry| entry.commands.is_empty()) {
            assert!(service.packages.is_empty());
            assert!(find_service(&[service.id.clone()], &source, Some(&service.id)).is_none());
            assert!(documentation_url(&service.id).is_ok());
        }
        let mut entry = catalog_fixture();
        entry.as_object_mut().unwrap().remove("commands");
        let service: ServiceDefinition = serde_json::from_value(entry).unwrap();
        assert!(service.validate_information().is_err());
    }

    #[test]
    fn catalog_is_unique_and_discovery_contains_only_service_capabilities() {
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        for service in services() {
            assert!(ids.insert(&service.id));
            assert!(!service.description.is_empty());
            if service.commands.is_empty() {
                assert!(service.packages.is_empty());
                continue;
            }
            assert!(matches!(
                service.category,
                ToolCategory::AiAgent
                    | ToolCategory::Collaboration
                    | ToolCategory::CloudService
                    | ToolCategory::Observability
            ));
            for command in &service.commands {
                assert!(commands.insert(command));
            }
            assert!(service
                .standalone_commands
                .iter()
                .all(|name| service.commands.contains(name)));
            assert_eq!(
                service
                    .packages
                    .iter()
                    .filter(|package| package.install)
                    .count(),
                1
            );
            assert!(service.packages.iter().all(|package| matches!(
                package.source.as_str(),
                "npm" | "homebrew"
            ) && (!package.cask
                || package.source == "homebrew")));
        }
    }

    #[test]
    fn short_commands_and_conflicting_packages_are_not_guessed() {
        let mut source = ToolSource {
            kind: "user".into(),
            label: "User PATH".into(),
            package_name: None,
            package_version: None,
            scope: "user".into(),
        };
        assert!(find_service(&["ve".into()], &source, None).is_none());
        assert_eq!(
            find_service(&["stripe".into()], &source, None).unwrap().id,
            "stripe"
        );
        source.kind = "npm".into();
        source.package_name = Some("unrelated-package".into());
        for name in ["ve", "wrangler", "firebase", "stripe"] {
            assert!(find_service(&[name.into()], &source, None).is_none());
        }
        source.package_name = Some("@volcengine/cli".into());
        assert_eq!(
            find_service(&["ve".into()], &source, None).unwrap().id,
            "volcengine"
        );
        assert!(find_service(&["helper".into()], &source, None).is_none());
    }
}
