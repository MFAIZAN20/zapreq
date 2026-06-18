use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use regex::Regex;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::time::Instant;

use zapreq::ai::ai_assist;
use zapreq::auth::{build_auth, AuthRegistry};
use zapreq::cli::{
    is_known_subcommand_name, parse_cli_from, CliArgs, CollectionsCommand, Command, DocsCommand,
    EnvCommand, NotesCommand, PerfCommand, PluginCommand, RegressionCommand, RequestsCommand,
    SecretCommand, SecurityCommand,
};
use zapreq::collections::{
    add_request_to_workspace, create_workspace, delete_request, export_workspace, import_workspace,
    list_requests, list_workspace_requests, list_workspaces, load_request, load_workspace_request,
    migrate_legacy_collections, parse_export_format, run_request, save_request,
};
use zapreq::config::{apply_profile, load_config, load_profile, merge_defaults, CliResolved};
use zapreq::diff::{diff_requests, print_diff};
use zapreq::download::download;
use zapreq::env_cmd::{get_profile, list_profiles, validate_profile};
// egui desktop GUI removed, migrated to Tauri
use zapreq::items::parse_request_items;
use zapreq::items::RequestItem;
use zapreq::notes::{
    add_note, list_notes, note_history, render_history, render_notes, update_note,
};
use zapreq::output::{build_print_opts, render_exchange_from_cli};
use zapreq::perf::{benchmark, render_report as render_perf_report};
use zapreq::plugins::manager::{
    install_plugin, print_plugin_list, run_plugin_command, uninstall_plugin, validate_plugins,
};
use zapreq::regression::{
    add_test_case, delete_test_case, list_test_cases, render_case_history,
    render_latest_case_report, render_suite_report, run_suite,
};
use zapreq::request::{RequestEngine, RequestSpec};
use zapreq::response::{RequestTrace, ResponseData};
use zapreq::secrets::{get_secret, list_secret_keys, mask_secret, set_secret};
use zapreq::security::{render_report as render_security_report, run_scan};
use zapreq::sessions::SessionData;
use zapreq::sessions::{
    apply_session_to_request, load_session, save_session, update_session_from_exchange,
};
use zapreq::testing::{evaluate_response, render_text_report, TestOptions};
use zapreq::tui::run_advanced_tui;
use zapreq::utils::{humanize_bytes, humanize_duration, terminal_width, truncate_str};
use zapreq::zapdocs::{generate_docs, render_report as render_docs_report};

/// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-02, CAUS-CORERUNTIM-03, CAUS-CORERUNTIM-04, CAUS-CORERUNTIM-05, CAUS-INTERNAL-52:
/// Main orchestration entrypoint with explicit contract wiring, isolated runtime state transitions, and exit-code handling.
fn run() -> Result<i32> {
    let config = load_config().context("failed to load config")?;
    let (mut args, pending_test) = match prepare_run_args(&config)? {
        PreparedRun::Exit(code) => return Ok(code),
        PreparedRun::Continue { args, pending_test } => (*args, pending_test),
    };
    let PreparedRequestContext {
        usable_url,
        request_items,
        loaded_session,
    } = prepare_request_context(&mut args)?;
    let registry = AuthRegistry::with_defaults();
    warn_if_auth_incomplete(&args);
    let auth_plugin = if let Some(credentials) = args.auth.as_deref() {
        registry
            .get(&args.auth_type)
            .context("unsupported auth type requested")?;
        Some(build_auth(&args.auth_type, credentials).context("failed to configure auth plugin")?)
    } else {
        None
    };

    let spec = RequestSpec {
        method: args.method.clone(),
        url: usable_url,
        items: request_items.clone(),
    };

    let engine = RequestEngine::new();
    let print_opts = build_print_opts(&args, &config);

    if let Some(code) =
        maybe_run_offline_mode(&engine, &args, &spec, auth_plugin.as_deref(), &print_opts)?
    {
        return Ok(code);
    }
    if let Some(code) =
        maybe_run_download_mode(&engine, &args, &spec, auth_plugin.as_deref(), &print_opts)?
    {
        return Ok(code);
    }

    let executed = execute_request(&engine, &args, &spec, auth_plugin.as_deref())?;
    record_http_report(&executed.trace, &executed.response, executed.elapsed_ms);
    persist_session_updates(
        loaded_session,
        args.session_read_only,
        &request_items,
        &args.auth_type,
        args.auth.as_deref(),
        &executed.response,
    )?;

    if let Some(code) = maybe_render_test_report(
        pending_test,
        &executed.trace,
        &executed.response,
        executed.elapsed_ms,
    )? {
        return Ok(code);
    }

    render_standard_output(&args, &config, &executed)?;
    Ok(final_status_code(&args, executed.response.status_code))
}

type PendingTest = Option<(TestOptions, String)>;
type LoadedSession = Option<(std::path::PathBuf, SessionData)>;

enum PreparedRun {
    Exit(i32),
    Continue {
        args: Box<CliArgs>,
        pending_test: PendingTest,
    },
}

struct PreparedRequestContext {
    usable_url: String,
    request_items: Vec<RequestItem>,
    loaded_session: LoadedSession,
}

struct ExecutedRequest {
    trace: RequestTrace,
    response: ResponseData,
    elapsed_ms: u64,
}

fn prepare_run_args(config: &zapreq::config::Config) -> Result<PreparedRun> {
    let mut argv: Vec<String> = std::env::args().collect();
    if should_launch_default_gui(&argv) {
        println!("ZapReq Desktop GUI has been migrated to Tauri. Run the desktop app directly, or use `npm run tauri dev` / `cargo tauri dev` to start the GUI. Alternatively, run `zapreq tui` for the Terminal UI, or `zapreq --help` for help.");
        return Ok(PreparedRun::Exit(0));
    }
    if !is_raw_subcommand_invocation(&argv) {
        merge_defaults(config, &mut argv);
    }

    let mut args = parse_cli_from(argv).context("failed to parse CLI args")?;
    let mut pending_test = None;

    if let Some(command) = args.command.clone() {
        match handle_subcommand_match(command, args, config)? {
            SubcommandOutcome::Exit(code) => return Ok(PreparedRun::Exit(code)),
            SubcommandOutcome::RunRequest {
                args: new_args,
                pending_test: new_pending_test,
            } => {
                args = *new_args;
                pending_test = new_pending_test;
            }
        }
    }

    Ok(PreparedRun::Continue {
        args: Box::new(args),
        pending_test,
    })
}

fn prepare_request_context(args: &mut CliArgs) -> Result<PreparedRequestContext> {
    if args.url.is_empty() {
        return Err(anyhow!("URL is required unless using plugin subcommands"));
    }

    let resolved_url = resolve_request_url(args)?;
    let mut resolved_items = resolve_request_items(args)?;
    append_resume_range_header(args, &mut resolved_items);

    let usable_url = zapreq::utils::normalize_url(&resolved_url, &args.default_scheme)
        .context("failed to build usable URL")?;
    let mut request_items =
        parse_request_items(&resolved_items).context("failed to parse REQUEST_ITEMS")?;
    let loaded_session =
        load_session(&usable_url, args.session.as_deref()).context("failed to load session")?;
    apply_loaded_session(
        &loaded_session,
        &mut request_items,
        &mut args.auth_type,
        &mut args.auth,
        args.verbose,
    );

    Ok(PreparedRequestContext {
        usable_url,
        request_items,
        loaded_session,
    })
}

fn resolve_request_url(args: &CliArgs) -> Result<String> {
    let mut resolved = CliResolved {
        url: args.url.clone(),
        request_items: args.request_items.clone(),
        profile_headers: HashMap::new(),
        variables: load_env_variables(args)?,
    };

    if let Some(profile_name) = args.env_profile.as_deref() {
        let profile = load_profile(profile_name)
            .with_context(|| format!("failed to load env profile: {profile_name}"))?;
        apply_profile(&profile, &mut resolved);
    }

    let resolved_url = substitute_placeholders(&resolved.url, &resolved.variables);
    let resolved_items = collect_resolved_items(&resolved);
    validate_unresolved_values(&resolved_url, &resolved_items)?;
    Ok(resolved_url)
}

fn resolve_request_items(args: &CliArgs) -> Result<Vec<String>> {
    let mut resolved = CliResolved {
        url: args.url.clone(),
        request_items: args.request_items.clone(),
        profile_headers: HashMap::new(),
        variables: load_env_variables(args)?,
    };

    if let Some(profile_name) = args.env_profile.as_deref() {
        let profile = load_profile(profile_name)
            .with_context(|| format!("failed to load env profile: {profile_name}"))?;
        apply_profile(&profile, &mut resolved);
    }

    let resolved_url = substitute_placeholders(&resolved.url, &resolved.variables);
    let resolved_items = collect_resolved_items(&resolved);
    validate_unresolved_values(&resolved_url, &resolved_items)?;
    Ok(resolved_items)
}

fn load_env_variables(args: &CliArgs) -> Result<HashMap<String, String>> {
    if let Some(path) = args.env_file.as_deref() {
        load_env_file(path).with_context(|| format!("failed to load env file: {path}"))
    } else {
        Ok(HashMap::new())
    }
}

fn collect_resolved_items(resolved: &CliResolved) -> Vec<String> {
    let mut resolved_items = resolved
        .request_items
        .iter()
        .map(|raw| substitute_item_value(raw, &resolved.variables))
        .collect::<Vec<_>>();

    for (k, v) in &resolved.profile_headers {
        resolved_items.push(format!(
            "{}:{}",
            substitute_placeholders(k, &resolved.variables),
            substitute_placeholders(v, &resolved.variables)
        ));
    }

    resolved_items
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
        "unresolved variables: {} (set them via --env, --env-profile, or REQUEST_ITEMS)",
        unresolved.join(", ")
    ))
}

fn append_resume_range_header(args: &CliArgs, resolved_items: &mut Vec<String>) {
    if !(args.download && args.continue_download) {
        return;
    }
    let Some(output_path) = args.output.as_deref() else {
        return;
    };
    let Ok(meta) = std::fs::metadata(output_path) else {
        return;
    };
    let existing = meta.len();
    if existing > 0 {
        resolved_items.push(format!("Range:bytes={existing}-"));
    }
}

fn apply_loaded_session(
    loaded_session: &LoadedSession,
    request_items: &mut Vec<RequestItem>,
    auth_type: &mut String,
    auth: &mut Option<String>,
    verbose: bool,
) {
    let Some((_, session_data)) = loaded_session else {
        return;
    };

    if verbose {
        eprintln!(
            "[session: loaded {} cookies, {} headers]",
            session_data.cookies.len(),
            session_data.headers.len()
        );
    }

    apply_session_to_request(request_items, auth_type, auth, session_data);
}

fn warn_if_auth_incomplete(args: &CliArgs) {
    if args.auth.is_none() && !args.auth_type.eq_ignore_ascii_case("basic") {
        eprintln!(
            "warning: --auth-type={} provided without --auth; request sent without credentials",
            args.auth_type
        );
    }
}

fn maybe_run_offline_mode(
    engine: &RequestEngine,
    args: &CliArgs,
    spec: &RequestSpec,
    auth_plugin: Option<&dyn zapreq::auth::AuthPlugin>,
    print_opts: &zapreq::output::PrintOpts,
) -> Result<Option<i32>> {
    if !args.offline {
        return Ok(None);
    }

    let prepared = engine
        .prepare(args, spec, auth_plugin)
        .context("failed to prepare offline request")?;
    let mut offline_opts = print_opts.clone();
    offline_opts.request_headers = true;
    offline_opts.request_body = true;
    offline_opts.response_headers = false;
    offline_opts.response_body = false;
    zapreq::output::print_request(
        &prepared.method,
        &prepared.url,
        &prepared.headers_preview,
        prepared.body_preview.as_ref(),
        &offline_opts,
    );
    println!(
        "{}",
        "[offline mode — request not sent]"
            .color(offline_opts.theme.offline_msg)
            .bold()
    );
    Ok(Some(0))
}

fn maybe_run_download_mode(
    engine: &RequestEngine,
    args: &CliArgs,
    spec: &RequestSpec,
    auth_plugin: Option<&dyn zapreq::auth::AuthPlugin>,
    print_opts: &zapreq::output::PrintOpts,
) -> Result<Option<i32>> {
    if !args.download {
        return Ok(None);
    }

    let started = Instant::now();
    let (trace, response) = engine
        .send_raw_for_download(args, spec, auth_plugin)
        .context("download request failed")?;
    let download_result = download(response, args, &print_opts.theme).context("download failed")?;

    if args.verbose {
        println!("Downloaded via {} {}", trace.method, trace.url);
        println!("Saved to {}", download_result.filename);
        println!(
            "Bytes: {}  Duration: {:.2}s  Resumed: {}",
            download_result.size,
            download_result.duration.as_secs_f64(),
            download_result.resumed
        );
        println!(
            "Elapsed: {}",
            humanize_duration(started.elapsed().as_millis() as u64)
        );
    }

    Ok(Some(0))
}

fn execute_request(
    engine: &RequestEngine,
    args: &CliArgs,
    spec: &RequestSpec,
    auth_plugin: Option<&dyn zapreq::auth::AuthPlugin>,
) -> Result<ExecutedRequest> {
    let started = Instant::now();
    let (trace, response) = engine
        .send(args, spec, auth_plugin)
        .context("request execution failed")?;
    Ok(ExecutedRequest {
        trace,
        response,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn record_http_report(trace: &RequestTrace, response: &ResponseData, elapsed_ms: u64) {
    let _ = zapreq::localdb::record_http_report(
        "CLI",
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
}

fn persist_session_updates(
    loaded_session: LoadedSession,
    session_read_only: bool,
    request_items: &[RequestItem],
    auth_type: &str,
    auth: Option<&str>,
    response: &ResponseData,
) -> Result<()> {
    let Some((session_path, mut session_data)) = loaded_session else {
        return Ok(());
    };
    if session_read_only {
        return Ok(());
    }

    update_session_from_exchange(&mut session_data, request_items, auth_type, auth, response);
    save_session(&session_path, &session_data).context("failed to save session")
}

fn maybe_render_test_report(
    pending_test: PendingTest,
    trace: &RequestTrace,
    response: &ResponseData,
    elapsed_ms: u64,
) -> Result<Option<i32>> {
    let Some((test_opts, report_kind)) = pending_test else {
        return Ok(None);
    };

    let report = evaluate_response(&trace.method, &trace.url, response, elapsed_ms, &test_opts);
    if report_kind.eq_ignore_ascii_case("json") {
        let json =
            serde_json::to_string_pretty(&report).context("failed to serialize test report")?;
        println!("{json}");
    } else {
        print!("{}", render_text_report(&report));
    }
    Ok(Some(if report.passed { 0 } else { 1 }))
}

fn render_standard_output(
    args: &CliArgs,
    config: &zapreq::config::Config,
    executed: &ExecutedRequest,
) -> Result<()> {
    render_exchange_from_cli(&executed.trace, &executed.response, args, config)
        .context("failed to render output")?;
    if args.verbose {
        if let Some(auth) = args.auth.as_deref() {
            eprintln!("Auth: {}", mask_auth(&args.auth_type, auth));
        }
    }
    if args.summary && !args.no_summary {
        print_compact_summary(&executed.trace, &executed.response, executed.elapsed_ms);
    }
    if args.meta {
        print_meta_summary(
            &executed.trace.method,
            &executed.trace.url,
            executed.response.status_code,
            &executed.response.reason,
            executed.elapsed_ms,
            &executed.response,
            infer_ssl_label(&executed.trace.url, args.ssl.as_deref()),
        );
    }
    Ok(())
}

fn final_status_code(args: &CliArgs, status_code: u16) -> i32 {
    if args.check_status && status_code >= 400 {
        1
    } else {
        0
    }
}

fn is_raw_subcommand_invocation(argv: &[String]) -> bool {
    argv.get(1)
        .map(String::as_str)
        .is_some_and(is_known_subcommand_name)
}

fn should_launch_default_gui(argv: &[String]) -> bool {
    argv.len() <= 1
}

/// CAUS-INTERNAL-51, CAUS-INTERNAL-55:
/// Process entrypoint with user-friendly error printing and exit codes.
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("zapreq error: {err}");
            2
        }
    };
    std::process::exit(code);
}

fn load_env_file(path: &str) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    let mut out = HashMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        if key.is_empty() {
            continue;
        }
        let mut value = v.trim().to_string();
        let double_quoted = value.starts_with('"') && value.ends_with('"');
        let single_quoted = value.starts_with('\'') && value.ends_with('\'');
        if value.len() >= 2 && (double_quoted || single_quoted) {
            value = value[1..value.len() - 1].to_string();
        } else if let Some((head, _comment)) = value.split_once(" #") {
            value = head.trim().to_string();
        }
        out.insert(key.to_string(), value);
    }

    Ok(out)
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let key = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or_default();
        if let Some(val) = vars.get(key) {
            val.clone()
        } else if let Ok(Some(secret_val)) = get_secret(key) {
            secret_val
        } else {
            caps[0].to_string()
        }
    })
    .into_owned()
}

fn unresolved_placeholders(values: &[&str]) -> Vec<String> {
    let re = Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    let mut unresolved = BTreeSet::new();
    for value in values {
        for caps in re.captures_iter(value) {
            let name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or_default();
            if !name.is_empty() {
                unresolved.insert(name.to_string());
            }
        }
    }
    unresolved.into_iter().collect()
}

fn substitute_item_value(raw: &str, vars: &HashMap<String, String>) -> String {
    let token = raw.trim();

    if let Some((k, v)) = token.split_once(":=@") {
        return format!("{}:=@{}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once(":=") {
        return format!("{}:={}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once("==") {
        return format!("{}=={}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once(':') {
        return format!("{}:{}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once("=@") {
        return format!("{}=@{}", k, substitute_placeholders(v, vars));
    }

    if let Some((k, v)) = token.split_once('=') {
        if token.contains('@') && token.contains(";type=") {
            // typed upload is handled by the @ operator branch below
        } else {
            return format!("{}={}", k, substitute_placeholders(v, vars));
        }
    }

    if let Some((k, v)) = token.split_once('@') {
        if let Some((path, ct)) = v.split_once(";type=") {
            return format!(
                "{}@{};type={}",
                k,
                substitute_placeholders(path, vars),
                substitute_placeholders(ct, vars)
            );
        }
        return format!("{}@{}", k, substitute_placeholders(v, vars));
    }

    substitute_placeholders(token, vars)
}

fn infer_ssl_label(url: &str, cli_ssl: Option<&str>) -> String {
    if !url.starts_with("https://") {
        return "none".to_string();
    }
    if let Some(explicit) = cli_ssl {
        return explicit.to_uppercase();
    }
    "TLS(auto)".to_string()
}

fn print_meta_summary(
    method: &str,
    url: &str,
    status: u16,
    reason: &str,
    elapsed_ms: u64,
    response: &ResponseData,
    ssl_label: String,
) {
    let size = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<u64>().ok())
        .unwrap_or(response.body.len() as u64);

    let mut rows = vec![
        format!("Method:   {}", method),
        format!("URL:      {}", url),
        format!("Status:   {} {}", status, reason),
        format!("Time:     {}", humanize_duration(elapsed_ms)),
        format!("Size:     {}", humanize_bytes(size)),
        format!("SSL:      {}", ssl_label),
    ];

    let max_inner = terminal_width().saturating_sub(4).max(20);
    for row in &mut rows {
        *row = truncate_str(row, max_inner);
    }

    let inner_width = rows
        .iter()
        .map(|r| r.chars().count())
        .max()
        .unwrap_or(20)
        .max(20);

    println!("┌{}┐", "─".repeat(inner_width + 2));
    for row in rows {
        let pad = inner_width.saturating_sub(row.chars().count());
        println!("│ {}{} │", row, " ".repeat(pad));
    }
    println!("└{}┘", "─".repeat(inner_width + 2));
}

enum SubcommandOutcome {
    Exit(i32),
    RunRequest {
        args: Box<CliArgs>,
        pending_test: Option<(TestOptions, String)>,
    },
}

fn handle_subcommand_match(
    command: Command,
    args: CliArgs,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        Command::Plugins { command } => handle_plugins_cmd(command, config),
        Command::Save { alias, request } => handle_save_cmd(alias, request, config),
        Command::Tui => handle_tui_cmd(config),
        Command::Gui => handle_gui_cmd(),
        Command::Run { alias, env_profile } => handle_run_cmd(alias, env_profile, config),
        Command::List => handle_list_cmd(),
        Command::Delete { alias } => handle_delete_cmd(alias),
        Command::Ai {
            prompt,
            send,
            save,
            explain,
            env_profile,
        } => handle_ai_cmd(prompt, send, save, explain, env_profile, config),
        Command::Test {
            expect_status,
            expect_header,
            expect_json,
            expect_body_contains,
            max_time_ms,
            report,
            request,
        } => handle_test_cmd(
            expect_status,
            expect_header,
            expect_json,
            expect_body_contains,
            max_time_ms,
            report,
            request,
            config,
        ),
        Command::Env { command } => handle_env_cmd(command),
        Command::Collections { command } => handle_collections_cmd(command),
        Command::Requests { command } => handle_requests_cmd(command, config),
        Command::Security { command } => handle_security_cmd(command, config),
        Command::Docs { command } => handle_docs_cmd(command),
        Command::Regression { command } => handle_regression_cmd(command, config),
        Command::Perf { command } => handle_perf_cmd(command, config),
        Command::Notes { command } => handle_notes_cmd(command),
        Command::Secrets { command } => handle_secrets_cmd(command),
        Command::Diff {
            url_a,
            url_b,
            request,
        } => handle_diff_cmd(url_a, url_b, request, args, config),
    }
}

fn handle_plugins_cmd(
    command: PluginCommand,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        PluginCommand::Install { name } => install_plugin(&name)?,
        PluginCommand::Uninstall { name } => uninstall_plugin(&name, config)?,
        PluginCommand::List => print_plugin_list(config)?,
        PluginCommand::Validate => {
            let issues = validate_plugins(config)?;
            return Ok(SubcommandOutcome::Exit(if issues == 0 { 0 } else { 1 }));
        }
        PluginCommand::Run { name, args } => {
            return Ok(SubcommandOutcome::Exit(run_plugin_command(
                &name, &args, config,
            )?));
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_save_cmd(
    alias: String,
    request: Vec<String>,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    let saved = cli_from_saved_request_tokens(&request, config)?;
    save_request(&alias, &saved).with_context(|| format!("failed to save collection '{alias}'"))?;
    println!("Saved request as '{alias}'");
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_tui_cmd(config: &zapreq::config::Config) -> Result<SubcommandOutcome> {
    run_advanced_tui(config)?;
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_gui_cmd() -> Result<SubcommandOutcome> {
    println!("ZapReq Desktop GUI has been migrated to Tauri. Run the desktop app directly, or use `npm run tauri dev` / `cargo tauri dev` in the development directory.");
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_run_cmd(
    alias: String,
    env_profile: Option<String>,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    let new_args = build_args_from_collection(&alias, env_profile, config)?;
    Ok(SubcommandOutcome::RunRequest {
        args: Box::new(new_args),
        pending_test: None,
    })
}

fn handle_list_cmd() -> Result<SubcommandOutcome> {
    let entries = list_requests().context("failed to list saved requests")?;
    if entries.is_empty() {
        println!("No saved requests.");
    } else {
        for e in entries {
            println!("{}  {} {}", e.alias, e.method, e.url);
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_delete_cmd(alias: String) -> Result<SubcommandOutcome> {
    delete_request(&alias).with_context(|| format!("failed to delete collection '{alias}'"))?;
    println!("Deleted request '{alias}'");
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_ai_cmd(
    prompt: String,
    send: bool,
    save: Option<String>,
    explain: bool,
    env_profile: Option<String>,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    let api_key = match std::env::var("ZAPREQ_AI_KEY") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("ZAPREQ_AI_KEY is not set. Export it first to use `zapreq ai`.");
            return Ok(SubcommandOutcome::Exit(1));
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build async runtime for AI assistant")?;
    let generated = runtime
        .block_on(ai_assist(&prompt, &api_key))
        .context("AI assistant request failed")?;
    let mut generated_items = Vec::new();
    for (k, v) in &generated.headers {
        generated_items.push(format!("{k}:{v}"));
    }
    for (k, v) in &generated.query {
        generated_items.push(format!("{k}=={v}"));
    }
    for (k, v) in &generated.body {
        if let Some(s) = v.as_str() {
            generated_items.push(format!("{k}={s}"));
        } else {
            generated_items.push(format!("{k}:={}", v));
        }
    }
    let method = if generated.method.trim().is_empty() {
        "GET".to_string()
    } else {
        generated.method.to_ascii_uppercase()
    };
    if generated.url.trim().is_empty() {
        return Err(anyhow!("AI assistant did not return a URL"));
    }
    let command_preview = format!(
        "zapreq {} {} {}",
        method,
        generated.url,
        generated_items.join(" ")
    );
    println!("Generated command: {command_preview}");
    if explain {
        println!("Method: {}", method);
        println!("URL: {}", generated.url);
        println!("Headers: {}", generated.headers.len());
        println!("Query params: {}", generated.query.len());
        println!("Body fields: {}", generated.body.len());
    }

    let mut synthetic = vec!["zapreq".to_string(), method.clone(), generated.url.clone()];
    synthetic.extend(generated_items.clone());
    merge_defaults(config, &mut synthetic);
    let mut generated_cli =
        parse_cli_from(synthetic).context("failed to parse AI-generated command")?;
    if generated_cli.env_profile.is_none() {
        generated_cli.env_profile = env_profile;
    }

    if let Some(alias) = save {
        save_request(&alias, &generated_cli)
            .with_context(|| format!("failed to save AI-generated request '{alias}'"))?;
        println!("Saved AI-generated request as '{alias}'");
    }

    if !send {
        println!("Dry run only. Re-run with `--send` to execute.");
        return Ok(SubcommandOutcome::Exit(0));
    }

    Ok(SubcommandOutcome::RunRequest {
        args: Box::new(generated_cli),
        pending_test: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_test_cmd(
    expect_status: Option<u16>,
    expect_header: Vec<String>,
    expect_json: Vec<String>,
    expect_body_contains: Vec<String>,
    max_time_ms: Option<u64>,
    report: String,
    request: Vec<String>,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    if request.is_empty() {
        return Err(anyhow!(
            "test requires request tokens: `zapreq test [ASSERTS] -- METHOD URL [ITEMS...]`"
        ));
    }
    let mut parsed = cli_from_saved_request_tokens(&request, config)
        .context("failed to parse test request tokens")?;
    parsed.command = None;
    let pending_test = Some((
        TestOptions {
            expect_status,
            expect_headers: expect_header,
            expect_json,
            expect_body_contains,
            max_time_ms,
        },
        report,
    ));
    Ok(SubcommandOutcome::RunRequest {
        args: Box::new(parsed),
        pending_test,
    })
}

fn handle_env_cmd(command: EnvCommand) -> Result<SubcommandOutcome> {
    match command {
        EnvCommand::List => {
            let profiles = list_profiles().context("failed to list env profiles")?;
            if profiles.is_empty() {
                println!("No env profiles found.");
            } else {
                for name in profiles {
                    println!("{name}");
                }
            }
        }
        EnvCommand::Show { name } => {
            let profile = get_profile(&name).with_context(|| format!("failed to show {name}"))?;
            let text =
                serde_json::to_string_pretty(&profile).context("failed to serialize profile")?;
            println!("{text}");
        }
        EnvCommand::Validate { name } => {
            let issues =
                validate_profile(&name).with_context(|| format!("failed to validate {name}"))?;
            if issues.is_empty() {
                println!("Profile '{name}' is valid.");
            } else {
                println!("Profile '{name}' has {} issue(s):", issues.len());
                for issue in issues {
                    println!("- {issue}");
                }
                return Ok(SubcommandOutcome::Exit(1));
            }
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_collections_cmd(command: CollectionsCommand) -> Result<SubcommandOutcome> {
    match command {
        CollectionsCommand::List => {
            let workspaces = list_workspaces().context("failed to list workspaces")?;
            if workspaces.is_empty() {
                println!("No workspaces found.");
            } else {
                for ws in workspaces {
                    println!(
                        "{}  requests={}  updated={}",
                        ws.name, ws.request_count, ws.updated
                    );
                }
            }
        }
        CollectionsCommand::New { name } => {
            let ws = create_workspace(&name)
                .with_context(|| format!("failed to create workspace '{name}'"))?;
            println!(
                "Workspace '{}' ready ({} requests).",
                ws.name,
                ws.requests.len()
            );
        }
        CollectionsCommand::Import { name, path } => {
            let ws = import_workspace(&name, &path)
                .with_context(|| format!("failed to import workspace from '{}'", path))?;
            println!(
                "Imported workspace '{}' with {} request(s).",
                ws.name,
                ws.requests.len()
            );
        }
        CollectionsCommand::Export { name, path, format } => {
            let fmt = parse_export_format(&format)?;
            let output_path = export_workspace(&name, &path, fmt)
                .with_context(|| format!("failed to export workspace '{}'", name))?;
            println!("Exported workspace '{}' to {}", name, output_path.display());
        }
        CollectionsCommand::Migrate { workspace } => {
            let report = migrate_legacy_collections(&workspace)?;
            println!(
                "Migration complete -> workspace='{}' imported={} skipped_existing={}",
                report.workspace, report.imported, report.skipped_existing
            );
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_requests_cmd(
    command: RequestsCommand,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        RequestsCommand::List { workspace } => {
            let requests = list_workspace_requests(&workspace)
                .with_context(|| format!("failed to list requests in '{}'", workspace))?;
            if requests.is_empty() {
                println!("Workspace '{}' has no requests.", workspace);
            } else {
                for request in requests {
                    println!("{}  {} {}", request.name, request.method, request.url);
                }
            }
            Ok(SubcommandOutcome::Exit(0))
        }
        RequestsCommand::Run {
            workspace,
            request,
            env_profile,
        } => {
            let new_args =
                build_args_from_workspace_request(&workspace, &request, env_profile, config)?;
            Ok(SubcommandOutcome::RunRequest {
                args: Box::new(new_args),
                pending_test: None,
            })
        }
        RequestsCommand::Save {
            workspace,
            name,
            request,
        } => {
            let parsed = cli_from_saved_request_tokens(&request, config)
                .context("failed to parse request tokens for workspace save")?;
            add_request_to_workspace(&workspace, &name, &parsed).with_context(|| {
                format!(
                    "failed to save request '{}' to workspace '{}'",
                    name, workspace
                )
            })?;
            println!("Saved request '{}' to workspace '{}'.", name, workspace);
            Ok(SubcommandOutcome::Exit(0))
        }
    }
}

fn handle_security_cmd(
    command: SecurityCommand,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        SecurityCommand::Scan {
            source,
            severity,
            live,
        } => {
            let report =
                run_scan(&source, severity, live, config).context("security scan failed")?;
            print!("{}", render_security_report(&report));
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_docs_cmd(command: DocsCommand) -> Result<SubcommandOutcome> {
    match command {
        DocsCommand::Generate {
            source,
            format,
            output,
        } => {
            let report = generate_docs(&source, format, output.as_deref())
                .context("documentation generation failed")?;
            print!("{}", render_docs_report(&report));
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_regression_cmd(
    command: RegressionCommand,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        RegressionCommand::Add {
            suite,
            name,
            source,
            expect_status,
            expect_header,
            expect_json,
            expect_body_contains,
            max_time_ms,
        } => {
            let opts = TestOptions {
                expect_status,
                expect_headers: expect_header,
                expect_json,
                expect_body_contains,
                max_time_ms,
            };
            add_test_case(&suite, &name, &source, &opts)
                .context("failed to save regression test case")?;
            println!("Saved test case '{}' in suite '{}'.", name, suite);
        }
        RegressionCommand::List { suite } => {
            let cases =
                list_test_cases(suite.as_deref()).context("failed to list regression cases")?;
            if cases.is_empty() {
                println!("No regression test cases found.");
            } else {
                for case in cases {
                    println!(
                        "{}  {}  {} {}",
                        case.suite, case.name, case.method, case.url
                    );
                }
            }
        }
        RegressionCommand::Run { suite } => {
            let report = run_suite(&suite, config).context("failed to execute regression suite")?;
            print!("{}", render_suite_report(&report));
            return Ok(SubcommandOutcome::Exit(if report.failed == 0 {
                0
            } else {
                1
            }));
        }
        RegressionCommand::Delete { suite, name } => {
            let removed = delete_test_case(&suite, &name).context("failed to delete test case")?;
            if removed {
                println!("Deleted test case '{}' from suite '{}'.", name, suite);
            } else {
                println!("No test case named '{}' found in suite '{}'.", name, suite);
                return Ok(SubcommandOutcome::Exit(1));
            }
        }
        RegressionCommand::History { suite, name } => {
            print!("{}", render_case_history(&suite, &name)?);
            print!("{}", render_latest_case_report(&suite, &name)?);
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_perf_cmd(
    command: PerfCommand,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    match command {
        PerfCommand::Benchmark {
            source,
            iterations,
            duration_secs,
        } => {
            let report = benchmark(&source, iterations, duration_secs, config)
                .context("performance benchmark failed")?;
            print!("{}", render_perf_report(&report));
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_notes_cmd(command: NotesCommand) -> Result<SubcommandOutcome> {
    match command {
        NotesCommand::Add {
            source,
            title,
            tags,
            body,
        } => {
            let note =
                add_note(&source, title.as_deref(), &body, &tags).context("failed to add note")?;
            print!("{}", render_notes(&[note]));
        }
        NotesCommand::Update {
            id,
            title,
            tags,
            body,
        } => {
            let note =
                update_note(id, title.as_deref(), &body, &tags).context("failed to update note")?;
            print!("{}", render_notes(&[note]));
        }
        NotesCommand::List { source, query } => {
            let filter = if source.alias.is_none()
                && source.workspace.is_none()
                && source.file.is_none()
                && source.request.is_none()
            {
                None
            } else {
                Some(&source)
            };
            let notes = list_notes(filter, query.as_deref()).context("failed to list notes")?;
            print!("{}", render_notes(&notes));
        }
        NotesCommand::History { id } => {
            let history = note_history(id).context("failed to load note history")?;
            print!("{}", render_history(&history));
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_secrets_cmd(command: SecretCommand) -> Result<SubcommandOutcome> {
    match command {
        SecretCommand::Set { key, value } => {
            set_secret(&key, &value).with_context(|| format!("failed to save secret '{key}'"))?;
            println!("Secret '{key}' saved.");
        }
        SecretCommand::Get { key, reveal } => {
            let value =
                get_secret(&key).with_context(|| format!("failed to read secret '{key}'"))?;
            match value {
                Some(v) => {
                    if reveal {
                        println!("{v}");
                    } else {
                        println!("{}", mask_secret(&v));
                    }
                }
                None => {
                    println!("Secret '{key}' not found.");
                    return Ok(SubcommandOutcome::Exit(1));
                }
            }
        }
        SecretCommand::List => {
            let keys = list_secret_keys().context("failed to list secrets")?;
            if keys.is_empty() {
                println!("No secrets stored.");
            } else {
                for key in keys {
                    println!("{key}");
                }
            }
        }
    }
    Ok(SubcommandOutcome::Exit(0))
}

fn handle_diff_cmd(
    url_a: String,
    url_b: String,
    request: Vec<String>,
    args: CliArgs,
    config: &zapreq::config::Config,
) -> Result<SubcommandOutcome> {
    let mut diff_cli = if request.is_empty() {
        args
    } else {
        cli_from_diff_tokens(&url_a, &request, config)?
    };
    diff_cli.command = None;
    let result = diff_requests(&url_a, &url_b, &diff_cli).context("diff command failed")?;
    let opts = build_print_opts(&diff_cli, config);
    print_diff(&result, &opts.theme);
    Ok(SubcommandOutcome::Exit(0))
}

fn cli_from_saved_request_tokens(
    request: &[String],
    config: &zapreq::config::Config,
) -> Result<CliArgs> {
    if request.is_empty() {
        return Err(anyhow!(
            "save requires request tokens: use `zapreq save <alias> -- METHOD URL [ITEMS...]`"
        ));
    }
    let mut tokens = request.to_vec();
    if tokens.first().map(|t| t.as_str()) == Some("--") {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return Err(anyhow!("no request tokens supplied after `--`"));
    }

    let mut argv = vec!["zapreq".to_string()];
    argv.extend(tokens);
    merge_defaults(config, &mut argv);
    let parsed = parse_cli_from(argv).context("failed to parse request tokens for save")?;
    if parsed.command.is_some() {
        return Err(anyhow!("nested subcommands are not allowed in `save`"));
    }
    Ok(parsed)
}

fn build_args_from_collection(
    alias: &str,
    env_profile: Option<String>,
    config: &zapreq::config::Config,
) -> Result<CliArgs> {
    run_request(alias, env_profile.as_deref())?;
    let entry =
        load_request(alias).with_context(|| format!("failed to load collection '{alias}'"))?;
    let mut synthetic = vec![
        "zapreq".to_string(),
        entry.method.clone(),
        entry.url.clone(),
    ];
    synthetic.extend(entry.items.clone());
    merge_defaults(config, &mut synthetic);
    let mut args = parse_cli_from(synthetic).context("failed to parse saved request")?;
    if args.env_profile.is_none() {
        args.env_profile = env_profile;
    }
    for (k, v) in entry.headers {
        args.request_items.push(format!("{k}:{v}"));
    }
    Ok(args)
}

fn build_args_from_workspace_request(
    workspace: &str,
    request_ref: &str,
    env_profile: Option<String>,
    config: &zapreq::config::Config,
) -> Result<CliArgs> {
    let entry = load_workspace_request(workspace, request_ref).with_context(|| {
        format!(
            "failed to load request '{}' from '{}'",
            request_ref, workspace
        )
    })?;
    let mut synthetic = vec![
        "zapreq".to_string(),
        entry.method.clone(),
        entry.url.clone(),
    ];
    synthetic.extend(entry.items.clone());
    merge_defaults(config, &mut synthetic);
    let mut args = parse_cli_from(synthetic).context("failed to parse workspace request")?;
    if args.env_profile.is_none() {
        args.env_profile = env_profile;
    }
    for (k, v) in entry.headers {
        args.request_items.push(format!("{k}:{v}"));
    }
    Ok(args)
}

fn print_compact_summary(
    trace: &zapreq::response::RequestTrace,
    response: &ResponseData,
    elapsed_ms: u64,
) {
    let content_type = response
        .content_type
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    println!(
        "{} {} -> {} {} ({} ms, {} bytes, {})",
        trace.method,
        trace.url,
        response.status_code,
        response.reason,
        elapsed_ms,
        response.body.len(),
        content_type
    );
}

fn mask_auth(auth_type: &str, auth: &str) -> String {
    if auth_type.eq_ignore_ascii_case("basic") {
        if let Some((user, _)) = auth.split_once(':') {
            return format!("{user}:****");
        }
    }
    "****".to_string()
}

fn cli_from_diff_tokens(
    url: &str,
    request: &[String],
    config: &zapreq::config::Config,
) -> Result<CliArgs> {
    let mut tokens = request.to_vec();
    if tokens.first().map(|t| t.as_str()) == Some("--") {
        tokens.remove(0);
    }

    let mut argv = vec!["zapreq".to_string()];
    if let Some(first) = tokens.first() {
        let upper = first.to_ascii_uppercase();
        let looks_like_method = matches!(
            upper.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
        );
        if looks_like_method {
            argv.push(tokens.remove(0));
        } else {
            argv.push("GET".to_string());
        }
    } else {
        argv.push("GET".to_string());
    }
    argv.push(url.to_string());
    argv.extend(tokens);
    merge_defaults(config, &mut argv);
    let parsed = parse_cli_from(argv).context("failed to parse diff request options")?;
    if parsed.command.is_some() {
        return Err(anyhow!("nested subcommands are not allowed in `diff`"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{
        append_resume_range_header, collect_resolved_items, final_status_code, infer_ssl_label,
        is_raw_subcommand_invocation, load_env_file, maybe_render_test_report, parse_cli_from,
        resolve_request_items, resolve_request_url, should_launch_default_gui,
        substitute_item_value, validate_unresolved_values,
    };
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;
    use zapreq::config::CliResolved;
    use zapreq::response::{RequestTrace, ResponseData};
    use zapreq::testing::TestOptions;

    fn parse_args(argv: &[&str]) -> zapreq::cli::CliArgs {
        parse_cli_from(
            argv.iter()
                .map(|arg| (*arg).to_string())
                .collect::<Vec<_>>(),
        )
        .expect("cli args should parse")
    }

    #[test]
    fn no_args_launches_default_gui() {
        assert!(should_launch_default_gui(&["zapreq".to_string()]));
        assert!(!should_launch_default_gui(&[
            "zapreq".to_string(),
            "GET".to_string()
        ]));
    }

    #[test]
    fn gui_commands_are_treated_as_raw_subcommands() {
        assert!(is_raw_subcommand_invocation(&[
            "zapreq".to_string(),
            "ui".to_string()
        ]));
        assert!(is_raw_subcommand_invocation(&[
            "zapreq".to_string(),
            "gui".to_string()
        ]));
        assert!(is_raw_subcommand_invocation(&[
            "zapreq".to_string(),
            "tui".to_string()
        ]));
    }

    #[test]
    fn load_env_file_parses_quotes_comments_and_whitespace() {
        let dir = tempdir().expect("tempdir should be created");
        let path = dir.path().join(".env");
        fs::write(
            &path,
            "HOST=api.example.com\nEMPTY=\nQUOTED=\"hello world\"\nSINGLE='abc'\nTRIM=value # inline comment\n# ignored\n INVALID\n",
        )
        .expect("env file should be written");

        let values = load_env_file(path.to_str().expect("path should be utf-8"))
            .expect("env file should parse");

        assert_eq!(values.get("HOST"), Some(&"api.example.com".to_string()));
        assert_eq!(values.get("EMPTY"), Some(&"".to_string()));
        assert_eq!(values.get("QUOTED"), Some(&"hello world".to_string()));
        assert_eq!(values.get("SINGLE"), Some(&"abc".to_string()));
        assert_eq!(values.get("TRIM"), Some(&"value".to_string()));
        assert!(!values.contains_key("INVALID"));
    }

    #[test]
    fn resolve_request_parts_apply_env_file_variables() {
        let dir = tempdir().expect("tempdir should be created");
        let env_path = dir.path().join(".env");
        fs::write(
            &env_path,
            "HOST=api.example.com\nLIMIT=25\nTOKEN=secret-token\nUPLOAD=docs/spec.json\nCTYPE=application/json\n",
        )
        .expect("env file should be written");

        let args = parse_args(&[
            "zapreq",
            "GET",
            "https://{HOST}/users",
            "limit=={LIMIT}",
            "Authorization:{TOKEN}",
            "spec@{UPLOAD};type={CTYPE}",
            "--env",
            env_path.to_str().expect("path should be utf-8"),
        ]);

        let resolved_url = resolve_request_url(&args).expect("url should resolve");
        let resolved_items = resolve_request_items(&args).expect("items should resolve");
        append_resume_range_header(&args, &mut Vec::new());

        assert_eq!(resolved_url, "https://api.example.com/users");
        assert!(resolved_items.contains(&"limit==25".to_string()));
        assert!(resolved_items.contains(&"Authorization:secret-token".to_string()));
        assert!(resolved_items.contains(&"spec@docs/spec.json;type=application/json".to_string()));
    }

    #[test]
    fn collect_resolved_items_merges_profile_headers() {
        let mut variables = HashMap::new();
        variables.insert("TOKEN".to_string(), "abc123".to_string());
        let mut profile_headers = HashMap::new();
        profile_headers.insert("Authorization".to_string(), "Bearer {TOKEN}".to_string());
        let resolved = CliResolved {
            url: "https://example.com".to_string(),
            request_items: vec!["q=={TOKEN}".to_string()],
            profile_headers,
            variables,
        };

        let items = collect_resolved_items(&resolved);

        assert_eq!(items[0], "q==abc123");
        assert!(items.contains(&"Authorization:Bearer abc123".to_string()));
    }

    #[test]
    fn validate_unresolved_values_reports_sorted_unique_names() {
        let err = validate_unresolved_values(
            "https://{HOST}/v1/{HOST}",
            &["token:{TOKEN}".to_string(), "query=={ACCOUNT}".to_string()],
        )
        .expect_err("unresolved placeholders should be rejected");

        let message = err.to_string();
        assert!(message.contains("ACCOUNT, HOST, TOKEN"));
    }

    #[test]
    fn substitute_item_value_supports_all_item_operator_shapes() {
        let mut vars = HashMap::new();
        vars.insert("VALUE".to_string(), "hello".to_string());
        vars.insert("PATH".to_string(), "fixtures/payload.json".to_string());
        vars.insert("CTYPE".to_string(), "application/json".to_string());

        assert_eq!(substitute_item_value("name={VALUE}", &vars), "name=hello");
        assert_eq!(substitute_item_value("name:={VALUE}", &vars), "name:=hello");
        assert_eq!(substitute_item_value("q=={VALUE}", &vars), "q==hello");
        assert_eq!(
            substitute_item_value("Header:{VALUE}", &vars),
            "Header:hello"
        );
        assert_eq!(
            substitute_item_value("data=@{PATH}", &vars),
            "data=@fixtures/payload.json"
        );
        assert_eq!(
            substitute_item_value("payload:=@{PATH}", &vars),
            "payload:=@fixtures/payload.json"
        );
        assert_eq!(
            substitute_item_value("spec@{PATH};type={CTYPE}", &vars),
            "spec@fixtures/payload.json;type=application/json"
        );
    }

    #[test]
    fn append_resume_range_header_only_when_partial_file_exists() {
        let dir = tempdir().expect("tempdir should be created");
        let download_path = dir.path().join("partial.bin");
        fs::write(&download_path, b"abcdef").expect("partial file should be written");

        let mut args = parse_args(&["zapreq", "GET", "https://example.com", "--download"]);
        args.continue_download = true;
        args.output = Some(download_path.to_string_lossy().into_owned());

        let mut items = Vec::new();
        append_resume_range_header(&args, &mut items);
        assert_eq!(items, vec!["Range:bytes=6-".to_string()]);

        let mut zero_args = parse_args(&["zapreq", "GET", "https://example.com", "--download"]);
        zero_args.continue_download = true;
        zero_args.output = Some(
            dir.path()
                .join("missing.bin")
                .to_string_lossy()
                .into_owned(),
        );
        let mut zero_items = Vec::new();
        append_resume_range_header(&zero_args, &mut zero_items);
        assert!(zero_items.is_empty());
    }

    #[test]
    fn maybe_render_test_report_returns_pass_and_fail_exit_codes() {
        let trace = RequestTrace {
            method: "GET".to_string(),
            url: "https://example.com/users".to_string(),
            headers: Vec::new(),
            body_preview: None,
        };
        let ok_response = ResponseData {
            status_code: 200,
            reason: "OK".to_string(),
            final_url: trace.url.clone(),
            headers: Vec::new(),
            content_type: Some("application/json".to_string()),
            body: br#"{"ok":true}"#.to_vec(),
        };
        let fail_response = ResponseData {
            status_code: 500,
            reason: "Server Error".to_string(),
            final_url: trace.url.clone(),
            headers: Vec::new(),
            content_type: Some("application/json".to_string()),
            body: br#"{"ok":false}"#.to_vec(),
        };
        let test_opts = TestOptions {
            expect_status: Some(200),
            expect_headers: Vec::new(),
            expect_json: Vec::new(),
            expect_body_contains: Vec::new(),
            max_time_ms: Some(500),
        };

        let ok_code = maybe_render_test_report(
            Some((test_opts.clone(), "json".to_string())),
            &trace,
            &ok_response,
            120,
        )
        .expect("report rendering should succeed");
        let fail_code = maybe_render_test_report(
            Some((test_opts, "text".to_string())),
            &trace,
            &fail_response,
            120,
        )
        .expect("report rendering should succeed");

        assert_eq!(ok_code, Some(0));
        assert_eq!(fail_code, Some(1));
    }

    #[test]
    fn final_status_code_and_ssl_label_cover_edge_cases() {
        let mut args = parse_args(&["zapreq", "GET", "https://example.com"]);
        args.check_status = true;
        assert_eq!(final_status_code(&args, 503), 1);
        assert_eq!(final_status_code(&args, 200), 0);
        assert_eq!(infer_ssl_label("http://example.com", None), "none");
        assert_eq!(
            infer_ssl_label("https://example.com", Some("tls1.3")),
            "TLS1.3"
        );
        assert_eq!(infer_ssl_label("https://example.com", None), "TLS(auto)");
    }
}
