//! Product identity is data, not an authorization to run management commands.
//! Installation recipes and scanning share this catalog; richer actions stay in adapters.
use crate::models::{ServiceIdentity, ToolCategory, ToolSource};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: ToolCategory,
    pub commands: Vec<String>,
    #[serde(default)]
    pub standalone_commands: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
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
    pub fn identity(&self) -> ServiceIdentity {
        ServiceIdentity {
            id: self.id.clone(),
            description: self.description.clone(),
        }
    }
}

pub fn services() -> &'static [ServiceDefinition] {
    static CATALOG: OnceLock<Vec<ServiceDefinition>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../service-catalog.json"))
            .expect("the bundled service catalog must be valid")
    })
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

    #[test]
    fn catalog_is_unique_and_contains_only_service_capabilities() {
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        for service in services() {
            assert!(ids.insert(&service.id));
            assert!(!service.description.is_empty());
            assert!(!service.commands.is_empty());
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
