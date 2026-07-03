use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::PathBuf;

use crate::config::config_root_dir;

pub fn open_connection() -> Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create local db dir: {}", parent.display()))?;
    }
    let conn = Connection::open(&path)
        .with_context(|| format!("failed to open local sqlite db: {}", path.display()))?;
    init_schema(&conn)?;
    Ok(conn)
}

pub fn reports_dir() -> Result<PathBuf> {
    Ok(config_root_dir()?.join("reports"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(config_root_dir()?.join("zapreq.sqlite3"))
}

pub fn record_report(
    conn: &Connection,
    module: &str,
    name: &str,
    summary: &str,
    payload_json: &str,
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO reports (module, name, summary, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![module, name, summary, payload_json, now],
    )
    .context("failed to persist report")?;
    Ok(conn.last_insert_rowid())
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            module TEXT NOT NULL,
            name TEXT NOT NULL,
            summary TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS test_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            suite TEXT NOT NULL,
            name TEXT NOT NULL,
            source_label TEXT NOT NULL,
            method TEXT NOT NULL,
            url TEXT NOT NULL,
            items_json TEXT NOT NULL,
            headers_json TEXT NOT NULL,
            expect_status INTEGER,
            expect_headers_json TEXT NOT NULL,
            expect_json_json TEXT NOT NULL,
            expect_body_contains_json TEXT NOT NULL,
            max_time_ms INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(suite, name)
        );

        CREATE TABLE IF NOT EXISTS test_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            suite TEXT NOT NULL,
            case_name TEXT NOT NULL,
            passed INTEGER NOT NULL,
            status_code INTEGER NOT NULL,
            elapsed_ms INTEGER NOT NULL,
            report_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scope TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS note_revisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL,
            version INTEGER NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(note_id) REFERENCES notes(id)
        );
        ",
    )
    .context("failed to initialize sqlite schema")?;
    ensure_report_http_columns(conn)?;
    Ok(())
}

fn ensure_report_http_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(reports)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (name, sql_type) in [
        ("method", "TEXT"),
        ("url", "TEXT"),
        ("final_url", "TEXT"),
        ("status", "INTEGER"),
        ("reason", "TEXT"),
        ("elapsed_ms", "INTEGER"),
        ("size_bytes", "INTEGER"),
        ("content_type", "TEXT"),
    ] {
        if !columns.iter().any(|column| column == name) {
            conn.execute(
                &format!("ALTER TABLE reports ADD COLUMN {name} {sql_type}"),
                [],
            )?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn record_http_report(
    module: &str,
    method: &str,
    url: &str,
    final_url: &str,
    status: u16,
    reason: &str,
    elapsed_ms: u64,
    size_bytes: usize,
    content_type: Option<&str>,
    headers: &[(String, String)],
    body: &str,
) -> Result<()> {
    let conn = open_connection()?;
    let summary = format!("{method} {url} -> {status} {reason} ({elapsed_ms} ms)");
    let masked_headers: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.clone(), crate::headers::mask_header_value(k, v)))
        .collect();
    let payload = serde_json::json!({
        "method": method,
        "url": url,
        "final_url": final_url,
        "status": status,
        "reason": reason,
        "elapsed_ms": elapsed_ms,
        "size_bytes": size_bytes,
        "content_type": content_type,
        "headers": masked_headers,
        "body": body,
    });
    let payload_json = serde_json::to_string_pretty(&payload)?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO reports (
            module, name, summary, payload_json, created_at,
            method, url, final_url, status, reason, elapsed_ms, size_bytes, content_type
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            module,
            url,
            summary,
            payload_json,
            now,
            method,
            url,
            final_url,
            status,
            reason,
            elapsed_ms as i64,
            size_bytes as i64,
            content_type,
        ],
    )
    .context("failed to persist HTTP report")?;
    Ok(())
}
