use crate::config::config_root_dir;
use crate::headers::{Header, HeaderSource};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Resolves presets storage directory.
pub fn presets_dir() -> Result<PathBuf> {
    let dir = config_root_dir()?.join("presets");
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create presets directory: {}", dir.display()))?;
    }
    Ok(dir)
}

/// Saves preset headers by name.
pub fn save_preset(name: &str, headers: &[Header]) -> Result<()> {
    let dir = presets_dir()?;
    let path = dir.join(format!("{}.json", name));
    let text =
        serde_json::to_string_pretty(headers).context("failed to serialize preset headers")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write preset file: {}", path.display()))?;
    Ok(())
}

/// Loads preset headers by name.
pub fn load_preset(name: &str) -> Result<Vec<Header>> {
    let dir = presets_dir()?;
    let path = dir.join(format!("{}.json", name));
    if !path.exists() {
        return Err(anyhow::anyhow!("Preset '{}' not found", name));
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read preset file: {}", path.display()))?;
    let mut headers: Vec<Header> = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse preset JSON: {}", path.display()))?;
    for h in &mut headers {
        h.source = HeaderSource::Preset;
    }
    Ok(headers)
}

/// Deletes preset headers by name.
pub fn delete_preset(name: &str) -> Result<()> {
    let dir = presets_dir()?;
    let path = dir.join(format!("{}.json", name));
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete preset file: {}", path.display()))?;
    }
    Ok(())
}

/// Lists all saved preset names.
pub fn list_presets() -> Result<Vec<String>> {
    let dir = presets_dir()?;
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}
