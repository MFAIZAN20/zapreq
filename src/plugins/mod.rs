use serde::Deserialize;
use std::path::PathBuf;

use crate::config::Config;

pub mod manager;

/// Plugin metadata exposed in plugin listings.
#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub auth_types: Vec<String>,
    pub executable: Option<String>,
}

/// Runtime plugin contract.
pub trait ZapReqPlugin: Send + Sync {
    fn info(&self) -> PluginInfo;
    fn auth_plugin(&self) -> Option<Box<dyn crate::auth::AuthPlugin>> {
        None
    }
}

#[derive(Debug, Deserialize)]
struct PluginManifest {
    plugin: ManifestPlugin,
}

#[derive(Debug, Deserialize)]
struct ManifestPlugin {
    name: String,
    version: String,
    description: String,
    #[serde(default)]
    auth_types: Vec<String>,
    executable: Option<String>,
}

/// Lists built-in plugins and manifest-based plugins from config.plugins_dir.
pub fn list_plugins(config: &Config) -> Vec<PluginInfo> {
    let mut out = builtin_plugins();
    let dir = plugins_dir(config);

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            if let Some(plugin) = parse_manifest(&path) {
                out.push(plugin);
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Prints plugin table.
pub fn print_plugins(plugins: &[PluginInfo]) {
    if plugins.is_empty() {
        println!("No plugins found.");
        return;
    }

    let name_width = plugins
        .iter()
        .map(|p| p.name.len())
        .max()
        .unwrap_or(4)
        .max("NAME".len());
    let version_width = plugins
        .iter()
        .map(|p| p.version.len())
        .max()
        .unwrap_or(7)
        .max("VERSION".len());

    println!(
        "{:<name_width$}  {:<version_width$}  DESCRIPTION",
        "NAME",
        "VERSION",
        name_width = name_width,
        version_width = version_width
    );
    for p in plugins {
        let _ = &p.auth_types;
        println!(
            "{:<name_width$}  {:<version_width$}  {}",
            p.name,
            p.version,
            p.description,
            name_width = name_width,
            version_width = version_width
        );
    }
}

/// Resolves the plugins directory path.
pub fn plugins_dir(config: &Config) -> PathBuf {
    if let Ok(root) = crate::config::config_root_dir() {
        if config.plugins_dir.trim().is_empty() {
            return root.join("plugins");
        }
    }
    let raw = config.plugins_dir.trim();
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

fn parse_manifest(path: &std::path::Path) -> Option<PluginInfo> {
    let data = std::fs::read_to_string(path).ok()?;
    let manifest: PluginManifest = toml::from_str(&data).ok()?;
    Some(PluginInfo {
        name: manifest.plugin.name,
        version: manifest.plugin.version,
        description: manifest.plugin.description,
        auth_types: manifest.plugin.auth_types,
        executable: manifest.plugin.executable,
    })
}

fn builtin_plugins() -> Vec<PluginInfo> {
    vec![
        PluginInfo {
            name: "basic".to_string(),
            version: "built-in".to_string(),
            description: "HTTP Basic Authentication".to_string(),
            auth_types: vec!["basic".to_string()],
            executable: None,
        },
        PluginInfo {
            name: "bearer".to_string(),
            version: "built-in".to_string(),
            description: "Bearer Token Authentication".to_string(),
            auth_types: vec!["bearer".to_string()],
            executable: None,
        },
        PluginInfo {
            name: "digest".to_string(),
            version: "built-in".to_string(),
            description: "HTTP Digest Authentication (RFC 7616)".to_string(),
            auth_types: vec!["digest".to_string()],
            executable: None,
        },
    ]
}
