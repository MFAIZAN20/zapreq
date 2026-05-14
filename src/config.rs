use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// CAUS-CORERUNTIM-04, CAUS-CLI-21:
/// User configuration contract loaded before CLI parsing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub default_options: Vec<String>,
    #[serde(default = "default_scheme")]
    pub default_scheme: String,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: String,
    #[serde(default = "default_output_theme")]
    pub output_theme: String,
    #[serde(default = "default_pretty")]
    pub pretty: String,
    #[serde(default = "default_verify")]
    pub verify: bool,
}

/// Public config alias used by output/printer APIs.
pub type Config = AppConfig;

/// Community feature contract for named environment profiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnvProfile {
    pub base_url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// Intermediate resolved CLI values after profile/environment expansion.
#[derive(Clone, Debug, Default)]
pub struct CliResolved {
    pub url: String,
    pub request_items: Vec<String>,
    pub profile_headers: HashMap<String, String>,
    pub variables: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_options: Vec::new(),
            default_scheme: default_scheme(),
            plugins_dir: default_plugins_dir(),
            output_theme: default_output_theme(),
            pretty: default_pretty(),
            verify: default_verify(),
        }
    }
}

/// CAUS-CORERUNTIM-04:
/// Returns config file path under user config directory.
pub fn default_config_path() -> Result<PathBuf> {
    Ok(config_root_dir()?.join("config.json"))
}

/// Returns ferrite config root directory.
pub fn config_root_dir() -> Result<PathBuf> {
    if let Ok(override_root) = std::env::var("FERRITE_CONFIG_DIR") {
        let trimmed = override_root.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let config_root = dirs::config_dir().context("could not resolve user config directory")?;
    Ok(config_root.join("ferrite"))
}

/// CAUS-CORERUNTIM-04:
/// Loads config from disk when present, else returns defaults.
pub fn load_config() -> Result<AppConfig> {
    let path = default_config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;

    let config: AppConfig = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    Ok(config)
}

/// Loads an environment profile from ~/.config/ferrite/envs/{name}.json.
pub fn load_profile(name: &str) -> Result<EnvProfile> {
    let path = config_root_dir()?.join("envs").join(format!("{name}.json"));

    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read env profile: {}", path.display()))?;
    let profile: EnvProfile = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse env profile: {}", path.display()))?;
    Ok(profile)
}

/// Applies profile URL/header/variable defaults onto a resolved CLI payload.
pub fn apply_profile(profile: &EnvProfile, cli: &mut CliResolved) {
    if let Some(base_url) = &profile.base_url {
        let trimmed_base = base_url.trim_end_matches('/');
        let is_absolute = cli.url.starts_with("http://") || cli.url.starts_with("https://");
        if !cli.url.is_empty() && !is_absolute {
            let relative = cli.url.trim_start_matches('/');
            cli.url = format!("{trimmed_base}/{relative}");
        }
    }

    for (k, v) in &profile.headers {
        cli.profile_headers.insert(k.clone(), v.clone());
    }

    for (k, v) in &profile.variables {
        cli.variables.entry(k.clone()).or_insert_with(|| v.clone());
    }
}

/// CAUS-CORERUNTIM-04:
/// Merges built-ins, config defaults, env defaults, then explicit CLI (highest priority).
pub fn merge_defaults(config: &AppConfig, argv: &mut Vec<String>) {
    if argv.is_empty() {
        return;
    }

    let explicit = argv.iter().skip(1).cloned().collect::<Vec<_>>();

    // Priority: built-in < config < FERRITE_DEFAULT_OPTIONS < explicit CLI
    let built_in = built_in_default_options();
    let config_defaults = config_default_options(config);
    let env_defaults = env_default_options();

    let mut merged = Vec::with_capacity(
        1 + built_in.len() + config_defaults.len() + env_defaults.len() + explicit.len(),
    );
    merged.push(argv[0].clone());
    merged.extend(built_in);
    merged.extend(config_defaults);
    merged.extend(env_defaults);
    merged.extend(explicit);

    resolve_body_mode_conflicts(&mut merged);
    *argv = merged;
}

/// CAUS-CORERUNTIM-04:
/// Returns built-in fallback defaults when no upstream defaults are provided.
fn built_in_default_options() -> Vec<String> {
    vec![
        "--pretty".to_string(),
        "all".to_string(),
        "--default-scheme".to_string(),
        "https".to_string(),
        "--verify".to_string(),
        "true".to_string(),
    ]
}

/// CAUS-CORERUNTIM-04:
/// Converts config fields and config default_options into CLI tokens.
fn config_default_options(config: &AppConfig) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(config.default_options.clone());

    out.push("--pretty".to_string());
    out.push(config.pretty.clone());

    out.push("--style".to_string());
    out.push(config.output_theme.clone());

    out.push("--default-scheme".to_string());
    out.push(config.default_scheme.clone());

    out.push("--verify".to_string());
    out.push(if config.verify { "true" } else { "false" }.to_string());

    out
}

/// CAUS-CORERUNTIM-04:
/// Parses environment-provided default CLI options from FERRITE_DEFAULT_OPTIONS.
fn env_default_options() -> Vec<String> {
    let raw = match std::env::var("FERRITE_DEFAULT_OPTIONS") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    raw.split_whitespace().map(|s| s.to_string()).collect()
}

fn resolve_body_mode_conflicts(argv: &mut Vec<String>) {
    if argv.len() <= 1 {
        return;
    }

    let mut last_mode_index = None;
    for (idx, token) in argv.iter().enumerate().skip(1) {
        if is_body_mode_flag(token) {
            last_mode_index = Some(idx);
        }
    }

    let Some(last_idx) = last_mode_index else {
        return;
    };

    let mut filtered = Vec::with_capacity(argv.len());
    filtered.push(argv[0].clone());
    for (idx, token) in argv.iter().enumerate().skip(1) {
        if is_body_mode_flag(token) && idx != last_idx {
            continue;
        }
        filtered.push(token.clone());
    }
    *argv = filtered;
}

fn is_body_mode_flag(token: &str) -> bool {
    matches!(token, "--json" | "-j" | "--form" | "-f" | "--multipart")
}

fn default_scheme() -> String {
    "https".to_string()
}

fn default_plugins_dir() -> String {
    "~/.config/ferrite/plugins".to_string()
}

fn default_output_theme() -> String {
    "monokai".to_string()
}

fn default_pretty() -> String {
    "all".to_string()
}

fn default_verify() -> bool {
    true
}
