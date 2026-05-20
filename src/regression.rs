use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::cli::SourceSelector;
use crate::config::Config;
use crate::localdb::open_connection;
use crate::sources::{execute_record, resolve_single_record};
use crate::testing::{evaluate_response, render_text_report, TestOptions, TestReport};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredTestCase {
    pub suite: String,
    pub name: String,
    pub source_label: String,
    pub method: String,
    pub url: String,
    pub items: Vec<String>,
    pub headers: Vec<(String, String)>,
    pub expect_status: Option<u16>,
    pub expect_headers: Vec<String>,
    pub expect_json: Vec<String>,
    pub expect_body_contains: Vec<String>,
    pub max_time_ms: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TestRunSummary {
    pub case_name: String,
    pub passed: bool,
    pub status: u16,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuiteRunReport {
    pub suite: String,
    pub generated_at: String,
    pub passed: usize,
    pub failed: usize,
    pub cases: Vec<TestRunSummary>,
}

pub fn add_test_case(
    suite: &str,
    name: &str,
    selector: &SourceSelector,
    options: &TestOptions,
) -> Result<()> {
    let record = resolve_single_record(selector)?;
    let conn = open_connection()?;
    let now = Utc::now().to_rfc3339();
    let headers = record
        .headers
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    conn.execute(
        "INSERT INTO test_cases (
            suite, name, source_label, method, url, items_json, headers_json,
            expect_status, expect_headers_json, expect_json_json, expect_body_contains_json,
            max_time_ms, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(suite, name) DO UPDATE SET
            source_label=excluded.source_label,
            method=excluded.method,
            url=excluded.url,
            items_json=excluded.items_json,
            headers_json=excluded.headers_json,
            expect_status=excluded.expect_status,
            expect_headers_json=excluded.expect_headers_json,
            expect_json_json=excluded.expect_json_json,
            expect_body_contains_json=excluded.expect_body_contains_json,
            max_time_ms=excluded.max_time_ms,
            updated_at=excluded.updated_at",
        params![
            suite,
            name,
            record.source_label,
            record.method,
            record.url,
            serde_json::to_string(&record.items)?,
            serde_json::to_string(&headers)?,
            options.expect_status.map(i64::from),
            serde_json::to_string(&options.expect_headers)?,
            serde_json::to_string(&options.expect_json)?,
            serde_json::to_string(&options.expect_body_contains)?,
            options.max_time_ms.map(|value| value as i64),
            now,
            now,
        ],
    )
    .context("failed to save test case")?;
    Ok(())
}

pub fn list_test_cases(suite: Option<&str>) -> Result<Vec<StoredTestCase>> {
    let conn = open_connection()?;
    let mut out = Vec::new();
    if let Some(suite) = suite {
        let mut stmt = conn.prepare(
            "SELECT suite, name, source_label, method, url, items_json, headers_json,
                    expect_status, expect_headers_json, expect_json_json,
                    expect_body_contains_json, max_time_ms, created_at, updated_at
             FROM test_cases
             WHERE suite = ?1
             ORDER BY suite, name",
        )?;
        let rows = stmt.query_map([suite], read_case_row)?;
        for row in rows {
            out.push(row?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT suite, name, source_label, method, url, items_json, headers_json,
                    expect_status, expect_headers_json, expect_json_json,
                    expect_body_contains_json, max_time_ms, created_at, updated_at
             FROM test_cases
             ORDER BY suite, name",
        )?;
        let rows = stmt.query_map([], read_case_row)?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

pub fn delete_test_case(suite: &str, name: &str) -> Result<bool> {
    let conn = open_connection()?;
    let affected = conn.execute(
        "DELETE FROM test_cases WHERE suite = ?1 AND name = ?2",
        params![suite, name],
    )?;
    Ok(affected > 0)
}

pub fn run_suite(suite: &str, config: &Config) -> Result<SuiteRunReport> {
    let cases = list_test_cases(Some(suite))?;
    if cases.is_empty() {
        return Err(anyhow!("no test cases found in suite '{suite}'"));
    }

    let conn = open_connection()?;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut summaries = Vec::new();
    let generated_at = Utc::now().to_rfc3339();

    for case in cases {
        let record = crate::sources::RequestRecord {
            name: case.name.clone(),
            method: case.method.clone(),
            url: case.url.clone(),
            items: case.items.clone(),
            headers: case.headers.iter().cloned().collect(),
            source_label: case.source_label.clone(),
        };
        let (trace, response, elapsed_ms) = execute_record(&record, config)?;
        let opts = TestOptions {
            expect_status: case.expect_status,
            expect_headers: case.expect_headers.clone(),
            expect_json: case.expect_json.clone(),
            expect_body_contains: case.expect_body_contains.clone(),
            max_time_ms: case.max_time_ms,
        };
        let report = evaluate_response(&trace.method, &trace.url, &response, elapsed_ms, &opts);
        if report.passed {
            passed += 1;
        } else {
            failed += 1;
        }
        persist_run(&conn, suite, &case.name, &report)?;
        summaries.push(TestRunSummary {
            case_name: case.name,
            passed: report.passed,
            status: report.status,
            elapsed_ms: report.elapsed_ms,
        });
    }

    Ok(SuiteRunReport {
        suite: suite.to_string(),
        generated_at,
        passed,
        failed,
        cases: summaries,
    })
}

pub fn latest_run_report(suite: &str, case_name: &str) -> Result<Option<TestReport>> {
    let conn = open_connection()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT report_json FROM test_runs
             WHERE suite = ?1 AND case_name = ?2
             ORDER BY id DESC LIMIT 1",
            params![suite, case_name],
            |row| row.get(0),
        )
        .optional()?;
    json.map(|raw| serde_json::from_str(&raw).context("failed to parse stored test report"))
        .transpose()
}

pub fn render_suite_report(report: &SuiteRunReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Suite {} [{}]\nPassed: {}  Failed: {}\n",
        report.suite, report.generated_at, report.passed, report.failed
    ));
    for case in &report.cases {
        let state = if case.passed { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "[{}] {} -> status={} time={}ms\n",
            state, case.case_name, case.status, case.elapsed_ms
        ));
    }
    out
}

pub fn render_case_history(suite: &str, case_name: &str) -> Result<String> {
    let conn = open_connection()?;
    let mut stmt = conn.prepare(
        "SELECT passed, status_code, elapsed_ms, created_at
         FROM test_runs
         WHERE suite = ?1 AND case_name = ?2
         ORDER BY id DESC
         LIMIT 10",
    )?;
    let mut rows = stmt.query(params![suite, case_name])?;
    let mut out = format!("Recent runs for {suite}/{case_name}\n");
    let mut any = false;
    while let Some(row) = rows.next()? {
        any = true;
        let passed: i64 = row.get(0)?;
        let status: i64 = row.get(1)?;
        let elapsed_ms: i64 = row.get(2)?;
        let created_at: String = row.get(3)?;
        let marker = if passed == 1 { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "[{}] status={} elapsed={}ms at {}\n",
            marker, status, elapsed_ms, created_at
        ));
    }
    if !any {
        out.push_str("No run history.\n");
    }
    Ok(out)
}

pub fn render_latest_case_report(suite: &str, case_name: &str) -> Result<String> {
    if let Some(report) = latest_run_report(suite, case_name)? {
        Ok(render_text_report(&report))
    } else {
        Ok(format!("No stored run report for {suite}/{case_name}\n"))
    }
}

fn persist_run(
    conn: &rusqlite::Connection,
    suite: &str,
    case_name: &str,
    report: &TestReport,
) -> Result<()> {
    conn.execute(
        "INSERT INTO test_runs (suite, case_name, passed, status_code, elapsed_ms, report_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            suite,
            case_name,
            if report.passed { 1 } else { 0 },
            i64::from(report.status),
            report.elapsed_ms as i64,
            serde_json::to_string_pretty(report)?,
            Utc::now().to_rfc3339(),
        ],
    )
    .context("failed to persist test run")?;
    Ok(())
}

fn read_case_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTestCase> {
    let items_json: String = row.get(5)?;
    let headers_json: String = row.get(6)?;
    let expect_headers_json: String = row.get(8)?;
    let expect_json_json: String = row.get(9)?;
    let expect_body_contains_json: String = row.get(10)?;
    Ok(StoredTestCase {
        suite: row.get(0)?,
        name: row.get(1)?,
        source_label: row.get(2)?,
        method: row.get(3)?,
        url: row.get(4)?,
        items: serde_json::from_str(&items_json).unwrap_or_default(),
        headers: serde_json::from_str(&headers_json).unwrap_or_default(),
        expect_status: row.get::<_, Option<i64>>(7)?.map(|value| value as u16),
        expect_headers: serde_json::from_str(&expect_headers_json).unwrap_or_default(),
        expect_json: serde_json::from_str(&expect_json_json).unwrap_or_default(),
        expect_body_contains: serde_json::from_str(&expect_body_contains_json).unwrap_or_default(),
        max_time_ms: row.get::<_, Option<i64>>(11)?.map(|value| value as u64),
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}
