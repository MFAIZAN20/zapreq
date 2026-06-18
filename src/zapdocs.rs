use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::cli::{DocFormat, SourceSelector};
use crate::localdb::{open_connection, record_report, reports_dir};
use crate::sources::{resolve_records, RequestRecord};

#[derive(Clone, Debug, Serialize)]
pub struct DocsReport {
    pub source: String,
    pub format: String,
    pub output_path: String,
    pub request_count: usize,
    pub generated_at: String,
    pub report_id: i64,
}

pub fn generate_docs(
    selector: &SourceSelector,
    format: DocFormat,
    output: Option<&str>,
) -> Result<DocsReport> {
    let records = resolve_records(selector)?;
    let source = source_name(selector);
    let generated_at = Utc::now().to_rfc3339();
    let text = match format {
        DocFormat::Markdown => generate_markdown(&source, &records, &generated_at),
        DocFormat::Openapi => serde_json::to_string_pretty(&generate_openapi(&source, &records))?,
        DocFormat::Html => generate_html(&source, &records, &generated_at),
    };

    let output_path = if let Some(path) = output {
        path.to_string()
    } else {
        default_output_path(&source, format)
            .to_string_lossy()
            .into_owned()
    };

    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create docs dir: {}", parent.display()))?;
    }
    std::fs::write(&output_path, text)
        .with_context(|| format!("failed to write generated docs: {output_path}"))?;

    let summary = format!(
        "Generated {} documentation for {}",
        format_name(format),
        source
    );
    let conn = open_connection()?;
    let payload = json!({
        "source": source,
        "format": format_name(format),
        "output_path": output_path,
        "request_count": records.len(),
        "generated_at": generated_at,
    });
    let report_id = record_report(
        &conn,
        "docs",
        &summary,
        &summary,
        &serde_json::to_string_pretty(&payload)?,
    )?;

    Ok(DocsReport {
        source: payload["source"].as_str().unwrap_or_default().to_string(),
        format: payload["format"].as_str().unwrap_or_default().to_string(),
        output_path,
        request_count: records.len(),
        generated_at,
        report_id,
    })
}

pub fn render_report(report: &DocsReport) -> String {
    format!(
        "Generated {} docs for {} -> {}\nrequest_count={} report_id={}",
        report.format, report.source, report.output_path, report.request_count, report.report_id
    )
}

fn generate_markdown(source: &str, records: &[RequestRecord], generated_at: &str) -> String {
    let mut out = String::new();
    out.push_str("# ZapReq API Documentation\n\n");
    out.push_str(&format!(
        "Source: `{source}`\n\nGenerated: `{generated_at}`\n\n"
    ));
    out.push_str("## Endpoints\n\n");
    for record in records {
        out.push_str(&format!("### {}\n\n", record.name));
        out.push_str(&format!("- Method: `{}`\n", record.method));
        out.push_str(&format!("- URL: `{}`\n", record.url));
        if !record.headers.is_empty() {
            out.push_str("- Headers:\n");
            for (key, value) in &record.headers {
                out.push_str(&format!("  - `{key}: {value}`\n"));
            }
        }
        if !record.items.is_empty() {
            out.push_str("- Request items:\n");
            for item in &record.items {
                out.push_str(&format!("  - `{item}`\n"));
            }
        }
        out.push_str(&format!(
            "- Example CLI:\n\n```bash\nzapreq {} {} {}\n```\n\n",
            record.method,
            record.url,
            record.items.join(" ")
        ));
    }
    out
}

fn generate_openapi(source: &str, records: &[RequestRecord]) -> Value {
    let mut paths = serde_json::Map::new();
    for record in records {
        let path_key = openapi_path(&record.url);
        let method_key = record.method.to_ascii_lowercase();
        let entry = paths
            .entry(path_key.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(obj) = entry {
            obj.insert(
                method_key,
                json!({
                    "summary": record.name,
                    "description": format!("Imported from {}", record.source_label),
                    "parameters": parameters_from_record(record),
                    "requestBody": request_body_from_record(record),
                    "responses": {
                        "200": { "description": "Successful response" },
                        "default": { "description": "Default response" }
                    }
                }),
            );
        }
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": format!("ZapReq export for {}", source),
            "version": "1.0.0"
        },
        "paths": Value::Object(paths)
    })
}

fn generate_html(source: &str, records: &[RequestRecord], generated_at: &str) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\">");
    out.push_str("<title>ZapReq API Documentation</title>");
    out.push_str(
        "<style>body{font-family:system-ui,sans-serif;background:#f4f7fb;color:#1d2329;margin:0;padding:32px;}main{max-width:980px;margin:0 auto;}section{background:#fff;border:1px solid #d6dde6;border-radius:14px;padding:20px;margin:0 0 18px;}code,pre{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;background:#eef3f8;border-radius:8px;}pre{padding:14px;overflow:auto;}h1,h2,h3{margin-top:0}.meta{color:#65707b}</style>",
    );
    out.push_str("</head><body><main>");
    out.push_str("<section>");
    out.push_str("<h1>ZapReq API Documentation</h1>");
    out.push_str(&format!(
        "<p class=\"meta\">Source: {}<br>Generated: {}</p>",
        escape_html(source),
        escape_html(generated_at)
    ));
    out.push_str("</section>");
    for record in records {
        out.push_str("<section>");
        out.push_str(&format!("<h2>{}</h2>", escape_html(&record.name)));
        out.push_str(&format!(
            "<p><strong>Method:</strong> <code>{}</code><br><strong>URL:</strong> <code>{}</code></p>",
            escape_html(&record.method),
            escape_html(&record.url)
        ));
        if !record.items.is_empty() {
            out.push_str("<h3>Request items</h3><ul>");
            for item in &record.items {
                out.push_str(&format!("<li><code>{}</code></li>", escape_html(item)));
            }
            out.push_str("</ul>");
        }
        out.push_str("<h3>Example CLI</h3>");
        out.push_str(&format!(
            "<pre>zapreq {} {} {}</pre>",
            escape_html(&record.method),
            escape_html(&record.url),
            escape_html(&record.items.join(" "))
        ));
        out.push_str("</section>");
    }
    out.push_str("</main></body></html>");
    out
}

fn default_output_path(source: &str, format: DocFormat) -> std::path::PathBuf {
    let extension = match format {
        DocFormat::Markdown => "md",
        DocFormat::Openapi => "json",
        DocFormat::Html => "html",
    };
    let slug = source
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect::<String>();
    reports_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("docs")
        .join(format!("{slug}.{extension}"))
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

fn parameters_from_record(record: &RequestRecord) -> Vec<Value> {
    let mut params = Vec::new();
    for item in &record.items {
        if let Some((key, value)) = item.split_once("==") {
            params.push(json!({
                "name": key,
                "in": "query",
                "required": false,
                "schema": { "type": "string" },
                "example": value
            }));
        } else if let Some((key, value)) = item.split_once(':') {
            params.push(json!({
                "name": key,
                "in": "header",
                "required": false,
                "schema": { "type": "string" },
                "example": value
            }));
        }
    }
    params
}

fn request_body_from_record(record: &RequestRecord) -> Value {
    let mut props = serde_json::Map::new();
    for item in &record.items {
        if let Some((key, value)) = item.split_once(":=") {
            let parsed = serde_json::from_str::<Value>(value).unwrap_or_else(|_| json!(value));
            props.insert(key.to_string(), parsed);
        } else if let Some((key, value)) = item.split_once('=') {
            if !item.contains("==") {
                props.insert(key.to_string(), json!(value));
            }
        }
    }
    if props.is_empty() {
        Value::Null
    } else {
        json!({
            "required": false,
            "content": {
                "application/json": {
                    "example": Value::Object(props)
                }
            }
        })
    }
}

fn openapi_path(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        let path = parsed.path();
        if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        }
    } else if url.starts_with('/') {
        url.to_string()
    } else {
        format!("/{}", url.trim_start_matches('/'))
    }
}

fn format_name(format: DocFormat) -> &'static str {
    match format {
        DocFormat::Markdown => "markdown",
        DocFormat::Openapi => "openapi",
        DocFormat::Html => "html",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
