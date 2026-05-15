use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::config::Config;
use crate::plugins::{list_plugins, plugins_dir, print_plugins};

/// Prints community plugin installation guidance.
pub fn install_plugin(name: &str) -> Result<()> {
    println!("To install a zapreq plugin:");
    println!("  1. cargo install zapreq-plugin-{name}");
    println!("  2. Place the .toml manifest in ~/.config/zapreq/plugins/");
    println!("  See https://github.com/MFAIZAN20/zapreq/wiki/plugins");
    Ok(())
}

/// Removes plugin manifest from plugins directory.
pub fn uninstall_plugin(name: &str, config: &Config) -> Result<()> {
    let dir = plugins_dir(config);
    let manifest = dir.join(format!("{name}.toml"));
    if manifest.exists() {
        std::fs::remove_file(&manifest)
            .with_context(|| format!("failed to remove plugin manifest: {}", manifest.display()))?;
        println!("Removed plugin manifest: {}", manifest.display());
    } else {
        println!("Plugin manifest not found: {}", manifest.display());
    }
    Ok(())
}

/// Lists and prints plugins.
pub fn print_plugin_list(config: &Config) -> Result<()> {
    let plugins = list_plugins(config);
    print_plugins(&plugins);
    Ok(())
}

/// Validates manifest files in plugins directory and returns issue count.
pub fn validate_plugins(config: &Config) -> Result<usize> {
    let dir = plugins_dir(config);
    if !dir.exists() {
        println!("Plugins directory does not exist: {}", dir.display());
        return Ok(0);
    }

    let mut issues = 0usize;
    let plugins = list_plugins(config);
    for plugin in plugins {
        if plugin.version != "built-in" {
            if plugin.name.trim().is_empty() {
                println!("Invalid plugin: empty name");
                issues += 1;
            }
            if let Some(exec) = plugin.executable.as_deref() {
                let path = resolve_plugin_executable_path(&dir, exec);
                if !path.exists() {
                    println!(
                        "Plugin '{}' executable not found: {}",
                        plugin.name,
                        path.display()
                    );
                    issues += 1;
                }
            }
        }
    }

    if issues == 0 {
        println!("All plugin manifests look valid.");
    }
    Ok(issues)
}

/// Runs a plugin executable with passthrough args.
pub fn run_plugin_command(name: &str, args: &[String], config: &Config) -> Result<i32> {
    let plugins = list_plugins(config);
    let Some(plugin) = plugins.into_iter().find(|p| p.name == name) else {
        println!("Plugin not found: {name}");
        return Ok(1);
    };

    let Some(executable) = plugin.executable else {
        println!(
            "Plugin '{}' has no executable configured. Add `executable = \"...\"` in manifest.",
            name
        );
        return Ok(1);
    };

    let dir = plugins_dir(config);
    let exe_path = resolve_plugin_executable_path(&dir, &executable);
    let status = std::process::Command::new(&exe_path)
        .args(args)
        .status()
        .with_context(|| format!("failed to launch plugin executable: {}", exe_path.display()))?;
    Ok(status.code().unwrap_or(1))
}

fn resolve_plugin_executable_path(plugins_dir: &std::path::Path, executable: &str) -> PathBuf {
    let path = PathBuf::from(executable);
    if path.is_absolute() {
        return path;
    }
    plugins_dir.join(path)
}
