use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::time::Instant;

use crate::cli::SourceSelector;
use crate::collections::{
    list_workspace_requests, load_request, load_requests_from_import_path, load_workspace_request,
    CollectionEntry,
};
use crate::config::{merge_defaults, Config};
use crate::items::parse_request_items;
use crate::request::{RequestEngine, RequestSpec};
use crate::response::{RequestTrace, ResponseData};
use crate::utils::normalize_url;

#[derive(Clone, Debug)]
pub struct RequestRecord {
    pub name: String,
    pub method: String,
    pub url: String,
    pub items: Vec<String>,
    pub headers: HashMap<String, String>,
    pub source_label: String,
}

pub fn resolve_records(selector: &SourceSelector) -> Result<Vec<RequestRecord>> {
    if selector.request.is_some() && selector.workspace.is_none() {
        return Err(anyhow!("--request requires --workspace"));
    }
    let selected = selected_source_count(selector);
    if selected == 0 {
        return Err(anyhow!(
            "choose exactly one source: --alias, --workspace, or --file"
        ));
    }
    if selected > 1 {
        return Err(anyhow!(
            "source flags are mutually exclusive; use only one of --alias, --workspace, or --file"
        ));
    }

    if let Some(alias) = selector.alias.as_deref() {
        let entry = load_request(alias)
            .with_context(|| format!("failed to load saved request '{alias}'"))?;
        return Ok(vec![record_from_entry(
            entry,
            format!("alias:{alias}"),
            alias.to_string(),
        )?]);
    }

    if let Some(workspace) = selector.workspace.as_deref() {
        if let Some(request) = selector.request.as_deref() {
            let entry = load_workspace_request(workspace, request).with_context(|| {
                format!(
                    "failed to load request '{}' from workspace '{}'",
                    request, workspace
                )
            })?;
            return Ok(vec![record_from_entry(
                entry,
                format!("request:{workspace}/{request}"),
                request.to_string(),
            )?]);
        }

        let entries = list_workspace_requests(workspace)
            .with_context(|| format!("failed to list workspace '{}'", workspace))?;
        let out = entries
            .into_iter()
            .map(|entry| {
                record_from_entry(
                    CollectionEntry {
                        alias: entry.name.clone(),
                        method: entry.method,
                        url: entry.url,
                        items: entry.items,
                        headers: entry.headers,
                        headers_v2: entry.headers_v2,
                        created: entry.created,
                    },
                    format!("workspace:{workspace}/{}", entry.name),
                    entry.name,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(out);
    }

    let Some(path) = selector.file.as_deref() else {
        return Err(anyhow!("no source selected"));
    };
    let entries = load_requests_from_import_path(path)
        .with_context(|| format!("failed to resolve requests from import file '{path}'"))?;
    entries
        .into_iter()
        .map(|entry| {
            let name = entry.alias.clone();
            record_from_entry(entry, format!("file:{path}#{name}"), name)
        })
        .collect::<Result<Vec<_>>>()
}

pub fn resolve_single_record(selector: &SourceSelector) -> Result<RequestRecord> {
    let records = resolve_records(selector)?;
    if records.len() != 1 {
        return Err(anyhow!(
            "this operation requires exactly one request source, but {} request(s) were resolved",
            records.len()
        ));
    }
    Ok(records.into_iter().next().expect("record count checked"))
}

pub fn build_cli_for_record(
    record: &RequestRecord,
    config: &Config,
) -> Result<crate::cli::CliArgs> {
    let mut argv = vec![
        "zapreq".to_string(),
        record.method.clone(),
        record.url.clone(),
    ];
    argv.extend(record.items.clone());
    merge_defaults(config, &mut argv);
    let mut cli = crate::cli::parse_cli_from(argv).context("failed to build CLI for request")?;
    cli.command = None;
    Ok(cli)
}

pub fn build_spec_for_record(
    record: &RequestRecord,
    config: &Config,
) -> Result<(crate::cli::CliArgs, RequestSpec)> {
    let cli = build_cli_for_record(record, config)?;
    let usable_url = normalize_url(&record.url, &cli.default_scheme)
        .with_context(|| format!("failed to normalize URL '{}'", record.url))?;

    let resolved_items = record.items.clone();
    let parsed_items =
        parse_request_items(&resolved_items).context("failed to parse saved request items")?;
    let headers = crate::headers::build_headers_from_cli(
        &cli,
        &parsed_items,
        &std::collections::HashMap::new(),
    )?;
    let spec = RequestSpec {
        method: cli.method.clone(),
        url: usable_url,
        items: parsed_items,
        headers,
    };
    Ok((cli, spec))
}

pub fn execute_record(
    record: &RequestRecord,
    config: &Config,
) -> Result<(RequestTrace, ResponseData, u64)> {
    let (cli, spec) = build_spec_for_record(record, config)?;
    let engine = RequestEngine::new();
    let started = Instant::now();
    let (trace, response) = engine
        .send(&cli, &spec, None)
        .with_context(|| format!("request execution failed for '{}'", record.name))?;
    Ok((trace, response, started.elapsed().as_millis() as u64))
}

pub fn scope_label_for_notes(selector: &SourceSelector) -> Result<String> {
    if let Some(alias) = selector.alias.as_deref() {
        return Ok(format!("alias:{alias}"));
    }
    if let Some(workspace) = selector.workspace.as_deref() {
        if let Some(request) = selector.request.as_deref() {
            return Ok(format!("request:{workspace}/{request}"));
        }
        return Ok(format!("workspace:{workspace}"));
    }
    Err(anyhow!(
        "notes support --alias or --workspace [--request]; import files are not a stable note scope"
    ))
}

fn record_from_entry(
    entry: CollectionEntry,
    source_label: String,
    name: String,
) -> Result<RequestRecord> {
    let items = crate::collections::materialize_entry_items(&entry)?;
    Ok(RequestRecord {
        name,
        method: entry.method,
        url: entry.url,
        items,
        headers: entry.headers,
        source_label,
    })
}

fn selected_source_count(selector: &SourceSelector) -> usize {
    usize::from(selector.alias.is_some())
        + usize::from(selector.workspace.is_some())
        + usize::from(selector.file.is_some())
}
