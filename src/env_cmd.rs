use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::config::{config_root_dir, load_profile, EnvProfile};

/// Lists all env profile names from config root.
pub fn list_profiles() -> Result<Vec<String>> {
    let dir = envs_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry.context("failed to read env profile directory entry")?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        names.push(stem.to_string());
    }

    names.sort();
    Ok(names)
}

/// Returns parsed profile for inspection.
pub fn get_profile(name: &str) -> Result<EnvProfile> {
    load_profile(name)
}

/// Validates one profile and returns warnings/errors.
pub fn validate_profile(name: &str) -> Result<Vec<String>> {
    let profile = load_profile(name)?;
    let mut issues = Vec::new();

    if let Some(base) = profile.base_url.as_deref() {
        let trimmed = base.trim();
        if !trimmed.is_empty()
            && !(trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        {
            issues.push(format!(
                "base_url should start with http:// or https://, got {trimmed:?}"
            ));
        }
    }

    for key in profile.variables.keys() {
        if key.trim().is_empty() {
            issues.push("variables contains empty key".to_string());
        }
    }

    for key in profile.headers.keys() {
        if key.trim().is_empty() {
            issues.push("headers contains empty key".to_string());
        }
    }

    Ok(issues)
}

fn envs_dir() -> Result<PathBuf> {
    let root = config_root_dir()?;
    let dir = root.join("envs");
    if dir.as_os_str().is_empty() {
        return Err(anyhow!("failed to resolve env directory"));
    }
    Ok(dir)
}
