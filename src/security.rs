use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::cli::{SeverityLevel, SourceSelector};
use crate::config::Config;
use crate::localdb::{open_connection, record_report};
use crate::sources::{execute_record, resolve_records, RequestRecord};

#[derive(Clone, Debug, Serialize)]
pub struct SecurityFinding {
    pub endpoint: String,
    pub severity: String,
    pub title: String,
    pub impact: String,
    pub remediation: String,
    pub risk_score: u8,
    pub evidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SecurityReport {
    pub source: String,
    pub generated_at: String,
    pub live_scan: bool,
    pub findings: Vec<SecurityFinding>,
    pub report_id: i64,
}

pub fn run_scan(
    selector: &SourceSelector,
    threshold: SeverityLevel,
    live_scan: bool,
    config: &Config,
) -> Result<SecurityReport> {
    let records = resolve_records(selector)?;
    let source = source_name(selector);
    let mut findings = Vec::new();
    for record in &records {
        findings.extend(scan_record(record));
        if live_scan {
            findings.extend(scan_live(record, config));
        }
    }
    findings.retain(|finding| meets_threshold(&finding.severity, threshold));
    findings.sort_by_key(|finding| severity_rank(&finding.severity));
    findings.reverse();

    let summary = format!(
        "{} finding(s) across {} request(s)",
        findings.len(),
        records.len()
    );
    let payload = SecurityReport {
        source: source.clone(),
        generated_at: Utc::now().to_rfc3339(),
        live_scan,
        findings,
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
        "Security scan for {} [{}] report_id={}\n",
        report.source, report.generated_at, report.report_id
    ));
    if report.findings.is_empty() {
        out.push_str("No findings matched the selected threshold.\n");
        return out;
    }
    for finding in &report.findings {
        out.push_str(&format!(
            "- [{}] {} :: {} (risk {})\n  Impact: {}\n  Remediation: {}\n  Evidence: {}\n",
            finding.severity,
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
        .unwrap_or_default();

    if url_lower.starts_with("http://") {
        findings.push(finding(
            record,
            "high",
            "Transport security disabled",
            "Requests over plain HTTP can expose credentials and payloads in transit.",
            "Use HTTPS endpoints and enforce TLS for all environments.",
            82,
            &record.url,
        ));
    }

    if auth_header.is_empty()
        && !joined_items.to_ascii_lowercase().contains("authorization:")
        && !joined_items.to_ascii_lowercase().contains("token")
        && !joined_items.to_ascii_lowercase().contains("apikey")
    {
        findings.push(finding(
            record,
            "medium",
            "No authentication detected",
            "Unauthenticated endpoints are more exposed to unauthorized access and BOLA-style abuse.",
            "Document the intended auth model and require auth where the endpoint is not explicitly public.",
            58,
            "No auth header, bearer token, or api key found in the saved request.",
        ));
    }

    if auth_header.to_ascii_lowercase().starts_with("basic ") {
        findings.push(finding(
            record,
            "high",
            "Basic authentication configured",
            "Basic auth is easy to mishandle and often relies on long-lived credentials.",
            "Prefer short-lived bearer tokens or signed requests with secret rotation.",
            76,
            &auth_header,
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
            "critical",
            "Potential secret exposure",
            "Hardcoded credentials or tokens can be extracted from local request definitions and copied into logs or exports.",
            "Move credentials to the local secret store or environment profiles and scrub them from saved collections.",
            95,
            "Detected token-, key-, or credential-like content in URL, headers, or request items.",
        ));
    }

    if has_sensitive_query(&record.url) {
        findings.push(finding(
            record,
            "high",
            "Sensitive data appears in query parameters",
            "Secrets in URLs leak into logs, analytics systems, browser history, and proxies.",
            "Send secrets in headers or request bodies instead of query parameters.",
            84,
            &record.url,
        ));
    }

    if record.url.contains('{')
        || record.url.contains("}/")
        || Regex::new(r"/\d+(/|$)")
            .expect("regex")
            .is_match(&record.url)
    {
        findings.push(finding(
            record,
            "medium",
            "Potential object-level authorization risk",
            "Endpoints with path identifiers commonly require strong authorization checks to prevent BOLA-style access.",
            "Verify resource ownership and authorization checks for every identifier-based lookup.",
            61,
            &record.url,
        ));
    }

    if joined_items.to_ascii_lowercase().contains("callback")
        || joined_items.to_ascii_lowercase().contains("webhook")
        || joined_items.to_ascii_lowercase().contains("redirect_uri")
    {
        findings.push(finding(
            record,
            "low",
            "Potential SSRF-sensitive input",
            "Callback and redirect-style fields can be abused if upstream services fetch attacker-controlled URLs.",
            "Validate destinations against an allowlist and reject internal/private network targets.",
            38,
            "callback/webhook/redirect style field found in request items.",
        ));
    }

    findings
}

fn scan_live(record: &RequestRecord, config: &Config) -> Vec<SecurityFinding> {
    let method = record.method.trim().to_ascii_uppercase();
    if method != "GET" && method != "HEAD" {
        return Vec::new();
    }
    if !record.url.starts_with("http://") && !record.url.starts_with("https://") {
        return Vec::new();
    }

    let mut findings = Vec::new();
    let Ok((_trace, response, _elapsed_ms)) = execute_record(record, config) else {
        return vec![finding(
            record,
            "low",
            "Live scan skipped",
            "The response could not be fetched, so header-level checks were not completed.",
            "Re-run the security scan when the endpoint is reachable from this machine.",
            24,
            "request execution failed during live scan",
        )];
    };

    let mut headers = std::collections::HashMap::new();
    for (key, value) in &response.headers {
        headers.insert(key.to_ascii_lowercase(), value.clone());
    }

    if record.url.starts_with("https://") && !headers.contains_key("strict-transport-security") {
        findings.push(finding(
            record,
            "medium",
            "Missing HSTS header",
            "Without HSTS, browsers may downgrade or reattempt insecure transport.",
            "Add Strict-Transport-Security with a long max-age and includeSubDomains where appropriate.",
            55,
            "strict-transport-security header not present",
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
                "low",
                title,
                "Missing defensive headers can weaken browser-side protection for API consoles or HTML error pages.",
                "Return the recommended security headers consistently from the gateway or service.",
                29,
                header,
            ));
        }
    }

    if !headers.contains_key("x-ratelimit-limit")
        && !headers.contains_key("ratelimit-limit")
        && !headers.contains_key("retry-after")
    {
        findings.push(finding(
            record,
            "low",
            "No rate-limiting indicators observed",
            "Missing rate-limit headers can make client-side backoff and abuse monitoring harder.",
            "Expose standard rate-limit headers or document the throttling model for this API.",
            31,
            "x-ratelimit-limit/ratelimit-limit/retry-after not present",
        ));
    }

    if let Ok(json) = serde_json::from_slice::<Value>(&response.body) {
        let mut sensitive_paths = Vec::new();
        collect_sensitive_json_paths("", &json, &mut sensitive_paths);
        if !sensitive_paths.is_empty() {
            findings.push(finding(
                record,
                "high",
                "Potential excessive data exposure",
                "The live response contained sensitive-looking fields that may not belong in routine client payloads.",
                "Review response filtering, serializer policies, and least-data principles for this endpoint.",
                79,
                &sensitive_paths.join(", "),
            ));
        }
    }

    findings
}

fn finding(
    record: &RequestRecord,
    severity: &str,
    title: &str,
    impact: &str,
    remediation: &str,
    risk_score: u8,
    evidence: &str,
) -> SecurityFinding {
    SecurityFinding {
        endpoint: record.source_label.clone(),
        severity: severity.to_string(),
        title: title.to_string(),
        impact: impact.to_string(),
        remediation: remediation.to_string(),
        risk_score,
        evidence: evidence.to_string(),
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

#[cfg(test)]
mod tests {
    use super::{contains_aws_key, contains_jwt, has_sensitive_query};

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
}
