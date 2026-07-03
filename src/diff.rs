use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::auth::build_auth;
use crate::cli::Cli;
use crate::items::parse_request_items;
use crate::output::theme::Theme;
use crate::request::{RequestEngine, RequestSpec};

/// Diff output contract for comparing two request responses.
pub struct DiffResult {
    pub url_a: String,
    pub url_b: String,
    pub status_a: u16,
    pub status_b: u16,
    pub diff: Vec<DiffLine>,
}

/// Line-level response differences.
pub enum DiffLine {
    Same(String),
    OnlyA(String),
    OnlyB(String),
}

/// Sends both requests and computes a flattened JSON/text diff.
pub fn diff_requests(url_a: &str, url_b: &str, cli: &Cli) -> Result<DiffResult> {
    let response_a = send_one(url_a, cli).with_context(|| format!("request A failed: {url_a}"))?;
    let response_b = send_one(url_b, cli).with_context(|| format!("request B failed: {url_b}"))?;

    let map_a = flatten_body(&response_a.body);
    let map_b = flatten_body(&response_b.body);
    let diff = build_diff(&map_a, &map_b);

    Ok(DiffResult {
        url_a: url_a.to_string(),
        url_b: url_b.to_string(),
        status_a: response_a.status_code,
        status_b: response_b.status_code,
        diff,
    })
}

/// Prints user-friendly diff with status headers and colored markers.
pub fn print_diff(result: &DiffResult, theme: &Theme) {
    println!(
        "A: GET {} -> {}",
        result.url_a,
        result
            .status_a
            .to_string()
            .color(status_color(result.status_a, theme))
    );
    println!(
        "B: GET {} -> {}",
        result.url_b,
        result
            .status_b
            .to_string()
            .color(status_color(result.status_b, theme))
    );

    let mut only_a: BTreeMap<String, String> = BTreeMap::new();
    let mut only_b: BTreeMap<String, String> = BTreeMap::new();

    for line in &result.diff {
        match line {
            DiffLine::Same(s) => {
                let (k, v) = split_kv(s);
                println!("= {:<20} {}", k, v);
            }
            DiffLine::OnlyA(s) => {
                let (k, v) = split_kv(s);
                only_a.insert(k, v);
            }
            DiffLine::OnlyB(s) => {
                let (k, v) = split_kv(s);
                only_b.insert(k, v);
            }
        }
    }

    let mut keys = BTreeSet::new();
    keys.extend(only_a.keys().cloned());
    keys.extend(only_b.keys().cloned());
    for key in keys {
        match (only_a.get(&key), only_b.get(&key)) {
            (Some(a), Some(b)) => {
                println!(
                    "{}",
                    format!("~ {:<20} {} -> {}", key, a, b).color(theme.status_3xx)
                );
            }
            (Some(a), None) => {
                println!("{}", format!("- {:<20} {}", key, a).color(theme.status_4xx));
            }
            (None, Some(b)) => {
                println!("{}", format!("+ {:<20} {}", key, b).color(theme.status_2xx));
            }
            _ => {}
        }
    }
}

fn status_color(status: u16, theme: &Theme) -> colored::Color {
    if (200..300).contains(&status) {
        theme.status_2xx
    } else if (300..400).contains(&status) {
        theme.status_3xx
    } else if (400..500).contains(&status) {
        theme.status_4xx
    } else {
        theme.status_5xx
    }
}

fn split_kv(line: &str) -> (String, String) {
    if let Some((k, v)) = line.split_once('\t') {
        return (k.to_string(), v.to_string());
    }
    (line.to_string(), String::new())
}

fn send_one(url: &str, cli: &Cli) -> Result<crate::response::ResponseData> {
    let mut args = cli.clone();
    args.url = url.to_string();

    let parsed_items =
        parse_request_items(&args.request_items).context("failed to parse request items")?;
    let headers = crate::headers::build_headers_from_cli(
        &args,
        &parsed_items,
        &std::collections::HashMap::new(),
    )?;
    let spec = RequestSpec {
        method: args.method.clone(),
        url: url.to_string(),
        items: parsed_items,
        headers,
    };

    let auth_plugin = if let Some(auth) = args.auth.as_deref() {
        Some(build_auth(&args.auth_type, auth).context("failed to build auth plugin")?)
    } else {
        None
    };

    let engine = RequestEngine::new();
    let (_, response) = engine
        .send(&args, &spec, auth_plugin.as_deref())
        .context("request execution failed")?;
    Ok(response)
}

fn flatten_body(body: &[u8]) -> BTreeMap<String, String> {
    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        let mut out = BTreeMap::new();
        flatten_json("", &value, &mut out);
        return out;
    }

    String::from_utf8_lossy(body)
        .lines()
        .enumerate()
        .map(|(i, line)| (format!("line.{}", i + 1), line.to_string()))
        .collect()
}

fn flatten_json(prefix: &str, value: &Value, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let next = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(&next, v, out);
            }
        }
        Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                let next = format!("{prefix}[{idx}]");
                flatten_json(&next, item, out);
            }
        }
        _ => {
            out.insert(prefix.to_string(), value.to_string());
        }
    }
}

fn build_diff(map_a: &BTreeMap<String, String>, map_b: &BTreeMap<String, String>) -> Vec<DiffLine> {
    let mut keys = BTreeSet::new();
    keys.extend(map_a.keys().cloned());
    keys.extend(map_b.keys().cloned());

    let mut diff = Vec::new();
    for key in keys {
        match (map_a.get(&key), map_b.get(&key)) {
            (Some(a), Some(b)) if a == b => diff.push(DiffLine::Same(format!("{key}\t{a}"))),
            (Some(a), Some(b)) => {
                diff.push(DiffLine::OnlyA(format!("{key}\t{a}")));
                diff.push(DiffLine::OnlyB(format!("{key}\t{b}")));
            }
            (Some(a), None) => diff.push(DiffLine::OnlyA(format!("{key}\t{a}"))),
            (None, Some(b)) => diff.push(DiffLine::OnlyB(format!("{key}\t{b}"))),
            (None, None) => {}
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::{build_diff, DiffLine};
    use std::collections::BTreeMap;

    #[test]
    fn identical_json_all_same() {
        let a = BTreeMap::from([(String::from("user.name"), String::from("\"faizan\""))]);
        let b = a.clone();
        let diff = build_diff(&a, &b);
        assert!(diff.iter().all(|l| matches!(l, DiffLine::Same(_))));
    }

    #[test]
    fn a_has_extra_key_only_a() {
        let a = BTreeMap::from([
            (String::from("x"), String::from("1")),
            (String::from("extra"), String::from("\"admin\"")),
        ]);
        let b = BTreeMap::from([(String::from("x"), String::from("1"))]);
        let diff = build_diff(&a, &b);
        assert!(diff
            .iter()
            .any(|l| matches!(l, DiffLine::OnlyA(v) if v.starts_with("extra\t"))));
    }

    #[test]
    fn b_has_extra_key_only_b() {
        let a = BTreeMap::from([(String::from("x"), String::from("1"))]);
        let b = BTreeMap::from([
            (String::from("x"), String::from("1")),
            (String::from("extra"), String::from("\"viewer\"")),
        ]);
        let diff = build_diff(&a, &b);
        assert!(diff
            .iter()
            .any(|l| matches!(l, DiffLine::OnlyB(v) if v.starts_with("extra\t"))));
    }

    #[test]
    fn same_key_diff_value_shows_both() {
        let a = BTreeMap::from([(String::from("role"), String::from("\"admin\""))]);
        let b = BTreeMap::from([(String::from("role"), String::from("\"viewer\""))]);
        let diff = build_diff(&a, &b);
        let only_a = diff
            .iter()
            .any(|l| matches!(l, DiffLine::OnlyA(v) if v.starts_with("role\t")));
        let only_b = diff
            .iter()
            .any(|l| matches!(l, DiffLine::OnlyB(v) if v.starts_with("role\t")));
        assert!(only_a && only_b);
    }
}
