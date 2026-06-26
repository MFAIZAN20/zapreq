use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::{SeverityLevel, SourceSelector};
use crate::config::{apply_profile, load_profile, CliResolved, Config};
use crate::items::{parse_request_items, RequestItem};
use crate::localdb::{open_connection, record_report};
use crate::request::{RequestEngine, RequestSpec};
use crate::response::ResponseData;
use crate::sources::{build_cli_for_record, execute_record, resolve_records, RequestRecord};
use crate::utils::normalize_url;

#[derive(Clone, Debug, Serialize)]
pub struct SecurityFinding {
    pub endpoint: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub impact: String,
    pub remediation: String,
    pub risk_score: u8,
    pub evidence: String,
    pub method: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SecurityCheckSummary {
    pub name: String,
    pub target: String,
    pub status: String,
    pub attempts: u32,
    pub findings: usize,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SecurityReport {
    pub source: String,
    pub generated_at: String,
    pub live_scan: bool,
    pub active_scan: bool,
    pub env_profile: Option<String>,
    pub request_count: usize,
    pub findings: Vec<SecurityFinding>,
    pub checks: Vec<SecurityCheckSummary>,
    pub report_id: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecurityScanOptions {
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
    #[serde(default)]
    pub env_profile: Option<String>,
    #[serde(default)]
    pub bola_session_a_profile: Option<String>,
    #[serde(default)]
    pub bola_session_b_profile: Option<String>,
    #[serde(default = "default_rate_limit_requests")]
    pub rate_limit_requests: u32,
    #[serde(default = "default_rate_limit_concurrency")]
    pub rate_limit_concurrency: u32,
}

impl Default for SecurityScanOptions {
    fn default() -> Self {
        Self {
            live_scan: false,
            active_scan: false,
            include_sqli: true,
            include_xss: true,
            include_bola: true,
            include_rate_limit: true,
            env_profile: None,
            bola_session_a_profile: None,
            bola_session_b_profile: None,
            rate_limit_requests: default_rate_limit_requests(),
            rate_limit_concurrency: default_rate_limit_concurrency(),
        }
    }
}

impl SecurityScanOptions {
    fn active_checks_enabled(&self) -> bool {
        self.active_scan
            || (self.include_bola
                && self.bola_session_a_profile.is_some()
                && self.bola_session_b_profile.is_some())
    }
}

#[derive(Clone)]
struct ObservedResponse {
    status: u16,
    reason: String,
    content_type: Option<String>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    elapsed_ms: u64,
}

#[derive(Clone)]
struct RateLimitObservation {
    status: Option<u16>,
    headers: Vec<(String, String)>,
    elapsed_ms: u64,
    error: Option<String>,
}

struct RateLimitProbeMetrics {
    throttled: usize,
    server_errors: usize,
    transport_errors: usize,
    retry_after_seen: bool,
    avg_ms: u64,
}

struct FindingSpec<'a> {
    severity: &'a str,
    category: &'a str,
    title: &'a str,
    impact: &'a str,
    remediation: &'a str,
    risk_score: u8,
    evidence: String,
}

macro_rules! finding_spec {
    ($severity:expr, $category:expr, $title:expr, $impact:expr, $remediation:expr, $risk_score:expr, $evidence:expr $(,)?) => {
        FindingSpec {
            severity: $severity,
            category: $category,
            title: $title,
            impact: $impact,
            remediation: $remediation,
            risk_score: $risk_score,
            evidence: ($evidence).into(),
        }
    };
}

pub fn run_scan(
    selector: &SourceSelector,
    threshold: SeverityLevel,
    live_scan: bool,
    config: &Config,
) -> Result<SecurityReport> {
    let options = SecurityScanOptions {
        live_scan,
        ..SecurityScanOptions::default()
    };
    run_scan_with_options(selector, threshold, &options, config)
}

pub fn run_scan_with_options(
    selector: &SourceSelector,
    threshold: SeverityLevel,
    options: &SecurityScanOptions,
    config: &Config,
) -> Result<SecurityReport> {
    let records = resolve_records(selector)?;
    run_scan_for_records(source_name(selector), records, threshold, options, config)
}

pub fn run_scan_for_records(
    source: String,
    records: Vec<RequestRecord>,
    threshold: SeverityLevel,
    options: &SecurityScanOptions,
    config: &Config,
) -> Result<SecurityReport> {
    let mut findings = Vec::new();
    let mut checks = Vec::new();

    for record in &records {
        let resolved = resolve_record_runtime(record, options.env_profile.as_deref(), config)
            .with_context(|| format!("failed to resolve {}", record.source_label))?;
        findings.extend(scan_record(&resolved));

        if options.live_scan {
            let (live_findings, live_summary) = scan_live(&resolved, config);
            findings.extend(live_findings);
            checks.push(live_summary);
        }

        if options.active_checks_enabled() {
            let (active_findings, active_checks) = scan_active(&resolved, options, config);
            findings.extend(active_findings);
            checks.extend(active_checks);
        }
    }

    findings.retain(|finding| meets_threshold(&finding.severity, threshold));
    findings.sort_by_key(|finding| severity_rank(&finding.severity));
    findings.reverse();

    let summary = format!(
        "{} finding(s) across {} request(s); active_scan={}",
        findings.len(),
        records.len(),
        options.active_checks_enabled()
    );
    let payload = SecurityReport {
        source: source.clone(),
        generated_at: Utc::now().to_rfc3339(),
        live_scan: options.live_scan,
        active_scan: options.active_checks_enabled(),
        env_profile: options.env_profile.clone(),
        request_count: records.len(),
        findings,
        checks,
        report_id: 0,
    };
    let payload_json = serde_json::to_string_pretty(&payload)?;
    let conn = open_connection()?;
    let report_id = record_report(&conn, "security", &source, &summary, &payload_json)?;

    Ok(SecurityReport {
        report_id,
        ..payload
    })
}

pub fn render_report(report: &SecurityReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Security scan for {} [{}] report_id={} live={} active={}\n",
        report.source, report.generated_at, report.report_id, report.live_scan, report.active_scan
    ));
    if let Some(env_profile) = &report.env_profile {
        out.push_str(&format!("Environment profile: {env_profile}\n"));
    }
    if !report.checks.is_empty() {
        out.push_str("Checks:\n");
        for check in &report.checks {
            out.push_str(&format!(
                "- {} [{}] target={} attempts={} findings={} note={}\n",
                check.name, check.status, check.target, check.attempts, check.findings, check.note
            ));
        }
    }
    if report.findings.is_empty() {
        out.push_str("No findings matched the selected threshold.\n");
        return out;
    }
    for finding in &report.findings {
        out.push_str(&format!(
            "- [{}] [{}] {} :: {} (risk {})\n  Impact: {}\n  Remediation: {}\n  Evidence: {}\n",
            finding.severity,
            finding.category,
            finding.endpoint,
            finding.title,
            finding.risk_score,
            finding.impact,
            finding.remediation,
            finding.evidence
        ));
    }
    out
}

fn scan_record(record: &RequestRecord) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    let url_lower = record.url.to_ascii_lowercase();
    let joined_items = record.items.join("\n");
    let joined_headers = record
        .headers
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let auth_header = record
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| {
            find_header_in_items(&record.items, "authorization").unwrap_or_default()
        });

    if url_lower.starts_with("http://") {
        findings.push(finding(
            record,
            finding_spec!(
                "high",
                "static",
                "Transport security disabled",
                "Requests over plain HTTP can expose credentials and payloads in transit.",
                "Use HTTPS endpoints and enforce TLS for all environments.",
                82,
                &record.url,
            ),
        ));
    }

    if auth_header.is_empty()
        && !joined_items.to_ascii_lowercase().contains("authorization:")
        && !joined_items.to_ascii_lowercase().contains("token")
        && !joined_items.to_ascii_lowercase().contains("apikey")
    {
        findings.push(finding(
            record,
            finding_spec!(
                "medium",
                "static",
                "No authentication detected",
                "Unauthenticated endpoints are more exposed to unauthorized access and BOLA-style abuse.",
                "Document the intended auth model and require auth where the endpoint is not explicitly public.",
                58,
                "No auth header, bearer token, or api key found in the saved request.",
            ),
        ));
    }

    if auth_header.to_ascii_lowercase().starts_with("basic ") {
        findings.push(finding(
            record,
            finding_spec!(
                "high",
                "static",
                "Basic authentication configured",
                "Basic auth is easy to mishandle and often relies on long-lived credentials.",
                "Prefer short-lived bearer tokens or signed requests with secret rotation.",
                76,
                &auth_header,
            ),
        ));
    }

    if contains_secret(&record.url)
        || contains_secret(&joined_items)
        || contains_secret(&joined_headers)
        || contains_aws_key(&record.url)
        || contains_aws_key(&joined_items)
        || contains_jwt(&record.url)
        || contains_jwt(&joined_items)
    {
        findings.push(finding(
            record,
            finding_spec!(
                "critical",
                "static",
                "Potential secret exposure",
                "Hardcoded credentials or tokens can be extracted from local request definitions and copied into logs or exports.",
                "Move credentials to the local secret store or environment profiles and scrub them from saved collections.",
                95,
                "Detected token-, key-, or credential-like content in URL, headers, or request items.",
            ),
        ));
    }

    if has_sensitive_query(&record.url) {
        findings.push(finding(
            record,
            finding_spec!(
                "high",
                "static",
                "Sensitive data appears in query parameters",
                "Secrets in URLs leak into logs, analytics systems, browser history, and proxies.",
                "Send secrets in headers or request bodies instead of query parameters.",
                84,
                &record.url,
            ),
        ));
    }

    if has_object_identifier(&record.url) {
        findings.push(finding(
            record,
            finding_spec!(
                "medium",
                "static",
                "Potential object-level authorization risk",
                "Endpoints with path identifiers commonly require strong authorization checks to prevent BOLA-style access.",
                "Verify resource ownership and authorization checks for every identifier-based lookup.",
                61,
                &record.url,
            ),
        ));
    }

    if joined_items.to_ascii_lowercase().contains("callback")
        || joined_items.to_ascii_lowercase().contains("webhook")
        || joined_items.to_ascii_lowercase().contains("redirect_uri")
    {
        findings.push(finding(
            record,
            finding_spec!(
                "low",
                "static",
                "Potential SSRF-sensitive input",
                "Callback and redirect-style fields can be abused if upstream services fetch attacker-controlled URLs.",
                "Validate destinations against an allowlist and reject internal/private network targets.",
                38,
                "callback/webhook/redirect style field found in request items.",
            ),
        ));
    }

    findings
}

fn scan_live(
    record: &RequestRecord,
    config: &Config,
) -> (Vec<SecurityFinding>, SecurityCheckSummary) {
    let method = record.method.trim().to_ascii_uppercase();
    if method != "GET" && method != "HEAD" {
        return (
            Vec::new(),
            SecurityCheckSummary {
                name: "Live Header Audit".to_string(),
                target: record.source_label.clone(),
                status: "skipped".to_string(),
                attempts: 0,
                findings: 0,
                note: "Live header checks only run for GET/HEAD requests.".to_string(),
            },
        );
    }
    if !record.url.starts_with("http://") && !record.url.starts_with("https://") {
        return (
            Vec::new(),
            SecurityCheckSummary {
                name: "Live Header Audit".to_string(),
                target: record.source_label.clone(),
                status: "skipped".to_string(),
                attempts: 0,
                findings: 0,
                note: "URL is not an absolute HTTP(S) endpoint.".to_string(),
            },
        );
    }

    let Ok((_trace, response, _elapsed_ms)) = execute_record(record, config) else {
        return (
            vec![finding(
                record,
                finding_spec!(
                    "low",
                    "live",
                    "Live scan skipped",
                    "The response could not be fetched, so header-level checks were not completed.",
                    "Re-run the security scan when the endpoint is reachable from this machine.",
                    24,
                    "request execution failed during live scan",
                ),
            )],
            SecurityCheckSummary {
                name: "Live Header Audit".to_string(),
                target: record.source_label.clone(),
                status: "error".to_string(),
                attempts: 1,
                findings: 1,
                note: "Baseline live request failed.".to_string(),
            },
        );
    };

    let mut findings = Vec::new();
    let mut headers = HashMap::new();
    for (key, value) in &response.headers {
        headers.insert(key.to_ascii_lowercase(), value.clone());
    }

    if record.url.starts_with("https://") && !headers.contains_key("strict-transport-security") {
        findings.push(finding(
            record,
            finding_spec!(
                "medium",
                "live",
                "Missing HSTS header",
                "Without HSTS, browsers may downgrade or reattempt insecure transport.",
                "Add Strict-Transport-Security with a long max-age and includeSubDomains where appropriate.",
                55,
                "strict-transport-security header not present",
            ),
        ));
    }

    for (header, title) in [
        ("content-security-policy", "Missing CSP header"),
        ("x-frame-options", "Missing X-Frame-Options header"),
        (
            "x-content-type-options",
            "Missing X-Content-Type-Options header",
        ),
    ] {
        if !headers.contains_key(header) {
            findings.push(finding(
                record,
                finding_spec!(
                    "low",
                    "live",
                    title,
                    "Missing defensive headers can weaken browser-side protection for API consoles or HTML error pages.",
                    "Return the recommended security headers consistently from the gateway or service.",
                    29,
                    header,
                ),
            ));
        }
    }

    if !headers.contains_key("x-ratelimit-limit")
        && !headers.contains_key("ratelimit-limit")
        && !headers.contains_key("retry-after")
    {
        findings.push(finding(
            record,
            finding_spec!(
                "low",
                "live",
                "No rate-limiting indicators observed",
                "Missing rate-limit headers can make client-side backoff and abuse monitoring harder.",
                "Expose standard rate-limit headers or document the throttling model for this API.",
                31,
                "x-ratelimit-limit/ratelimit-limit/retry-after not present",
            ),
        ));
    }

    if let Ok(json) = serde_json::from_slice::<Value>(&response.body) {
        let mut sensitive_paths = Vec::new();
        collect_sensitive_json_paths("", &json, &mut sensitive_paths);
        if !sensitive_paths.is_empty() {
            findings.push(finding(
                record,
                finding_spec!(
                    "high",
                    "live",
                    "Potential excessive data exposure",
                    "The live response contained sensitive-looking fields that may not belong in routine client payloads.",
                    "Review response filtering, serializer policies, and least-data principles for this endpoint.",
                    79,
                    sensitive_paths.join(", "),
                ),
            ));
        }
    }

    let findings_len = findings.len();
    (
        findings,
        SecurityCheckSummary {
            name: "Live Header Audit".to_string(),
            target: record.source_label.clone(),
            status: "completed".to_string(),
            attempts: 1,
            findings: findings_len,
            note: format!("Observed status {}", response.status_code),
        },
    )
}

fn scan_active(
    record: &RequestRecord,
    options: &SecurityScanOptions,
    config: &Config,
) -> (Vec<SecurityFinding>, Vec<SecurityCheckSummary>) {
    let parsed_items = match parse_request_items(&record.items) {
        Ok(items) => items,
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "active",
                        "Active scan skipped",
                        "The stored request items could not be parsed into a mutable active scan payload.",
                        "Repair malformed request items and retry the active scan.",
                        22,
                        format!("item parse failed: {err}"),
                    ),
                )],
                vec![SecurityCheckSummary {
                    name: "Active Probe Preparation".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 0,
                    findings: 1,
                    note: err.to_string(),
                }],
            );
        }
    };

    let baseline = match observe_request(record, parsed_items.clone(), config) {
        Ok(observed) => observed,
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "active",
                        "Active scan skipped",
                        "The baseline request failed, so mutation-based probes were not executed.",
                        "Make sure the request succeeds normally before enabling active attack probes.",
                        25,
                        err.to_string(),
                    ),
                )],
                vec![SecurityCheckSummary {
                    name: "Active Baseline".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 1,
                    findings: 1,
                    note: "Baseline request failed.".to_string(),
                }],
            );
        }
    };

    let mut findings = Vec::new();
    let mut checks = vec![SecurityCheckSummary {
        name: "Active Baseline".to_string(),
        target: record.source_label.clone(),
        status: "completed".to_string(),
        attempts: 1,
        findings: 0,
        note: format!(
            "Baseline response {} {} in {}ms",
            baseline.status, baseline.reason, baseline.elapsed_ms
        ),
    }];

    if options.include_sqli {
        let (scan_findings, summary) = run_sqli_probes(record, &parsed_items, &baseline, config);
        findings.extend(scan_findings);
        checks.push(summary);
    }

    if options.include_xss {
        let (scan_findings, summary) = run_xss_probes(record, &parsed_items, &baseline, config);
        findings.extend(scan_findings);
        checks.push(summary);
    }

    if options.include_bola {
        let (scan_findings, summary) = run_bola_probe(record, options, config);
        findings.extend(scan_findings);
        checks.push(summary);
    }

    if options.include_rate_limit {
        let (scan_findings, summary) = run_rate_limit_probe(record, &parsed_items, options, config);
        findings.extend(scan_findings);
        checks.push(summary);
    }

    (findings, checks)
}

fn run_sqli_probes(
    record: &RequestRecord,
    parsed_items: &[RequestItem],
    baseline: &ObservedResponse,
    config: &Config,
) -> (Vec<SecurityFinding>, SecurityCheckSummary) {
    let payloads = ["' OR '1'='1", "\" OR 1=1 --", "') OR ('1'='1"];
    let mut findings = Vec::new();
    let mut attempts = 0u32;

    for target in injection_targets(parsed_items) {
        for payload in payloads {
            attempts += 1;
            let mutated_items = mutate_items(parsed_items, target.index, payload, target.kind);
            let Ok(observed) = observe_request(record, mutated_items, config) else {
                continue;
            };
            if let Some(finding) = analyze_sqli(record, baseline, &observed, &target.label, payload)
            {
                findings.push(finding);
                break;
            }
        }
    }

    let status = if attempts == 0 {
        "skipped"
    } else {
        "completed"
    };
    let note = if attempts == 0 {
        "No query or body fields were available for SQL injection probes.".to_string()
    } else {
        format!("Compared {} SQLi mutations against the baseline.", attempts)
    };
    (
        findings.clone(),
        SecurityCheckSummary {
            name: "SQL Injection Fuzzing".to_string(),
            target: record.source_label.clone(),
            status: status.to_string(),
            attempts,
            findings: findings.len(),
            note,
        },
    )
}

fn run_xss_probes(
    record: &RequestRecord,
    parsed_items: &[RequestItem],
    baseline: &ObservedResponse,
    config: &Config,
) -> (Vec<SecurityFinding>, SecurityCheckSummary) {
    let payloads = ["<zapreq-xss-probe>", "\"><script>zapreqXssProbe()</script>"];
    let mut findings = Vec::new();
    let mut attempts = 0u32;

    for target in injection_targets(parsed_items) {
        for payload in payloads {
            attempts += 1;
            let mutated_items = mutate_items(parsed_items, target.index, payload, target.kind);
            let Ok(observed) = observe_request(record, mutated_items, config) else {
                continue;
            };
            if let Some(finding) = analyze_xss(record, baseline, &observed, &target.label, payload)
            {
                findings.push(finding);
                break;
            }
        }
    }

    let status = if attempts == 0 {
        "skipped"
    } else {
        "completed"
    };
    let note = if attempts == 0 {
        "No query or body fields were available for XSS probes.".to_string()
    } else {
        format!("Compared {} XSS mutations against the baseline.", attempts)
    };
    (
        findings.clone(),
        SecurityCheckSummary {
            name: "Cross-Site Scripting Fuzzing".to_string(),
            target: record.source_label.clone(),
            status: status.to_string(),
            attempts,
            findings: findings.len(),
            note,
        },
    )
}

fn run_bola_probe(
    record: &RequestRecord,
    options: &SecurityScanOptions,
    config: &Config,
) -> (Vec<SecurityFinding>, SecurityCheckSummary) {
    let Some(profile_a) = options.bola_session_a_profile.as_deref() else {
        return (
            Vec::new(),
            SecurityCheckSummary {
                name: "BOLA Probe".to_string(),
                target: record.source_label.clone(),
                status: "skipped".to_string(),
                attempts: 0,
                findings: 0,
                note: "Session A environment profile was not configured.".to_string(),
            },
        );
    };
    let Some(profile_b) = options.bola_session_b_profile.as_deref() else {
        return (
            Vec::new(),
            SecurityCheckSummary {
                name: "BOLA Probe".to_string(),
                target: record.source_label.clone(),
                status: "skipped".to_string(),
                attempts: 0,
                findings: 0,
                note: "Session B environment profile was not configured.".to_string(),
            },
        );
    };
    if !has_object_identifier(&record.url) {
        return (
            Vec::new(),
            SecurityCheckSummary {
                name: "BOLA Probe".to_string(),
                target: record.source_label.clone(),
                status: "skipped".to_string(),
                attempts: 0,
                findings: 0,
                note: "Request path does not look identifier-based.".to_string(),
            },
        );
    }

    let resolved_a = match resolve_record_runtime(record, Some(profile_a), config) {
        Ok(record) => record,
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "bola",
                        "BOLA test skipped",
                        "Session A could not be resolved for an authorization comparison.",
                        "Verify the environment profile and retry the BOLA test.",
                        21,
                        err.to_string(),
                    ),
                )],
                SecurityCheckSummary {
                    name: "BOLA Probe".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 0,
                    findings: 1,
                    note: format!("Failed to resolve Session A profile '{profile_a}'."),
                },
            );
        }
    };
    let resolved_b = match resolve_record_runtime(record, Some(profile_b), config) {
        Ok(record) => record,
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "bola",
                        "BOLA test skipped",
                        "Session B could not be resolved for an authorization comparison.",
                        "Verify the environment profile and retry the BOLA test.",
                        21,
                        err.to_string(),
                    ),
                )],
                SecurityCheckSummary {
                    name: "BOLA Probe".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 0,
                    findings: 1,
                    note: format!("Failed to resolve Session B profile '{profile_b}'."),
                },
            );
        }
    };

    let session_a = match execute_record(&resolved_a, config) {
        Ok((_trace, response, elapsed_ms)) => ObservedResponse::from_response(response, elapsed_ms),
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "bola",
                        "BOLA test skipped",
                        "The Session A baseline request did not succeed, so ownership comparison could not start.",
                        "Verify the Session A profile can access the resource before running BOLA tests.",
                        24,
                        err.to_string(),
                    ),
                )],
                SecurityCheckSummary {
                    name: "BOLA Probe".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 1,
                    findings: 1,
                    note: "Session A baseline request failed.".to_string(),
                },
            );
        }
    };
    let session_b = match execute_record(&resolved_b, config) {
        Ok((_trace, response, elapsed_ms)) => ObservedResponse::from_response(response, elapsed_ms),
        Err(err) => {
            return (
                vec![finding(
                    record,
                    finding_spec!(
                        "low",
                        "bola",
                        "BOLA test skipped",
                        "The Session B baseline request did not complete, so the authorization check is inconclusive.",
                        "Verify the Session B profile and endpoint reachability before retrying.",
                        24,
                        err.to_string(),
                    ),
                )],
                SecurityCheckSummary {
                    name: "BOLA Probe".to_string(),
                    target: record.source_label.clone(),
                    status: "error".to_string(),
                    attempts: 2,
                    findings: 1,
                    note: "Session B comparison request failed.".to_string(),
                },
            );
        }
    };

    let mut findings = Vec::new();
    if session_a.status < 400
        && session_b.status < 400
        && body_similarity(&session_a.body, &session_b.body) >= 0.8
    {
        findings.push(finding(
            record,
            finding_spec!(
                "critical",
                "bola",
                "Possible broken object level authorization",
                "Session B was able to access a path-scoped resource that was also readable by Session A.",
                "Enforce per-object ownership checks for every path identifier and verify that cross-tenant requests fail closed.",
                92,
                format!(
                    "SessionA(profile={profile_a}) -> {} in {}ms; SessionB(profile={profile_b}) -> {} in {}ms; body_similarity={:.2}",
                    session_a.status,
                    session_a.elapsed_ms,
                    session_b.status,
                    session_b.elapsed_ms,
                    body_similarity(&session_a.body, &session_b.body)
                ),
            ),
        ));
    }

    (
        findings.clone(),
        SecurityCheckSummary {
            name: "BOLA Probe".to_string(),
            target: record.source_label.clone(),
            status: "completed".to_string(),
            attempts: 2,
            findings: findings.len(),
            note: format!(
                "Session A status={}, Session B status={}",
                session_a.status, session_b.status
            ),
        },
    )
}

fn run_rate_limit_probe(
    record: &RequestRecord,
    parsed_items: &[RequestItem],
    options: &SecurityScanOptions,
    config: &Config,
) -> (Vec<SecurityFinding>, SecurityCheckSummary) {
    let total_requests = options.rate_limit_requests.max(1);
    let concurrency = options.rate_limit_concurrency.max(1);
    let collected =
        collect_rate_limit_observations(record, parsed_items, total_requests, concurrency, config);
    let metrics = summarize_rate_limit_observations(&collected);

    let mut findings = Vec::new();
    if metrics.server_errors > metrics.throttled {
        findings.push(finding(
            record,
            finding_spec!(
                "high",
                "rate-limit",
                "Burst load caused server errors before graceful throttling",
                "The endpoint emitted 5xx responses under concurrent load instead of cleanly signaling throttling or backoff.",
                "Introduce gateway or application rate limits that shed load with explicit 429/Retry-After responses before the service destabilizes.",
                81,
                format!(
                "requests={} concurrency={} throttled={} server_errors={} transport_errors={} avg_ms={}",
                total_requests, concurrency, metrics.throttled, metrics.server_errors, metrics.transport_errors, metrics.avg_ms
                ),
            ),
        ));
    } else if metrics.throttled > 0 && !metrics.retry_after_seen {
        findings.push(finding(
            record,
            finding_spec!(
                "medium",
                "rate-limit",
                "Throttling observed without Retry-After guidance",
                "Clients received throttled responses but no concrete backoff instruction, which can amplify retry storms.",
                "Return Retry-After or standard RateLimit-* headers on throttled responses.",
                57,
                format!(
                "requests={} concurrency={} throttled={} avg_ms={}",
                total_requests, concurrency, metrics.throttled, metrics.avg_ms
                ),
            ),
        ));
    } else if metrics.throttled == 0 && metrics.transport_errors == 0 {
        findings.push(finding(
            record,
            finding_spec!(
                "low",
                "rate-limit",
                "No throttling signal observed during burst test",
                "The configured burst completed without visible throttling, so abuse resistance remains unverified at this load level.",
                "Verify gateway and application rate limits at a load profile that matches production expectations.",
                34,
                format!(
                "requests={} concurrency={} server_errors={} avg_ms={}",
                total_requests, concurrency, metrics.server_errors, metrics.avg_ms
                ),
            ),
        ));
    }

    (
        findings.clone(),
        SecurityCheckSummary {
            name: "Rate-Limit Burst Test".to_string(),
            target: record.source_label.clone(),
            status: "completed".to_string(),
            attempts: total_requests,
            findings: findings.len(),
            note: format!(
                "throttled={} server_errors={} transport_errors={} avg_ms={}",
                metrics.throttled, metrics.server_errors, metrics.transport_errors, metrics.avg_ms
            ),
        },
    )
}

fn collect_rate_limit_observations(
    record: &RequestRecord,
    parsed_items: &[RequestItem],
    total_requests: u32,
    concurrency: u32,
    config: &Config,
) -> Vec<RateLimitObservation> {
    let counter = Arc::new(AtomicUsize::new(0));
    let observations: Arc<Mutex<Vec<RateLimitObservation>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::with_capacity(concurrency as usize);

    for _ in 0..concurrency {
        handles.push(spawn_rate_limit_worker(
            record.clone(),
            parsed_items.to_vec(),
            total_requests,
            config.clone(),
            Arc::clone(&counter),
            Arc::clone(&observations),
        ));
    }

    for handle in handles {
        let _ = handle.join();
    }

    let collected = observations
        .lock()
        .expect("rate-limit observation lock poisoned")
        .clone();
    collected
}

fn spawn_rate_limit_worker(
    record: RequestRecord,
    items: Vec<RequestItem>,
    total_requests: u32,
    config: Config,
    counter: Arc<AtomicUsize>,
    observations: Arc<Mutex<Vec<RateLimitObservation>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        let idx = counter.fetch_add(1, Ordering::SeqCst);
        if idx >= total_requests as usize {
            break;
        }
        let entry = observe_rate_limit_request(&record, &items, &config);
        observations
            .lock()
            .expect("rate-limit observation lock poisoned")
            .push(entry);
    })
}

fn observe_rate_limit_request(
    record: &RequestRecord,
    items: &[RequestItem],
    config: &Config,
) -> RateLimitObservation {
    let started = Instant::now();
    match observe_request(record, items.to_vec(), config) {
        Ok(observed) => RateLimitObservation {
            status: Some(observed.status),
            headers: observed.headers,
            elapsed_ms: started.elapsed().as_millis() as u64,
            error: None,
        },
        Err(err) => RateLimitObservation {
            status: None,
            headers: Vec::new(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            error: Some(err.to_string()),
        },
    }
}

fn summarize_rate_limit_observations(
    observations: &[RateLimitObservation],
) -> RateLimitProbeMetrics {
    RateLimitProbeMetrics {
        throttled: observations
            .iter()
            .filter(|observation| matches!(observation.status, Some(429) | Some(503)))
            .count(),
        server_errors: observations
            .iter()
            .filter(|observation| matches!(observation.status, Some(code) if code >= 500))
            .count(),
        transport_errors: observations
            .iter()
            .filter(|observation| observation.error.is_some())
            .count(),
        retry_after_seen: observations
            .iter()
            .filter(|observation| matches!(observation.status, Some(429) | Some(503)))
            .any(|observation| has_header(&observation.headers, "retry-after")),
        avg_ms: average_observation_latency(observations),
    }
}

fn average_observation_latency(observations: &[RateLimitObservation]) -> u64 {
    if observations.is_empty() {
        0
    } else {
        observations
            .iter()
            .map(|observation| observation.elapsed_ms)
            .sum::<u64>()
            / observations.len() as u64
    }
}

#[derive(Clone, Copy)]
enum InjectionKind {
    QueryParam,
    DataString,
    DataJson,
}

#[derive(Clone)]
struct InjectionTarget {
    index: usize,
    kind: InjectionKind,
    label: String,
}

fn injection_targets(items: &[RequestItem]) -> Vec<InjectionTarget> {
    let mut targets = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            RequestItem::QueryParam { key, .. } => targets.push(InjectionTarget {
                index,
                kind: InjectionKind::QueryParam,
                label: format!("query:{key}"),
            }),
            RequestItem::DataString { key, .. } => targets.push(InjectionTarget {
                index,
                kind: InjectionKind::DataString,
                label: format!("body:{key}"),
            }),
            RequestItem::DataJson { key, .. } => targets.push(InjectionTarget {
                index,
                kind: InjectionKind::DataJson,
                label: format!("json:{key}"),
            }),
            _ => {}
        }
    }
    targets
}

fn mutate_items(
    items: &[RequestItem],
    index: usize,
    payload: &str,
    kind: InjectionKind,
) -> Vec<RequestItem> {
    let mut out = items.to_vec();
    if let Some(item) = out.get_mut(index) {
        match (kind, item) {
            (InjectionKind::QueryParam, RequestItem::QueryParam { value, .. })
            | (InjectionKind::DataString, RequestItem::DataString { value, .. }) => {
                let base = value.clone();
                *value = merge_probe_value(&base, payload);
            }
            (InjectionKind::DataJson, RequestItem::DataJson { value, .. }) => {
                let base = match &*value {
                    Value::String(raw) => raw.clone(),
                    other => other.to_string(),
                };
                *value = Value::String(merge_probe_value(&base, payload));
            }
            _ => {}
        }
    }
    out
}

fn observe_request(
    record: &RequestRecord,
    items: Vec<RequestItem>,
    config: &Config,
) -> Result<ObservedResponse> {
    let cli = build_cli_for_record(record, config)?;
    let headers =
        crate::headers::build_headers_from_cli(&cli, &items, &std::collections::HashMap::new())?;
    let spec = RequestSpec {
        method: record.method.clone(),
        url: record.url.clone(),
        items,
        headers,
    };
    let started = Instant::now();
    let (_trace, response) = RequestEngine::new()
        .send(&cli, &spec, None)
        .with_context(|| format!("request execution failed for '{}'", record.name))?;
    Ok(ObservedResponse::from_response(
        response,
        started.elapsed().as_millis() as u64,
    ))
}

impl ObservedResponse {
    fn from_response(response: ResponseData, elapsed_ms: u64) -> Self {
        Self {
            status: response.status_code,
            reason: response.reason,
            content_type: response.content_type,
            headers: response.headers,
            body: response.body,
            elapsed_ms,
        }
    }
}

fn analyze_sqli(
    record: &RequestRecord,
    baseline: &ObservedResponse,
    observed: &ObservedResponse,
    target: &str,
    payload: &str,
) -> Option<SecurityFinding> {
    let baseline_text = String::from_utf8_lossy(&baseline.body);
    let observed_text = String::from_utf8_lossy(&observed.body);
    let baseline_error = sql_error_excerpt(&baseline_text).is_some();
    let observed_error = sql_error_excerpt(&observed_text);
    let suspicious = (baseline.status < 500 && observed.status >= 500)
        || (!baseline_error && observed_error.is_some())
        || (baseline.status == observed.status
            && response_length_delta(baseline.body.len(), observed.body.len()) >= 0.45
            && observed.status >= 500);

    if !suspicious {
        return None;
    }

    Some(finding(
        record,
        finding_spec!(
            "high",
            "sqli",
            "Potential SQL injection behavior observed",
            "A crafted probe changed the server-side behavior in a way that resembles unsafe query handling.",
            "Use parameterized queries, central input validation, and defensive error handling to prevent SQL construction from user input.",
            87,
            format!(
            "{target} payload={payload:?} baseline={} mutated={} baseline_len={} mutated_len={} sql_error={}",
            baseline.status,
            observed.status,
            baseline.body.len(),
            observed.body.len(),
            observed_error.unwrap_or("none")
            ),
        ),
    ))
}

fn analyze_xss(
    record: &RequestRecord,
    baseline: &ObservedResponse,
    observed: &ObservedResponse,
    target: &str,
    payload: &str,
) -> Option<SecurityFinding> {
    let baseline_text = String::from_utf8_lossy(&baseline.body);
    let observed_text = String::from_utf8_lossy(&observed.body);
    let reflected = observed_text.contains(payload) && !baseline_text.contains(payload);
    let html_like = observed
        .content_type
        .as_deref()
        .map(|value| value.to_ascii_lowercase().contains("html"))
        .unwrap_or(false)
        || observed_text.contains("<html")
        || observed_text.contains("<body");

    if !reflected {
        return None;
    }

    Some(finding(
        record,
        finding_spec!(
            if html_like { "high" } else { "medium" },
            "xss",
            "Potential reflected cross-site scripting behavior observed",
            "A tag-bearing payload was reflected without escaping, which can become executable in HTML-capable clients or admin tooling.",
            "Encode output by context, reject dangerous markup in untrusted fields, and render user input as text rather than HTML.",
            if html_like { 83 } else { 69 },
            format!(
            "{target} payload={payload:?} baseline_status={} mutated_status={} content_type={}",
            baseline.status,
            observed.status,
            observed.content_type.clone().unwrap_or_else(|| "unknown".to_string())
            ),
        ),
    ))
}

fn resolve_record_runtime(
    record: &RequestRecord,
    env_profile: Option<&str>,
    _config: &Config,
) -> Result<RequestRecord> {
    let mut resolved = CliResolved {
        url: record.url.clone(),
        request_items: combined_record_items(record),
        profile_headers: HashMap::new(),
        variables: HashMap::new(),
    };

    if let Some(profile_name) = env_profile {
        let profile = load_profile(profile_name)
            .with_context(|| format!("failed to load env profile: {profile_name}"))?;
        apply_profile(&profile, &mut resolved);
    }

    let resolved_url = substitute_placeholders(&resolved.url, &resolved.variables);
    let mut resolved_items = resolved
        .request_items
        .iter()
        .map(|raw| substitute_item_value(raw, &resolved.variables))
        .collect::<Vec<_>>();
    for (key, value) in &resolved.profile_headers {
        resolved_items.push(format!(
            "{}:{}",
            substitute_placeholders(key, &resolved.variables),
            substitute_placeholders(value, &resolved.variables)
        ));
    }

    validate_unresolved_values(&resolved_url, &resolved_items)?;
    let url = normalize_url(&resolved_url, "https")
        .with_context(|| format!("failed to normalize URL '{}'", resolved_url))?;

    Ok(RequestRecord {
        name: record.name.clone(),
        method: record.method.clone(),
        url,
        items: resolved_items,
        headers: HashMap::new(),
        source_label: record.source_label.clone(),
    })
}

fn combined_record_items(record: &RequestRecord) -> Vec<String> {
    let mut items = record.items.clone();
    for (key, value) in &record.headers {
        items.push(format!("{key}:{value}"));
    }
    items
}

fn validate_unresolved_values(resolved_url: &str, resolved_items: &[String]) -> Result<()> {
    let unresolved = unresolved_placeholders(
        std::iter::once(resolved_url)
            .chain(resolved_items.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .as_slice(),
    );
    if unresolved.is_empty() {
        return Ok(());
    }

    Err(anyhow!(
        "unresolved variables: {} (set them in the selected environment profile)",
        unresolved.join(", ")
    ))
}

fn unresolved_placeholders(values: &[&str]) -> Vec<String> {
    let re = Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    let mut output = Vec::new();
    for value in values {
        for caps in re.captures_iter(value) {
            let key = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|token| token.as_str())
                .unwrap_or_default();
            if !output.iter().any(|existing| existing == key) {
                output.push(key.to_string());
            }
        }
    }
    output
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let key = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|value| value.as_str())
            .unwrap_or_default();
        vars.get(key)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

fn substitute_item_value(raw: &str, vars: &HashMap<String, String>) -> String {
    let token = raw.trim();
    if let Some((key, value)) = token.split_once(":=@") {
        return format!("{key}:=@{}", substitute_placeholders(value, vars));
    }
    if let Some((key, value)) = token.split_once(":=") {
        return format!("{key}:={}", substitute_placeholders(value, vars));
    }
    if let Some((key, value)) = token.split_once("==") {
        return format!("{key}=={}", substitute_placeholders(value, vars));
    }
    if let Some((key, value)) = token.split_once(':') {
        return format!("{key}:{}", substitute_placeholders(value, vars));
    }
    if let Some((key, value)) = token.split_once("=@") {
        return format!("{key}=@{}", substitute_placeholders(value, vars));
    }
    if let Some((key, value)) = token.split_once('=') {
        if !(token.contains('@') && token.contains(";type=")) {
            return format!("{key}={}", substitute_placeholders(value, vars));
        }
    }
    if let Some((key, value)) = token.split_once('@') {
        if let Some((path, content_type)) = value.split_once(";type=") {
            return format!(
                "{key}@{};type={}",
                substitute_placeholders(path, vars),
                substitute_placeholders(content_type, vars)
            );
        }
        return format!("{key}@{}", substitute_placeholders(value, vars));
    }
    substitute_placeholders(token, vars)
}

fn finding(record: &RequestRecord, spec: FindingSpec<'_>) -> SecurityFinding {
    SecurityFinding {
        endpoint: record.source_label.clone(),
        severity: spec.severity.to_string(),
        category: spec.category.to_string(),
        title: spec.title.to_string(),
        impact: spec.impact.to_string(),
        remediation: spec.remediation.to_string(),
        risk_score: spec.risk_score,
        evidence: spec.evidence,
        method: Some(record.method.clone()),
        url: Some(record.url.clone()),
    }
}

fn source_name(selector: &SourceSelector) -> String {
    if let Some(alias) = selector.alias.as_deref() {
        return format!("alias:{alias}");
    }
    if let Some(workspace) = selector.workspace.as_deref() {
        if let Some(request) = selector.request.as_deref() {
            return format!("request:{workspace}/{request}");
        }
        return format!("workspace:{workspace}");
    }
    selector
        .file
        .as_deref()
        .map(|path| format!("file:{path}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn meets_threshold(severity: &str, threshold: SeverityLevel) -> bool {
    severity_rank(severity) >= threshold_rank(threshold)
}

fn threshold_rank(level: SeverityLevel) -> u8 {
    match level {
        SeverityLevel::Low => 1,
        SeverityLevel::Medium => 2,
        SeverityLevel::High => 3,
        SeverityLevel::Critical => 4,
    }
}

fn severity_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

fn merge_probe_value(original: &str, payload: &str) -> String {
    if original.trim().is_empty() {
        payload.to_string()
    } else {
        format!("{original}{payload}")
    }
}

fn response_length_delta(a: usize, b: usize) -> f64 {
    let largest = a.max(b);
    if largest == 0 {
        return 0.0;
    }
    let smallest = a.min(b);
    (largest - smallest) as f64 / largest as f64
}

fn body_similarity(a: &[u8], b: &[u8]) -> f64 {
    let left = normalize_body_for_similarity(a);
    let right = normalize_body_for_similarity(b);
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left == right {
        return 1.0;
    }
    let common = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count();
    common as f64 / left.len().max(right.len()) as f64
}

fn normalize_body_for_similarity(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
}

fn has_header(headers: &[(String, String)], target: &str) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case(target))
}

fn find_header_in_items(items: &[String], target: &str) -> Option<String> {
    for item in items {
        let trimmed = item.trim();
        if let Some((key, value)) = trimmed.split_once(':') {
            if key.trim().eq_ignore_ascii_case(target) {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn sql_error_excerpt(body: &str) -> Option<&'static str> {
    let lower = body.to_ascii_lowercase();
    [
        "sql syntax",
        "unclosed quotation mark",
        "sqlite error",
        "postgresql",
        "mysql",
        "odbc",
        "database error",
        "ora-",
        "syntax error at or near",
    ]
    .into_iter()
    .find(|needle| lower.contains(needle))
}

fn contains_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "password=",
        "secret=",
        "token=",
        "apikey=",
        "api_key=",
        "client_secret",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn contains_aws_key(value: &str) -> bool {
    Regex::new(r"AKIA[0-9A-Z]{16}")
        .expect("regex")
        .is_match(value)
}

fn contains_jwt(value: &str) -> bool {
    Regex::new(r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9._-]+\.[A-Za-z0-9._-]+")
        .expect("regex")
        .is_match(value)
}

fn has_sensitive_query(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    [
        "access_token=",
        "token=",
        "password=",
        "apikey=",
        "api_key=",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn has_object_identifier(url: &str) -> bool {
    url.contains('{')
        || url.contains("}/")
        || Regex::new(r"/\d+([/?#]|$)").expect("regex").is_match(url)
}

fn collect_sensitive_json_paths(path: &str, value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                let lowered = key.to_ascii_lowercase();
                if matches!(
                    lowered.as_str(),
                    "password"
                        | "secret"
                        | "token"
                        | "access_token"
                        | "refresh_token"
                        | "api_key"
                        | "apikey"
                ) {
                    output.push(next.clone());
                }
                collect_sensitive_json_paths(&next, child, output);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                collect_sensitive_json_paths(&next, child, output);
            }
        }
        _ => {}
    }
}

fn default_true() -> bool {
    true
}

fn default_rate_limit_requests() -> u32 {
    12
}

fn default_rate_limit_concurrency() -> u32 {
    4
}

#[cfg(test)]
mod tests {
    use super::{
        average_observation_latency, body_similarity, contains_aws_key, contains_jwt,
        has_object_identifier, has_sensitive_query, response_length_delta, sql_error_excerpt,
        summarize_rate_limit_observations, RateLimitObservation,
    };

    #[test]
    fn aws_key_detection_works() {
        assert!(contains_aws_key("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn jwt_detection_works() {
        assert!(contains_jwt(
            "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.signature"
        ));
    }

    #[test]
    fn sensitive_query_detection_works() {
        assert!(has_sensitive_query(
            "https://example.com?a=1&access_token=secret"
        ));
    }

    #[test]
    fn sql_error_detection_works() {
        assert_eq!(
            sql_error_excerpt("PG::SyntaxError: syntax error at or near \"'\""),
            Some("syntax error at or near")
        );
    }

    #[test]
    fn object_identifier_detection_works() {
        assert!(has_object_identifier("https://example.com/api/users/42"));
        assert!(has_object_identifier("https://example.com/api/users/{id}"));
    }

    #[test]
    fn response_deltas_are_ratio_based() {
        assert!(response_length_delta(100, 190) > 0.4);
    }

    #[test]
    fn body_similarity_detects_identical_payloads() {
        assert_eq!(body_similarity(br#"{"id":1}"#, br#"{"id":1}"#), 1.0);
    }

    #[test]
    fn rate_limit_summary_tracks_throttling_retry_after_and_average_latency() {
        let observations = vec![
            RateLimitObservation {
                status: Some(429),
                headers: vec![("Retry-After".to_string(), "30".to_string())],
                elapsed_ms: 90,
                error: None,
            },
            RateLimitObservation {
                status: Some(503),
                headers: Vec::new(),
                elapsed_ms: 150,
                error: None,
            },
            RateLimitObservation {
                status: Some(500),
                headers: Vec::new(),
                elapsed_ms: 210,
                error: Some("boom".to_string()),
            },
        ];

        let summary = summarize_rate_limit_observations(&observations);

        assert_eq!(summary.throttled, 2);
        assert_eq!(summary.server_errors, 2);
        assert_eq!(summary.transport_errors, 1);
        assert!(summary.retry_after_seen);
        assert_eq!(summary.avg_ms, average_observation_latency(&observations));
    }
}
