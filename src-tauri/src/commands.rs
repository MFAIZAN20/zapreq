use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use zapreq::cli::{parse_cli_from, CliArgs, SeverityLevel};
use zapreq::collections::{
    create_workspace as core_create_workspace, delete_workspace as core_delete_workspace,
    list_requests, list_workspaces, load_request, load_workspace, load_workspace_request,
    save_request as save_legacy_request, CollectionEntry, Workspace, WorkspaceRequest,
    WorkspaceSummary,
};
use zapreq::config::{
    apply_profile, config_root_dir, load_config, load_profile, merge_defaults, CliResolved,
    EnvProfile,
};
use zapreq::env_cmd;
use zapreq::headers::{Header, HeaderSource};
use zapreq::items::parse_request_items;
use zapreq::localdb::open_connection;
use zapreq::regression::{
    delete_test_case as core_delete_test_case, list_test_cases, StoredTestCase,
};
use zapreq::request::{RequestEngine, RequestSpec};
use zapreq::response::ResponseData;
use zapreq::security::{run_scan_for_records, SecurityReport, SecurityScanOptions};
use zapreq::sources::{execute_record, RequestRecord};
use zapreq::testing::{evaluate_response, TestOptions, TestReport};
use zapreq::utils::{
    humanize_bytes, humanize_duration, is_binary, normalize_url,
    substitute_item_value_with_secrets as substitute_item_value,
    substitute_placeholders_with_secrets as substitute_placeholders,
};

const SETTINGS_FILE: &str = "desktop_settings.json";

#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceDto {
    pub name: String,
    pub description: String,
    pub request_count: usize,
    pub updated: String,
    pub requests: Vec<RequestDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestDto {
    pub id: Option<String>,
    pub name: String,
    pub method: String,
    pub url: String,
    pub items: Vec<String>,
    pub headers: Option<Vec<Header>>,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeaderDto {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponseDto {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub reason: String,
    pub final_url: String,
    pub headers: Vec<HeaderDto>,
    pub content_type: Option<String>,
    pub body: String,
    pub body_is_base64: bool,
    pub elapsed_ms: u64,
    pub elapsed_label: String,
    pub size_bytes: usize,
    pub size_label: String,
    pub test_results: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportDto {
    pub id: i64,
    pub module: String,
    pub name: String,
    pub summary: String,
    pub payload_json: String,
    pub created_at: String,
    pub method: Option<String>,
    pub url: Option<String>,
    pub final_url: Option<String>,
    pub status: Option<u16>,
    pub reason: Option<String>,
    pub elapsed_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub content_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub sidebar_width: f32,
    pub response_width: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sidebar_width: 280.0,
            response_width: 420.0,
        }
    }
}

impl AppSettings {
    fn sanitized(mut self) -> Self {
        self.sidebar_width = self.sidebar_width.clamp(240.0, 360.0);
        self.response_width = self.response_width.clamp(340.0, 620.0);
        self
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateWorkspacePayload {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateCollectionPayload {
    pub alias: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RequestLookupPayload {
    pub workspace: Option<String>,
    pub request: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SaveRequestPayload {
    pub id: Option<String>,
    pub workspace: Option<String>,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub headers: Option<Vec<Header>>,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SendRequestPayload {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub headers: Option<Vec<Header>>,
    pub env_profile: Option<String>,
    pub pre_request_script: Option<String>,
    pub post_response_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunSecurityScanPayload {
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub items: Vec<String>,
    pub env_profile: Option<String>,
    pub pre_request_script: Option<String>,
    pub severity: Option<String>,
    #[serde(default)]
    pub live_scan: bool,
    #[serde(default)]
    pub active_scan: bool,
    #[serde(default = "default_true")]
    pub include_sqli: bool,
    #[serde(default = "default_true")]
    pub include_xss: bool,
    #[serde(default = "default_true")]
    pub include_bola: bool,
    #[serde(default = "default_true")]
    pub include_rate_limit: bool,
    pub bola_session_a_profile: Option<String>,
    pub bola_session_b_profile: Option<String>,
    pub rate_limit_requests: Option<u32>,
    pub rate_limit_concurrency: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SaveEnvironmentPayload {
    pub name: String,
    pub profile: EnvProfile,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TestCasePayload {
    pub suite: String,
    pub name: String,
}

#[tauri::command]
pub fn get_workspaces() -> Result<Vec<WorkspaceDto>, String> {
    try_command(|| {
        list_workspaces()?
            .into_iter()
            .map(workspace_to_dto)
            .collect::<Result<Vec<_>>>()
    })
}

#[tauri::command]
pub fn create_workspace(payload: CreateWorkspacePayload) -> Result<WorkspaceDto, String> {
    try_command(|| {
        let mut workspace = core_create_workspace(payload.name.trim())?;
        if let Some(description) = payload.description {
            workspace.description = description.trim().to_string();
            zapreq::collections::save_workspace(&workspace)?;
        }
        let summary = WorkspaceSummary {
            name: workspace.name,
            description: workspace.description,
            request_count: workspace.requests.len(),
            updated: workspace.updated,
        };
        workspace_to_dto(summary)
    })
}

#[tauri::command]
pub fn delete_workspace(name: String) -> Result<(), String> {
    try_command(|| core_delete_workspace(name.trim()).map(|_| ()))
}

#[tauri::command]
pub fn delete_request(workspace: String, id: String) -> Result<(), String> {
    try_command(|| {
        let workspace = workspace.trim();
        let id = id.trim();
        if workspace.is_empty() {
            return Err(anyhow!("workspace name cannot be empty"));
        }
        let mut ws = load_workspace(workspace)?;
        ws.requests.retain(|r| r.id != id);
        ws.updated = chrono::Utc::now().to_rfc3339();
        zapreq::collections::save_workspace(&ws)?;
        Ok(())
    })
}

#[tauri::command]
pub fn rename_workspace(old_name: String, new_name: String) -> Result<(), String> {
    try_command(|| {
        let old_name = old_name.trim();
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(anyhow::anyhow!("new workspace name cannot be empty"));
        }
        let mut ws = load_workspace(old_name)?;
        core_delete_workspace(old_name)?;
        ws.name = new_name.to_string();
        zapreq::collections::save_workspace(&ws)?;
        Ok(())
    })
}

#[tauri::command]
pub fn get_collections() -> Result<Vec<RequestDto>, String> {
    try_command(|| Ok(list_requests()?.into_iter().map(legacy_to_dto).collect()))
}

#[tauri::command]
pub fn create_collection(payload: CreateCollectionPayload) -> Result<RequestDto, String> {
    try_command(|| {
        let cli = cli_from_parts(&payload.method, &payload.url, &payload.items)?;
        save_legacy_request(payload.alias.trim(), &cli)?;
        Ok(RequestDto {
            id: None,
            name: payload.alias,
            method: payload.method,
            url: payload.url,
            items: payload.items,
            headers: None,
            pre_request_script: None,
            post_response_script: None,
        })
    })
}

#[tauri::command]
pub fn get_request(payload: RequestLookupPayload) -> Result<RequestDto, String> {
    try_command(|| {
        if let Some(workspace) = payload
            .workspace
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let entry = load_workspace_request(workspace, &payload.request)?;
            Ok(collection_entry_to_dto(entry, Some(payload.request)))
        } else {
            Ok(legacy_to_dto(load_request(&payload.request)?))
        }
    })
}

fn map_v2_to_legacy(v2: &Option<Vec<Header>>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(headers) = v2 {
        for h in headers {
            if h.enabled {
                map.insert(h.name.clone(), h.value.clone());
            }
        }
    }
    map
}

fn map_legacy_headers_to_v2(legacy: &std::collections::HashMap<String, String>) -> Vec<Header> {
    legacy
        .iter()
        .map(|(k, v)| Header {
            name: k.clone(),
            value: v.clone(),
            enabled: true,
            sensitive: zapreq::headers::is_sensitive_header(k),
            source: HeaderSource::User,
        })
        .collect()
}

fn save_workspace_request_helper(
    ws: &mut Workspace,
    payload: &SaveRequestPayload,
    now: &str,
) -> String {
    let headers_v2 = payload.headers.clone();
    let headers = map_v2_to_legacy(&payload.headers);

    match &payload.id {
        Some(id) if !id.is_empty() => {
            if let Some(existing) = ws.requests.iter_mut().find(|r| r.id == *id) {
                existing.name = payload.name.clone();
                existing.method = payload.method.clone();
                existing.url = payload.url.clone();
                existing.items = payload.items.clone();
                existing.headers = headers;
                existing.headers_v2 = headers_v2;
                existing.pre_request_script = payload.pre_request_script.clone();
                existing.post_response_script = payload.post_response_script.clone();
                existing.updated = now.to_string();
                id.clone()
            } else {
                let new_id = uuid::Uuid::new_v4().to_string();
                ws.requests.push(WorkspaceRequest {
                    id: new_id.clone(),
                    name: payload.name.clone(),
                    method: payload.method.clone(),
                    url: payload.url.clone(),
                    items: payload.items.clone(),
                    headers,
                    headers_v2,
                    tests: Vec::new(),
                    pre_request_script: payload.pre_request_script.clone(),
                    post_response_script: payload.post_response_script.clone(),
                    created: now.to_string(),
                    updated: now.to_string(),
                });
                new_id
            }
        }
        _ => {
            if let Some(existing) = ws
                .requests
                .iter_mut()
                .find(|r| r.name.eq_ignore_ascii_case(&payload.name))
            {
                existing.method = payload.method.clone();
                existing.url = payload.url.clone();
                existing.items = payload.items.clone();
                existing.headers = headers;
                existing.headers_v2 = headers_v2;
                existing.pre_request_script = payload.pre_request_script.clone();
                existing.post_response_script = payload.post_response_script.clone();
                existing.updated = now.to_string();
                existing.id.clone()
            } else {
                let new_id = uuid::Uuid::new_v4().to_string();
                ws.requests.push(WorkspaceRequest {
                    id: new_id.clone(),
                    name: payload.name.clone(),
                    method: payload.method.clone(),
                    url: payload.url.clone(),
                    items: payload.items.clone(),
                    headers,
                    headers_v2,
                    tests: Vec::new(),
                    pre_request_script: payload.pre_request_script.clone(),
                    post_response_script: payload.post_response_script.clone(),
                    created: now.to_string(),
                    updated: now.to_string(),
                });
                new_id
            }
        }
    }
}

#[tauri::command]
pub fn save_request(payload: SaveRequestPayload) -> Result<RequestDto, String> {
    try_command(|| {
        let cli = cli_from_parts(&payload.method, &payload.url, &payload.items)?;

        let final_id = if let Some(workspace_name) = payload
            .workspace
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let mut ws = if list_workspaces()?
                .iter()
                .any(|ws| ws.name == workspace_name)
            {
                load_workspace(workspace_name)?
            } else {
                zapreq::collections::create_workspace(workspace_name)?
            };

            let now = chrono::Utc::now().to_rfc3339();
            let req_id = save_workspace_request_helper(&mut ws, &payload, &now);

            ws.updated = now;
            zapreq::collections::save_workspace(&ws)?;
            req_id
        } else {
            save_legacy_request(&payload.name, &cli)?;
            "".to_string()
        };

        Ok(RequestDto {
            id: if final_id.is_empty() {
                None
            } else {
                Some(final_id)
            },
            name: payload.name,
            method: payload.method,
            url: payload.url,
            items: payload.items,
            headers: payload.headers,
            pre_request_script: payload.pre_request_script,
            post_response_script: payload.post_response_script,
        })
    })
}

fn merge_payload_and_parsed_headers(
    payload_headers: &[Header],
    parsed_items: &[zapreq::items::RequestItem],
) -> Vec<Header> {
    let mut merged = payload_headers
        .iter()
        .map(|header| Header {
            name: header.name.clone(),
            value: header.value.clone(),
            enabled: header.enabled,
            sensitive: header.sensitive || zapreq::headers::is_sensitive_header(&header.name),
            source: header.source.clone(),
        })
        .collect::<Vec<_>>();

    let mut existing = HashSet::new();
    for header in &merged {
        existing.insert((header.name.to_ascii_lowercase(), header.value.clone()));
    }

    for header in zapreq::headers::headers_from_parsed_items(parsed_items, HeaderSource::User) {
        let key = (header.name.to_ascii_lowercase(), header.value.clone());
        if existing.insert(key) {
            merged.push(header);
        }
    }

    merged
}

fn resolve_final_headers_for_runtime(
    method: &str,
    resolved_url: &str,
    resolved_items: &[String],
    parsed_items: &[zapreq::items::RequestItem],
    payload_headers: Option<&[Header]>,
    env_profile: Option<&str>,
    variables: &HashMap<String, String>,
) -> Result<Vec<Header>> {
    let mut env_headers = Vec::new();
    if let Some(profile_name) = env_profile {
        if let Ok(profile) = load_profile(profile_name) {
            for (key, value) in profile.headers {
                env_headers.push(Header {
                    name: key.clone(),
                    value: zapreq::config::substitute_placeholders(&value, variables),
                    enabled: true,
                    sensitive: zapreq::headers::is_sensitive_header(&key),
                    source: HeaderSource::Environment,
                });
            }
        }
    }

    let collected = zapreq::items::collect_from_parsed(parsed_items)?;
    let has_file_uploads = !collected.files.is_empty();
    let cli = cli_from_parts(method, resolved_url, resolved_items)?;
    let body_type = if cli.multipart || has_file_uploads {
        "multipart"
    } else if cli.form {
        "form"
    } else if !collected.data_strings.is_empty() || !collected.data_json.is_empty() {
        "json"
    } else {
        "none"
    };
    let autos = zapreq::headers::get_auto_headers(body_type);

    let user_headers = if let Some(payload_headers) = payload_headers {
        merge_payload_and_parsed_headers(payload_headers, parsed_items)
    } else {
        zapreq::headers::headers_from_parsed_items(parsed_items, HeaderSource::User)
    };

    let final_headers = zapreq::headers::merge_headers(&[], &env_headers, &user_headers, &autos);
    Ok(final_headers
        .into_iter()
        .map(|mut header| {
            header.value = zapreq::config::substitute_placeholders(&header.value, variables);
            header
        })
        .collect())
}

#[tauri::command]
pub fn send_request(payload: SendRequestPayload) -> Result<ResponseDto, String> {
    try_command(|| {
        let config = load_config()?;
        let resolved = resolve_runtime_request(
            &config,
            &payload.method,
            payload.url,
            payload.items,
            payload.env_profile.as_deref(),
            payload.pre_request_script.as_deref(),
        )?;
        let parsed_items = parse_request_items(&resolved.items)?;
        let cli = cli_from_parts(&resolved.method, &resolved.url, &resolved.items)?;
        let final_headers = resolve_final_headers_for_runtime(
            &resolved.method,
            &resolved.url,
            &resolved.items,
            &parsed_items,
            payload.headers.as_deref(),
            payload.env_profile.as_deref(),
            &resolved.variables,
        )?;

        let spec = RequestSpec {
            method: resolved.method.clone(),
            url: resolved.url.clone(),
            items: parsed_items,
            headers: final_headers,
        };
        let started = Instant::now();
        let (trace, response) = RequestEngine::new().send(&cli, &spec, None)?;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        let mut test_results = Vec::new();
        let mut response_dto =
            response_to_dto(&trace.method, &trace.url, response, elapsed_ms, Vec::new());

        if let Some(script) = payload
            .post_response_script
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let mut variables = resolved.variables;
            if let Ok(tests) = run_post_response_script(script, &response_dto, &mut variables) {
                test_results = tests;
            }
        }
        response_dto.test_results = test_results;

        let _ = zapreq::localdb::record_http_report(
            "Tauri",
            &trace.method,
            &trace.url,
            &response_dto.final_url,
            response_dto.status,
            &response_dto.reason,
            elapsed_ms,
            response_dto.size_bytes,
            response_dto.content_type.as_deref(),
            &response_dto
                .headers
                .iter()
                .map(|header| (header.key.clone(), header.value.clone()))
                .collect::<Vec<_>>(),
            &response_dto.body,
        );

        Ok(response_dto)
    })
}

#[tauri::command]
pub fn run_security_scan(payload: RunSecurityScanPayload) -> Result<SecurityReport, String> {
    try_command(|| {
        let config = load_config()?;
        let resolved = resolve_runtime_request(
            &config,
            &payload.method,
            payload.url,
            payload.items,
            payload.env_profile.as_deref(),
            payload.pre_request_script.as_deref(),
        )?;
        let source_label = payload
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|name| format!("tauri:{name}"))
            .unwrap_or_else(|| format!("tauri:{} {}", resolved.method, resolved.url));
        let record = RequestRecord {
            name: payload
                .name
                .unwrap_or_else(|| format!("{} {}", resolved.method, resolved.url)),
            method: resolved.method.clone(),
            url: resolved.url.clone(),
            items: resolved.items,
            headers: HashMap::new(),
            source_label,
        };
        let options = SecurityScanOptions {
            live_scan: payload.live_scan,
            active_scan: payload.active_scan,
            include_sqli: payload.include_sqli,
            include_xss: payload.include_xss,
            include_bola: payload.include_bola,
            include_rate_limit: payload.include_rate_limit,
            env_profile: payload
                .env_profile
                .filter(|value| !value.trim().is_empty() && value != "none"),
            bola_session_a_profile: payload
                .bola_session_a_profile
                .filter(|value| !value.trim().is_empty() && value != "none"),
            bola_session_b_profile: payload
                .bola_session_b_profile
                .filter(|value| !value.trim().is_empty() && value != "none"),
            rate_limit_requests: payload.rate_limit_requests.unwrap_or(12),
            rate_limit_concurrency: payload.rate_limit_concurrency.unwrap_or(4),
        };
        let severity = parse_severity_level(payload.severity.as_deref())?;
        run_scan_for_records(
            record.source_label.clone(),
            vec![record],
            severity,
            &options,
            &config,
        )
    })
}

#[tauri::command]
pub fn get_environments() -> Result<Vec<String>, String> {
    try_command(env_cmd::list_profiles)
}

#[tauri::command]
pub fn save_environment(payload: SaveEnvironmentPayload) -> Result<(), String> {
    try_command(|| {
        let name = payload.name.trim();
        if name.is_empty() {
            return Err(anyhow!("environment name cannot be empty"));
        }
        if name.chars().any(|ch| {
            matches!(
                ch,
                '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        }) {
            return Err(anyhow!(
                "environment name contains invalid filesystem characters"
            ));
        }

        let dir = config_root_dir()?.join("envs");
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create env directory: {}", dir.display()))?;
        let path = dir.join(format!("{name}.json"));
        fs::write(path, serde_json::to_string_pretty(&payload.profile)?)?;
        Ok(())
    })
}

#[tauri::command]
pub fn get_reports() -> Result<Vec<ReportDto>, String> {
    try_command(|| {
        let conn = open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, module, name, summary, payload_json, created_at,
                    method, url, final_url, status, reason, elapsed_ms, size_bytes, content_type
             FROM reports
             ORDER BY id DESC
             LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            let payload_json: String = row.get(4)?;
            let payload = serde_json::from_str::<serde_json::Value>(&payload_json).ok();
            let method = row
                .get::<_, Option<String>>(6)?
                .or_else(|| payload_string(&payload, "method"));
            let url = row
                .get::<_, Option<String>>(7)?
                .or_else(|| payload_string(&payload, "url"));
            let final_url = row
                .get::<_, Option<String>>(8)?
                .or_else(|| payload_string(&payload, "final_url"));
            let status = row
                .get::<_, Option<i64>>(9)?
                .map(|value| value as u16)
                .or_else(|| payload_u64(&payload, "status").map(|value| value as u16));
            let reason = row
                .get::<_, Option<String>>(10)?
                .or_else(|| payload_string(&payload, "reason"));
            let elapsed_ms = row
                .get::<_, Option<i64>>(11)?
                .map(|value| value as u64)
                .or_else(|| payload_u64(&payload, "elapsed_ms"));
            let size_bytes = row
                .get::<_, Option<i64>>(12)?
                .map(|value| value as u64)
                .or_else(|| payload_u64(&payload, "size_bytes"));
            let content_type = row
                .get::<_, Option<String>>(13)?
                .or_else(|| payload_string(&payload, "content_type"));

            Ok(ReportDto {
                id: row.get(0)?,
                module: row.get(1)?,
                name: row.get(2)?,
                summary: row.get(3)?,
                payload_json,
                created_at: row.get(5)?,
                method,
                url,
                final_url,
                status,
                reason,
                elapsed_ms,
                size_bytes,
                content_type,
            })
        })?;

        let mut reports = Vec::new();
        for row in rows {
            reports.push(row?);
        }
        Ok(reports)
    })
}

fn payload_string(payload: &Option<serde_json::Value>, key: &str) -> Option<String> {
    payload
        .as_ref()?
        .get(key)?
        .as_str()
        .map(ToString::to_string)
}

fn payload_u64(payload: &Option<serde_json::Value>, key: &str) -> Option<u64> {
    payload.as_ref()?.get(key)?.as_u64()
}

#[tauri::command]
pub fn get_test_cases(suite: Option<String>) -> Result<Vec<StoredTestCase>, String> {
    try_command(|| list_test_cases(suite.as_deref()))
}

#[tauri::command]
pub fn run_test_case(payload: TestCasePayload) -> Result<TestReport, String> {
    try_command(|| {
        let cases = list_test_cases(Some(&payload.suite))?;
        let case = cases
            .into_iter()
            .find(|case| case.name == payload.name)
            .ok_or_else(|| anyhow!("test case not found: {}/{}", payload.suite, payload.name))?;
        let record = RequestRecord {
            name: case.name,
            method: case.method,
            url: case.url,
            items: case.items,
            headers: case.headers.into_iter().collect(),
            source_label: case.source_label,
        };
        let opts = TestOptions {
            expect_status: case.expect_status,
            expect_headers: case.expect_headers,
            expect_json: case.expect_json,
            expect_body_contains: case.expect_body_contains,
            max_time_ms: case.max_time_ms,
        };
        let config = load_config()?;
        let (trace, response, elapsed_ms) = execute_record(&record, &config)?;
        let report = evaluate_response(&trace.method, &trace.url, &response, elapsed_ms, &opts);
        let _ = zapreq::localdb::record_http_report(
            "Tauri Test",
            &trace.method,
            &trace.url,
            &response.final_url,
            response.status_code,
            &response.reason,
            elapsed_ms,
            response.body.len(),
            response.content_type.as_deref(),
            &response.headers,
            &String::from_utf8_lossy(&response.body),
        );
        Ok(report)
    })
}

#[tauri::command]
pub fn run_test_suite(suite: String) -> Result<zapreq::regression::SuiteRunReport, String> {
    try_command(|| {
        let config = load_config()?;
        zapreq::regression::run_suite(suite.trim(), &config)
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct TestRunDto {
    pub id: i64,
    pub suite: String,
    pub case_name: String,
    pub passed: bool,
    pub status_code: u16,
    pub elapsed_ms: u64,
    pub created_at: String,
}

#[tauri::command]
pub fn get_test_runs() -> Result<Vec<TestRunDto>, String> {
    try_command(|| {
        let conn = open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, suite, case_name, passed, status_code, elapsed_ms, created_at
             FROM test_runs
             ORDER BY id DESC
             LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TestRunDto {
                id: row.get(0)?,
                suite: row.get(1)?,
                case_name: row.get(2)?,
                passed: row.get::<_, i64>(3)? == 1,
                status_code: row.get::<_, i64>(4)? as u16,
                elapsed_ms: row.get::<_, i64>(5)? as u64,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn get_app_settings() -> Result<AppSettings, String> {
    try_command(load_app_settings)
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<AppSettings, String> {
    try_command(|| {
        let settings = settings.sanitized();
        let path = settings_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create settings dir: {}", parent.display()))?;
        }
        fs::write(&path, serde_json::to_string_pretty(&settings)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(settings)
    })
}

fn try_command<T>(f: impl FnOnce() -> Result<T>) -> Result<T, String> {
    f().map_err(|err| err.to_string())
}

fn workspace_to_dto(summary: WorkspaceSummary) -> Result<WorkspaceDto> {
    let workspace = load_workspace(&summary.name)?;
    let requests = workspace
        .requests
        .into_iter()
        .map(workspace_request_to_dto)
        .collect::<Vec<_>>();
    Ok(WorkspaceDto {
        name: summary.name,
        description: summary.description,
        request_count: summary.request_count,
        updated: summary.updated,
        requests,
    })
}

fn workspace_request_to_dto(request: WorkspaceRequest) -> RequestDto {
    let headers = Some(
        request
            .headers_v2
            .unwrap_or_else(|| map_legacy_headers_to_v2(&request.headers)),
    );
    RequestDto {
        id: Some(request.id),
        name: request.name,
        method: request.method,
        url: request.url,
        items: request.items,
        headers,
        pre_request_script: request.pre_request_script,
        post_response_script: request.post_response_script,
    }
}

fn legacy_to_dto(entry: CollectionEntry) -> RequestDto {
    collection_entry_to_dto(entry, None)
}

fn collection_entry_to_dto(entry: CollectionEntry, id: Option<String>) -> RequestDto {
    let headers = Some(
        entry
            .headers_v2
            .unwrap_or_else(|| map_legacy_headers_to_v2(&entry.headers)),
    );
    RequestDto {
        id,
        name: entry.alias,
        method: entry.method,
        url: entry.url,
        items: entry.items,
        headers,
        pre_request_script: None,
        post_response_script: None,
    }
}

fn cli_from_parts(method: &str, url: &str, items: &[String]) -> Result<CliArgs> {
    let config = load_config()?;
    let mut argv = vec!["zapreq".to_string(), method.to_string(), url.to_string()];
    argv.extend(items.iter().cloned());
    merge_defaults(&config, &mut argv);
    let mut cli = parse_cli_from(argv)?;
    cli.command = None;
    Ok(cli)
}

fn normalized_method(raw: &str) -> Result<String> {
    let method = raw.trim().to_ascii_uppercase();
    if method.is_empty() {
        return Err(anyhow!("HTTP method cannot be empty"));
    }
    Ok(method)
}

struct ResolvedRuntimeRequest {
    method: String,
    url: String,
    items: Vec<String>,
    variables: HashMap<String, String>,
}

fn resolve_runtime_request(
    config: &zapreq::config::Config,
    method_raw: &str,
    url: String,
    items: Vec<String>,
    env_profile: Option<&str>,
    pre_request_script: Option<&str>,
) -> Result<ResolvedRuntimeRequest> {
    let mut method = normalized_method(method_raw)?;
    let mut resolved = CliResolved {
        url,
        request_items: items,
        profile_headers: HashMap::new(),
        variables: HashMap::new(),
    };

    if let Some(profile_name) = env_profile
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "none")
    {
        let profile = load_profile(profile_name)?;
        apply_profile(&profile, &mut resolved);
    }

    let mut url = resolved.url.clone();
    let mut request_items = resolved.request_items.clone();
    let mut variables = resolved.variables.clone();

    if let Some(script) = pre_request_script.filter(|script| !script.trim().is_empty()) {
        let _ = run_pre_request_script(
            script,
            &mut method,
            &mut url,
            &mut request_items,
            &mut variables,
        );
    }

    let url = substitute_placeholders(&url, &variables);
    let mut final_items = request_items
        .iter()
        .map(|raw| substitute_item_value(raw, &variables))
        .collect::<Vec<_>>();
    for (key, value) in &resolved.profile_headers {
        final_items.push(format!(
            "{}:{}",
            substitute_placeholders(key, &variables),
            substitute_placeholders(value, &variables)
        ));
    }

    let url = normalize_url(&url, &config.default_scheme)?;

    Ok(ResolvedRuntimeRequest {
        method,
        url,
        items: final_items,
        variables,
    })
}

fn parse_severity_level(raw: Option<&str>) -> Result<SeverityLevel> {
    let normalized = raw.unwrap_or("low").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" => Ok(SeverityLevel::Low),
        "medium" => Ok(SeverityLevel::Medium),
        "high" => Ok(SeverityLevel::High),
        "critical" => Ok(SeverityLevel::Critical),
        _ => Err(anyhow!(
            "invalid severity '{}'; expected low, medium, high, or critical",
            normalized
        )),
    }
}

fn response_to_dto(
    method: &str,
    url: &str,
    response: ResponseData,
    elapsed_ms: u64,
    test_results: Vec<String>,
) -> ResponseDto {
    let body_is_base64 = is_binary(&response.body);
    let body = if body_is_base64 {
        base64::engine::general_purpose::STANDARD.encode(&response.body)
    } else {
        String::from_utf8_lossy(&response.body).into_owned()
    };
    let size_bytes = response.body.len();

    ResponseDto {
        method: method.to_string(),
        url: url.to_string(),
        status: response.status_code,
        reason: response.reason,
        final_url: response.final_url,
        headers: response
            .headers
            .into_iter()
            .map(|(key, value)| HeaderDto { key, value })
            .collect(),
        content_type: response.content_type,
        body,
        body_is_base64,
        elapsed_ms,
        elapsed_label: humanize_duration(elapsed_ms),
        size_bytes,
        size_label: humanize_bytes(size_bytes as u64),
        test_results,
    }
}

fn settings_path() -> Result<std::path::PathBuf> {
    Ok(config_root_dir()?.join(SETTINGS_FILE))
}

fn default_true() -> bool {
    true
}

fn load_app_settings() -> Result<AppSettings> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_str::<AppSettings>(&raw)
        .unwrap_or_default()
        .sanitized())
}

// ==========================================
// SCRIPT RUNNER AND UTILITIES FOR PRE/POST HOOKS
// ==========================================

fn parse_items_to_headers_and_params(items: &[String]) -> (HashMap<String, String>, Vec<String>) {
    let mut headers = HashMap::new();
    let mut params = Vec::new();
    for item in items {
        if let Some((k, v)) = item.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        } else {
            params.push(item.clone());
        }
    }
    (headers, params)
}

fn apply_updated_context(
    updated_context: &serde_json::Value,
    method: &mut String,
    url: &mut String,
    variables: &mut HashMap<String, String>,
    items: &mut Vec<String>,
    params: &[String],
) {
    if let Some(new_method) = updated_context.get("method").and_then(|v| v.as_str()) {
        *method = new_method.to_string();
    }
    if let Some(new_url) = updated_context.get("url").and_then(|v| v.as_str()) {
        *url = new_url.to_string();
    }
    if let Some(new_vars) = updated_context.get("variables").and_then(|v| v.as_object()) {
        for (k, v) in new_vars {
            if let Some(val_str) = v.as_str() {
                variables.insert(k.clone(), val_str.to_string());
            }
        }
    }
    if let Some(new_headers) = updated_context.get("headers").and_then(|v| v.as_object()) {
        let mut new_items = Vec::new();
        for (k, v) in new_headers {
            if let Some(val_str) = v.as_str() {
                new_items.push(format!("{}: {}", k, val_str));
            }
        }
        new_items.extend(params.to_vec());
        *items = new_items;
    }
}

fn run_pre_request_script(
    script: &str,
    method: &mut String,
    url: &mut String,
    items: &mut Vec<String>,
    variables: &mut HashMap<String, String>,
) -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("zapreq_pre_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let context_path = temp_dir.join("context.json");
    let script_path = temp_dir.join("script.js");

    let (headers, params) = parse_items_to_headers_and_params(items);

    let context = serde_json::json!({
        "method": method.clone(),
        "url": url.clone(),
        "headers": headers,
        "variables": variables.clone(),
    });

    std::fs::write(&context_path, serde_json::to_string_pretty(&context)?)?;

    let wrapper = format!(
        r#"
const fs = require('fs');
const context = JSON.parse(fs.readFileSync('{}', 'utf8'));

const pm = {{
    variables: {{
        get: (key) => context.variables[key],
        set: (key, val) => {{ context.variables[key] = String(val); }}
    }},
    request: {{
        url: context.url,
        method: context.method,
        headers: {{
            get: (key) => context.headers[key],
            set: (key, val) => {{ context.headers[key] = String(val); }}
        }}
    }}
}};

// Execute user script
(() => {{
    {}
}})();

fs.writeFileSync('{}', JSON.stringify(context, null, 2));
"#,
        context_path.to_string_lossy().replace('\\', "\\\\"),
        script,
        context_path.to_string_lossy().replace('\\', "\\\\")
    );

    std::fs::write(&script_path, wrapper)?;

    let output = std::process::Command::new("node")
        .arg(&script_path)
        .current_dir(&temp_dir)
        .output();

    let result = match output {
        Ok(out) => {
            if out.status.success() {
                let updated_data = std::fs::read_to_string(&context_path)?;
                let updated_context: serde_json::Value = serde_json::from_str(&updated_data)?;
                apply_updated_context(&updated_context, method, url, variables, items, &params);
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(anyhow!("Pre-request script failed: {}", stderr))
            }
        }
        Err(e) => Err(anyhow!(
            "Node.js execution failed: {}. Make sure Node.js is installed.",
            e
        )),
    };

    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

fn apply_updated_post_context(
    updated_context: &serde_json::Value,
    variables: &mut HashMap<String, String>,
) -> Vec<String> {
    if let Some(new_vars) = updated_context.get("variables").and_then(|v| v.as_object()) {
        for (k, v) in new_vars {
            if let Some(val_str) = v.as_str() {
                variables.insert(k.clone(), val_str.to_string());
            }
        }
    }

    let mut test_results = Vec::new();
    if let Some(tests_arr) = updated_context.get("tests").and_then(|v| v.as_array()) {
        for t in tests_arr {
            if let Some(t_str) = t.as_str() {
                test_results.push(t_str.to_string());
            }
        }
    }
    test_results
}

fn run_post_response_script(
    script: &str,
    response: &ResponseDto,
    variables: &mut HashMap<String, String>,
) -> Result<Vec<String>> {
    let temp_dir = std::env::temp_dir().join(format!("zapreq_post_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let context_path = temp_dir.join("context.json");
    let script_path = temp_dir.join("script.js");

    let context = serde_json::json!({
        "response": {
            "status": response.status,
            "reason": response.reason.clone(),
            "elapsed_ms": response.elapsed_ms,
            "size_bytes": response.size_bytes,
            "body": response.body.clone(),
        },
        "variables": variables.clone(),
        "tests": Vec::<String>::new(),
    });

    std::fs::write(&context_path, serde_json::to_string_pretty(&context)?)?;

    let wrapper = format!(
        r#"
const fs = require('fs');
const context = JSON.parse(fs.readFileSync('{}', 'utf8'));

const pm = {{
    variables: {{
        get: (key) => context.variables[key],
        set: (key, val) => {{ context.variables[key] = String(val); }}
    }},
    response: {{
        code: context.response.status,
        reason: context.response.reason,
        responseTime: context.response.elapsed_ms,
        responseSize: context.response.size_bytes,
        text: () => context.response.body,
        json: () => JSON.parse(context.response.body)
    }},
    test: (name, fn) => {{
        try {{
            fn();
            context.tests.push("PASS: " + name);
        }} catch (e) {{
            context.tests.push("FAIL: " + name + " (" + e.message + ")");
        }}
    }},
    expect: (val) => ({{
        toBe: (expected) => {{
            if (val !== expected) throw new Error("expected " + val + " to be " + expected);
        }},
        toContain: (expected) => {{
            if (!String(val).includes(expected)) throw new Error("expected " + val + " to contain " + expected);
        }}
    }})
}};

// Execute user script
(() => {{
    {}
}})();

fs.writeFileSync('{}', JSON.stringify(context, null, 2));
"#,
        context_path.to_string_lossy().replace('\\', "\\\\"),
        script,
        context_path.to_string_lossy().replace('\\', "\\\\")
    );

    std::fs::write(&script_path, wrapper)?;

    let output = std::process::Command::new("node")
        .arg(&script_path)
        .current_dir(&temp_dir)
        .output();

    let result = match output {
        Ok(out) => {
            if out.status.success() {
                let updated_data = std::fs::read_to_string(&context_path)?;
                let updated_context: serde_json::Value = serde_json::from_str(&updated_data)?;
                let test_results = apply_updated_post_context(&updated_context, variables);
                Ok(test_results)
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(anyhow!("Post-response script failed: {}", stderr))
            }
        }
        Err(e) => Err(anyhow!(
            "Node.js execution failed: {}. Make sure Node.js is installed.",
            e
        )),
    };

    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

// ==========================================
// IMPORT/EXPORT AND SECRETS TAURI COMMANDS
// ==========================================

#[tauri::command]
pub fn import_workspace(name: String, path: String) -> Result<(), String> {
    try_command(|| zapreq::collections::import_workspace(name.trim(), path.trim()).map(|_| ()))
}

#[tauri::command]
pub fn export_workspace(name: String, path: String, format: String) -> Result<String, String> {
    try_command(|| {
        let fmt = zapreq::collections::parse_export_format(format.trim())?;
        let output_path = zapreq::collections::export_workspace(name.trim(), path.trim(), fmt)?;
        Ok(output_path.display().to_string())
    })
}

#[tauri::command]
pub fn get_secrets() -> Result<Vec<String>, String> {
    try_command(zapreq::secrets::list_secret_keys)
}

#[tauri::command]
pub fn set_secret(key: String, value: String) -> Result<(), String> {
    try_command(|| zapreq::secrets::set_secret(key.trim(), &value))
}

#[tauri::command]
pub fn delete_secret(key: String) -> Result<(), String> {
    try_command(|| zapreq::secrets::delete_secret(key.trim()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddTestCasePayload {
    pub suite: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub items: Vec<String>,
    pub expect_status: Option<u16>,
    pub expect_headers: Vec<String>,
    pub expect_json: Vec<String>,
    pub expect_body_contains: Vec<String>,
    pub max_time_ms: Option<u64>,
}

#[tauri::command]
pub fn save_test_case(payload: AddTestCasePayload) -> Result<(), String> {
    try_command(|| {
        let suite = payload.suite.trim();
        let name = payload.name.trim();
        if suite.is_empty() || name.is_empty() {
            return Err(anyhow!("Suite and name cannot be empty"));
        }

        let conn = open_connection()?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO test_cases (
                suite, name, source_label, method, url, items_json, headers_json,
                expect_status, expect_headers_json, expect_json_json, expect_body_contains_json,
                max_time_ms, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(suite, name) DO UPDATE SET
                method=excluded.method,
                url=excluded.url,
                items_json=excluded.items_json,
                expect_status=excluded.expect_status,
                expect_headers_json=excluded.expect_headers_json,
                expect_json_json=excluded.expect_json_json,
                expect_body_contains_json=excluded.expect_body_contains_json,
                max_time_ms=excluded.max_time_ms,
                updated_at=excluded.updated_at",
            params![
                suite,
                name,
                "Tauri GUI",
                payload.method,
                payload.url,
                serde_json::to_string(&payload.items)?,
                "[]", // empty headers array
                payload.expect_status.map(i64::from),
                serde_json::to_string(&payload.expect_headers)?,
                serde_json::to_string(&payload.expect_json)?,
                serde_json::to_string(&payload.expect_body_contains)?,
                payload.max_time_ms.map(|v| v as i64),
                now,
                now,
            ],
        )
        .context("failed to save test case")?;
        Ok(())
    })
}

#[tauri::command]
pub fn delete_test_case(suite: String, name: String) -> Result<bool, String> {
    try_command(|| core_delete_test_case(suite.trim(), name.trim()))
}

#[tauri::command]
pub fn get_presets() -> Result<Vec<String>, String> {
    try_command(|| zapreq::header_presets::list_presets())
}

#[tauri::command]
pub fn get_preset(name: String) -> Result<Vec<Header>, String> {
    try_command(|| zapreq::header_presets::load_preset(&name))
}

#[tauri::command]
pub fn create_preset(name: String, headers: Vec<Header>) -> Result<(), String> {
    try_command(|| zapreq::header_presets::save_preset(&name, &headers))
}

#[tauri::command]
pub fn delete_preset(name: String) -> Result<(), String> {
    try_command(|| zapreq::header_presets::delete_preset(&name))
}

#[tauri::command]
pub fn get_header_suggestions() -> Result<Vec<zapreq::headers::HeaderSuggestion>, String> {
    try_command(|| Ok(zapreq::headers::header_suggestions()))
}

#[tauri::command]
pub fn get_merged_headers(
    method: String,
    url: String,
    items: Vec<String>,
    user_headers: Vec<Header>,
    env_profile: Option<String>,
) -> Result<Vec<Header>, String> {
    try_command(|| {
        let config = load_config()?;
        let resolved =
            resolve_runtime_request(&config, &method, url, items, env_profile.as_deref(), None)?;

        let parsed_items = parse_request_items(&resolved.items)?;
        resolve_final_headers_for_runtime(
            &method,
            &resolved.url,
            &resolved.items,
            &parsed_items,
            Some(&user_headers),
            env_profile.as_deref(),
            &resolved.variables,
        )
    })
}

#[tauri::command]
pub fn validate_request_headers(
    headers: Vec<Header>,
    url: String,
    body_type: String,
    body_content: Option<String>,
) -> Result<Vec<zapreq::headers::HeaderWarning>, String> {
    try_command(|| {
        let is_unencrypted = url.to_ascii_lowercase().starts_with("http://");
        Ok(zapreq::headers::validate_headers(
            &headers,
            &body_type,
            body_content.as_deref(),
            is_unencrypted,
        ))
    })
}
