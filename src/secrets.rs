use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::config_root_dir;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SecretFile {
    #[serde(default)]
    values: HashMap<String, String>,
}

/// Stores a secret value by key.
pub fn set_secret(key: &str, value: &str) -> Result<()> {
    let mut file = load_secret_file()?;
    file.values
        .insert(key.to_string(), STANDARD.encode(value.as_bytes()));
    write_secret_file(&file)
}

/// Gets a secret value by key. Returns None when key is missing.
pub fn get_secret(key: &str) -> Result<Option<String>> {
    let file = load_secret_file()?;
    let Some(encoded) = file.values.get(key) else {
        return Ok(None);
    };
    let bytes = STANDARD
        .decode(encoded)
        .with_context(|| format!("failed to decode secret for key '{key}'"))?;
    let value = String::from_utf8(bytes)
        .with_context(|| format!("secret value for key '{key}' is not valid UTF-8"))?;
    Ok(Some(value))
}

/// Lists all known secret keys.
pub fn list_secret_keys() -> Result<Vec<String>> {
    let file = load_secret_file()?;
    let mut keys = file.values.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    Ok(keys)
}

/// Masks a secret value for display.
pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "".to_string();
    }
    if value.len() <= 4 {
        return "****".to_string();
    }
    let prefix = &value[..2];
    let suffix = &value[value.len() - 2..];
    format!("{prefix}****{suffix}")
}

fn load_secret_file() -> Result<SecretFile> {
    let path = secrets_path()?;
    if !path.exists() {
        return Ok(SecretFile::default());
    }

    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read secrets file: {}", path.display()))?;
    let file: SecretFile = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse secrets file: {}", path.display()))?;
    Ok(file)
}

fn write_secret_file(file: &SecretFile) -> Result<()> {
    let path = secrets_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create secrets dir: {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(file).context("failed to serialize secrets")?;
    std::fs::write(&path, text)
        .with_context(|| format!("failed to write secrets file: {}", path.display()))?;
    Ok(())
}

fn secrets_path() -> Result<PathBuf> {
    Ok(config_root_dir()?.join("secrets.json"))
}
