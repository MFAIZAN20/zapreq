use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::cli::CliArgs;
use crate::config::config_root_dir;

/// Persisted legacy request collection entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionEntry {
    pub alias: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub created: String,
}

/// Workspace request contract for structured request storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRequest {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub tests: Vec<String>,
    pub created: String,
    pub updated: String,
}

/// Workspace model for grouped requests.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default = "workspace_version")]
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub requests: Vec<WorkspaceRequest>,
    pub created: String,
    pub updated: String,
}

/// Workspace list summary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub name: String,
    pub request_count: usize,
    pub updated: String,
}

/// Supported export formats for workspace export.
#[derive(Clone, Debug)]
pub enum WorkspaceExportFormat {
    Zapreq,
    Postman,
    OpenApi,
}

/// Output of migration from legacy alias files to workspace requests.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationReport {
    pub workspace: String,
    pub imported: usize,
    pub skipped_existing: usize,
}

/// Saves a named legacy request collection from current CLI arguments.
pub fn save_request(alias: &str, cli: &CliArgs) -> Result<()> {
    let entry = CollectionEntry {
        alias: alias.to_string(),
        method: cli.method.clone(),
        url: cli.url.clone(),
        items: cli.request_items.clone(),
        headers: HashMap::new(),
        created: Utc::now().to_rfc3339(),
    };
    save_collection_entry(&entry)
}

/// Loads and prints summary for a named legacy request collection run.
pub fn run_request(alias: &str, profile: Option<&str>) -> Result<()> {
    let entry = load_request(alias)?;
    if let Some(profile) = profile {
        eprintln!(
            "Running saved request '{}' with env profile '{}'",
            entry.alias, profile
        );
    } else {
        eprintln!("Running saved request '{}'", entry.alias);
    }
    Ok(())
}

/// Lists all saved legacy request collections.
pub fn list_requests() -> Result<Vec<CollectionEntry>> {
    let dir = collections_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("failed to read collections dir: {}", dir.display()))?
    {
        let entry = entry.context("failed to read collection directory entry")?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read collection: {}", path.display()))?;
        let parsed: CollectionEntry = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse collection: {}", path.display()))?;
        out.push(parsed);
    }
    out.sort_by(|a, b| a.alias.cmp(&b.alias));
    Ok(out)
}

/// Deletes a saved legacy request collection by alias.
pub fn delete_request(alias: &str) -> Result<()> {
    let path = collection_path(alias)?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to delete collection: {}", path.display()))?;
    }
    Ok(())
}

/// Loads a legacy collection entry by alias.
pub fn load_request(alias: &str) -> Result<CollectionEntry> {
    let path = collection_path(alias)?;
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read collection: {}", path.display()))?;
    let entry: CollectionEntry = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse collection: {}", path.display()))?;
    Ok(entry)
}

/// Creates a new empty workspace file if it does not exist.
pub fn create_workspace(name: &str) -> Result<Workspace> {
    validate_workspace_name(name)?;
    let path = workspace_path(name)?;
    if path.exists() {
        return load_workspace(name);
    }
    let now = Utc::now().to_rfc3339();
    let ws = Workspace {
        version: workspace_version(),
        name: name.to_string(),
        requests: Vec::new(),
        created: now.clone(),
        updated: now,
    };
    save_workspace(&ws)?;
    Ok(ws)
}

/// Lists workspace summaries.
pub fn list_workspaces() -> Result<Vec<WorkspaceSummary>> {
    let dir = workspaces_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("failed to read workspaces dir: {}", dir.display()))?
    {
        let entry = entry.context("failed to read workspace directory entry")?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read workspace file: {}", path.display()))?;
        let ws: Workspace = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse workspace file: {}", path.display()))?;
        out.push(WorkspaceSummary {
            name: ws.name,
            request_count: ws.requests.len(),
            updated: ws.updated,
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Loads one workspace by name.
pub fn load_workspace(name: &str) -> Result<Workspace> {
    validate_workspace_name(name)?;
    let path = workspace_path(name)?;
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read workspace: {}", path.display()))?;
    let ws: Workspace = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse workspace: {}", path.display()))?;
    Ok(ws)
}

/// Persists one workspace to disk.
pub fn save_workspace(workspace: &Workspace) -> Result<()> {
    validate_workspace_name(&workspace.name)?;
    let path = workspace_path(&workspace.name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create workspace dir: {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(workspace).context("failed to encode workspace JSON")?;
    std::fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Adds a request to an existing workspace.
pub fn add_request_to_workspace(
    workspace_name: &str,
    request_name: &str,
    cli: &CliArgs,
) -> Result<()> {
    let mut ws = if workspace_path(workspace_name)?.exists() {
        load_workspace(workspace_name)?
    } else {
        create_workspace(workspace_name)?
    };

    let now = Utc::now().to_rfc3339();
    if let Some(existing) = ws
        .requests
        .iter_mut()
        .find(|r| r.name.eq_ignore_ascii_case(request_name))
    {
        existing.method = cli.method.clone();
        existing.url = cli.url.clone();
        existing.items = cli.request_items.clone();
        existing.updated = now.clone();
    } else {
        ws.requests.push(WorkspaceRequest {
            id: Uuid::new_v4().to_string(),
            name: request_name.to_string(),
            method: cli.method.clone(),
            url: cli.url.clone(),
            items: cli.request_items.clone(),
            headers: HashMap::new(),
            tests: Vec::new(),
            created: now.clone(),
            updated: now.clone(),
        });
    }
    ws.updated = now;
    save_workspace(&ws)
}

/// Lists all requests in a workspace.
pub fn list_workspace_requests(workspace_name: &str) -> Result<Vec<WorkspaceRequest>> {
    let mut ws = load_workspace(workspace_name)?;
    ws.requests.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ws.requests)
}

/// Loads one workspace request by name or id and returns a collection-style request payload.
pub fn load_workspace_request(workspace_name: &str, request_ref: &str) -> Result<CollectionEntry> {
    let ws = load_workspace(workspace_name)?;
    let Some(req) = ws
        .requests
        .iter()
        .find(|r| r.name == request_ref || r.id == request_ref)
    else {
        return Err(anyhow!(
            "request '{}' not found in workspace '{}'",
            request_ref,
            workspace_name
        ));
    };
    Ok(CollectionEntry {
        alias: req.name.clone(),
        method: req.method.clone(),
        url: req.url.clone(),
        items: req.items.clone(),
        headers: req.headers.clone(),
        created: req.created.clone(),
    })
}

/// Imports a workspace from zapreq/postman/openapi JSON.
pub fn import_workspace(name: &str, path: &str) -> Result<Workspace> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read import file: {path}"))?;
    let value: Value =
        serde_json::from_str(&raw).with_context(|| format!("failed to parse JSON: {path}"))?;

    let ws = if value.get("requests").is_some() {
        let mut ws: Workspace =
            serde_json::from_value(value).context("failed to decode zapreq workspace JSON")?;
        ws.name = name.to_string();
        if ws.version == 0 {
            ws.version = workspace_version();
        }
        if ws.created.trim().is_empty() {
            ws.created = Utc::now().to_rfc3339();
        }
        ws.updated = Utc::now().to_rfc3339();
        ws
    } else if value.get("item").is_some() && value.get("info").is_some() {
        parse_postman_import(name, &value)?
    } else if value.get("paths").is_some() && value.get("openapi").is_some() {
        parse_openapi_import(name, &value)?
    } else if value.get("alias").is_some()
        && value.get("method").is_some()
        && value.get("url").is_some()
    {
        let entry: CollectionEntry =
            serde_json::from_value(value).context("failed to decode legacy request JSON")?;
        let now = Utc::now().to_rfc3339();
        Workspace {
            version: workspace_version(),
            name: name.to_string(),
            requests: vec![WorkspaceRequest {
                id: Uuid::new_v4().to_string(),
                name: entry.alias,
                method: entry.method,
                url: entry.url,
                items: entry.items,
                headers: entry.headers,
                tests: Vec::new(),
                created: now.clone(),
                updated: now.clone(),
            }],
            created: now.clone(),
            updated: now,
        }
    } else {
        return Err(anyhow!(
            "unsupported import format: expected zapreq workspace, postman collection, openapi, or legacy request"
        ));
    };

    save_workspace(&ws)?;
    Ok(ws)
}

/// Exports a workspace in the requested format.
pub fn export_workspace(
    workspace_name: &str,
    path: &str,
    format: WorkspaceExportFormat,
) -> Result<()> {
    let ws = load_workspace(workspace_name)?;
    let payload = match format {
        WorkspaceExportFormat::Zapreq => {
            serde_json::to_value(&ws).context("failed to encode workspace JSON")?
        }
        WorkspaceExportFormat::Postman => workspace_to_postman(&ws),
        WorkspaceExportFormat::OpenApi => workspace_to_openapi(&ws),
    };

    let text = serde_json::to_string_pretty(&payload).context("failed to encode export JSON")?;
    std::fs::write(path, text).with_context(|| format!("failed to write export file: {path}"))?;
    Ok(())
}

/// Migrates legacy saved requests into a workspace.
pub fn migrate_legacy_collections(workspace_name: &str) -> Result<MigrationReport> {
    let legacy = list_requests()?;
    let mut ws = if workspace_path(workspace_name)?.exists() {
        load_workspace(workspace_name)?
    } else {
        create_workspace(workspace_name)?
    };

    let mut existing = HashSet::new();
    for req in &ws.requests {
        existing.insert(req.name.to_ascii_lowercase());
    }

    let mut imported = 0usize;
    let mut skipped = 0usize;
    let now = Utc::now().to_rfc3339();
    for entry in legacy {
        let key = entry.alias.to_ascii_lowercase();
        if existing.contains(&key) {
            skipped += 1;
            continue;
        }
        ws.requests.push(WorkspaceRequest {
            id: Uuid::new_v4().to_string(),
            name: entry.alias.clone(),
            method: entry.method.clone(),
            url: entry.url.clone(),
            items: entry.items.clone(),
            headers: entry.headers.clone(),
            tests: Vec::new(),
            created: now.clone(),
            updated: now.clone(),
        });
        existing.insert(key);
        imported += 1;
    }
    ws.updated = now;
    save_workspace(&ws)?;
    Ok(MigrationReport {
        workspace: workspace_name.to_string(),
        imported,
        skipped_existing: skipped,
    })
}

/// Parses export format string.
pub fn parse_export_format(raw: &str) -> Result<WorkspaceExportFormat> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "zapreq" | "json" => Ok(WorkspaceExportFormat::Zapreq),
        "postman" => Ok(WorkspaceExportFormat::Postman),
        "openapi" | "oas" => Ok(WorkspaceExportFormat::OpenApi),
        _ => Err(anyhow!(
            "unsupported export format '{}'; use zapreq|postman|openapi",
            raw
        )),
    }
}

fn workspace_to_postman(ws: &Workspace) -> Value {
    let items = ws
        .requests
        .iter()
        .map(|req| {
            let headers = req
                .headers
                .iter()
                .map(|(k, v)| json!({ "key": k, "value": v }))
                .collect::<Vec<_>>();
            json!({
                "name": req.name,
                "request": {
                    "method": req.method,
                    "header": headers,
                    "url": {
                        "raw": req.url
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "info": {
            "name": ws.name,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": items
    })
}

fn workspace_to_openapi(ws: &Workspace) -> Value {
    let mut paths = serde_json::Map::new();
    for req in &ws.requests {
        let (path_key, method) = openapi_path_and_method(req);
        let method_key = method.to_ascii_lowercase();

        let path_entry = paths
            .entry(path_key.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(obj) = path_entry {
            obj.insert(
                method_key,
                json!({
                    "operationId": req.id,
                    "summary": req.name,
                    "responses": {
                        "default": { "description": "default response" }
                    }
                }),
            );
        }
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": ws.name,
            "version": "1.0.0"
        },
        "paths": Value::Object(paths)
    })
}

fn parse_postman_import(name: &str, value: &Value) -> Result<Workspace> {
    let items = value
        .get("item")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("invalid postman import: 'item' must be an array"))?;

    let mut requests = Vec::new();
    let now = Utc::now().to_rfc3339();
    for item in items {
        let request = item
            .get("request")
            .ok_or_else(|| anyhow!("invalid postman item: missing request"))?;
        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_string();
        let url = extract_postman_url(request).unwrap_or_default();
        let req_name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("request")
            .to_string();
        requests.push(WorkspaceRequest {
            id: Uuid::new_v4().to_string(),
            name: req_name,
            method,
            url,
            items: Vec::new(),
            headers: HashMap::new(),
            tests: Vec::new(),
            created: now.clone(),
            updated: now.clone(),
        });
    }

    Ok(Workspace {
        version: workspace_version(),
        name: name.to_string(),
        requests,
        created: now.clone(),
        updated: now,
    })
}

fn parse_openapi_import(name: &str, value: &Value) -> Result<Workspace> {
    let paths = value
        .get("paths")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("invalid openapi import: missing paths object"))?;

    let now = Utc::now().to_rfc3339();
    let mut requests = Vec::new();
    for (path, methods) in paths {
        let Some(method_map) = methods.as_object() else {
            continue;
        };
        for (method, operation) in method_map {
            if !is_http_method(method) {
                continue;
            }
            let summary = operation
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or(path);
            requests.push(WorkspaceRequest {
                id: Uuid::new_v4().to_string(),
                name: summary.to_string(),
                method: method.to_ascii_uppercase(),
                url: path.to_string(),
                items: Vec::new(),
                headers: HashMap::new(),
                tests: Vec::new(),
                created: now.clone(),
                updated: now.clone(),
            });
        }
    }

    Ok(Workspace {
        version: workspace_version(),
        name: name.to_string(),
        requests,
        created: now.clone(),
        updated: now,
    })
}

fn extract_postman_url(request: &Value) -> Option<String> {
    let url = request.get("url")?;
    if let Some(raw) = url.get("raw").and_then(|v| v.as_str()) {
        return Some(raw.to_string());
    }
    url.as_str().map(|s| s.to_string())
}

fn openapi_path_and_method(req: &WorkspaceRequest) -> (String, String) {
    if let Ok(parsed) = reqwest::Url::parse(&req.url) {
        let mut path = parsed.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        (path, req.method.clone())
    } else if req.url.starts_with('/') {
        (req.url.clone(), req.method.clone())
    } else {
        (
            format!("/{}", req.url.trim_start_matches('/')),
            req.method.clone(),
        )
    }
}

fn save_collection_entry(entry: &CollectionEntry) -> Result<()> {
    let path = collection_path(&entry.alias)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create collections dir: {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(entry).context("failed to encode collection JSON")?;
    std::fs::write(&path, json).with_context(|| format!("failed to save {}", path.display()))?;
    Ok(())
}

fn collections_dir() -> Result<PathBuf> {
    let root = config_root_dir()?;
    Ok(root.join("collections"))
}

fn collection_path(alias: &str) -> Result<PathBuf> {
    Ok(collections_dir()?.join(format!("{alias}.json")))
}

fn workspaces_dir() -> Result<PathBuf> {
    let root = config_root_dir()?;
    Ok(root.join("workspaces"))
}

fn workspace_path(name: &str) -> Result<PathBuf> {
    Ok(workspaces_dir()?.join(format!("{name}.json")))
}

fn workspace_version() -> u32 {
    1
}

fn validate_workspace_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("workspace name cannot be empty"));
    }
    if name.chars().any(|ch| {
        matches!(
            ch,
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        )
    }) {
        return Err(anyhow!(
            "workspace name contains invalid filesystem characters"
        ));
    }
    Ok(())
}

fn is_http_method(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

#[allow(dead_code)]
fn _path_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_export_format, parse_openapi_import, parse_postman_import, workspace_to_openapi,
        workspace_to_postman, Workspace, WorkspaceExportFormat, WorkspaceRequest,
    };

    #[test]
    fn export_format_parse_works() {
        assert!(matches!(
            parse_export_format("postman").expect("postman format should parse"),
            WorkspaceExportFormat::Postman
        ));
        assert!(parse_export_format("unknown").is_err());
    }

    #[test]
    fn postman_conversion_has_item() {
        let ws = Workspace {
            version: 1,
            name: "demo".to_string(),
            requests: vec![WorkspaceRequest {
                id: "1".to_string(),
                name: "hello".to_string(),
                method: "GET".to_string(),
                url: "https://example.com/hello".to_string(),
                items: vec![],
                headers: Default::default(),
                tests: vec![],
                created: "2026-01-01T00:00:00Z".to_string(),
                updated: "2026-01-01T00:00:00Z".to_string(),
            }],
            created: "2026-01-01T00:00:00Z".to_string(),
            updated: "2026-01-01T00:00:00Z".to_string(),
        };
        let value = workspace_to_postman(&ws);
        assert!(value.get("item").is_some());
    }

    #[test]
    fn openapi_conversion_has_paths() {
        let ws = Workspace {
            version: 1,
            name: "demo".to_string(),
            requests: vec![WorkspaceRequest {
                id: "1".to_string(),
                name: "hello".to_string(),
                method: "GET".to_string(),
                url: "https://example.com/hello".to_string(),
                items: vec![],
                headers: Default::default(),
                tests: vec![],
                created: "2026-01-01T00:00:00Z".to_string(),
                updated: "2026-01-01T00:00:00Z".to_string(),
            }],
            created: "2026-01-01T00:00:00Z".to_string(),
            updated: "2026-01-01T00:00:00Z".to_string(),
        };
        let value = workspace_to_openapi(&ws);
        assert!(value.get("paths").is_some());
    }

    #[test]
    fn postman_import_parses() {
        let v = serde_json::json!({
            "info": { "name": "demo" },
            "item": [
                {
                    "name": "users",
                    "request": {
                        "method": "GET",
                        "url": { "raw": "https://example.com/users" }
                    }
                }
            ]
        });
        let ws = parse_postman_import("demo", &v).expect("postman import should parse");
        assert_eq!(ws.requests.len(), 1);
        assert_eq!(ws.requests[0].method, "GET");
    }

    #[test]
    fn openapi_import_parses() {
        let v = serde_json::json!({
            "openapi": "3.1.0",
            "paths": {
                "/users": {
                    "get": {
                        "summary": "list users"
                    }
                }
            }
        });
        let ws = parse_openapi_import("demo", &v).expect("openapi import should parse");
        assert_eq!(ws.requests.len(), 1);
        assert_eq!(ws.requests[0].method, "GET");
    }
}
