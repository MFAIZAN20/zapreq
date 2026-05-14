use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;
use std::time::Instant;

use zapreq::ai::ai_assist;
use zapreq::auth::{build_auth, AuthRegistry};
use zapreq::cli::{parse_cli_from, CliArgs, Command, PluginCommand};
use zapreq::collections::{
    delete_request, list_requests, load_request, run_request, save_request,
};
use zapreq::config::{apply_profile, load_config, load_profile, merge_defaults, CliResolved};
use zapreq::diff::{diff_requests, print_diff};
use zapreq::download::download;
use zapreq::items::parse_request_items;
use zapreq::output::{build_print_opts, render_exchange_from_cli};
use zapreq::plugins::manager::{install_plugin, print_plugin_list, uninstall_plugin};
use zapreq::request::{RequestEngine, RequestSpec};
use zapreq::response::ResponseData;
use zapreq::sessions::{
    apply_session_to_request, load_session, save_session, update_session_from_exchange,
};
use zapreq::utils::{humanize_bytes, humanize_duration, terminal_width, truncate_str};

/// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-02, CAUS-CORERUNTIM-03, CAUS-CORERUNTIM-04, CAUS-CORERUNTIM-05, CAUS-INTERNAL-52:
/// Main orchestration entrypoint with explicit contract wiring, isolated runtime state transitions, and exit-code handling.
fn run() -> Result<i32> {
    let config = load_config().context("failed to load config")?;
    let mut argv: Vec<String> = std::env::args().collect();
    if !is_raw_subcommand_invocation(&argv) {
        merge_defaults(&config, &mut argv);
    }
    let mut args = parse_cli_from(argv).context("failed to parse CLI args")?;

    if let Some(command) = args.command.clone() {
        match command {
            Command::Plugins { command } => {
                match command {
                    PluginCommand::Install { name } => install_plugin(&name)?,
                    PluginCommand::Uninstall { name } => uninstall_plugin(&name, &config)?,
                    PluginCommand::List => print_plugin_list(&config)?,
                }
                return Ok(0);
            }
            Command::Save { alias, request } => {
                let saved = cli_from_saved_request_tokens(&request, &config)?;
                save_request(&alias, &saved)
                    .with_context(|| format!("failed to save collection '{alias}'"))?;
                println!("Saved request as '{alias}'");
                return Ok(0);
            }
            Command::Run { alias, env_profile } => {
                run_request(&alias, env_profile.as_deref())?;
                let entry = load_request(&alias)
                    .with_context(|| format!("failed to load collection '{alias}'"))?;
                let mut synthetic =
                    vec!["http".to_string(), entry.method.clone(), entry.url.clone()];
                synthetic.extend(entry.items.clone());
                merge_defaults(&config, &mut synthetic);
                args = parse_cli_from(synthetic).context("failed to parse saved request")?;
                if args.env_profile.is_none() {
                    args.env_profile = env_profile;
                }
                for (k, v) in entry.headers {
                    args.request_items.push(format!("{k}:{v}"));
                }
            }
            Command::List => {
                let entries = list_requests().context("failed to list saved requests")?;
                if entries.is_empty() {
                    println!("No saved requests.");
                } else {
                    for e in entries {
                        println!("{}  {} {}", e.alias, e.method, e.url);
                    }
                }
                return Ok(0);
            }
            Command::Delete { alias } => {
                delete_request(&alias)
                    .with_context(|| format!("failed to delete collection '{alias}'"))?;
                println!("Deleted request '{alias}'");
                return Ok(0);
            }
            Command::Ai { prompt } => {
                let api_key = match std::env::var("ZAPREQ_AI_KEY") {
                    Ok(v) if !v.trim().is_empty() => v,
                    _ => {
                        eprintln!("ZAPREQ_AI_KEY is not set. Export it first to use `http ai`.");
                        return Ok(1);
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
                    "http {} {} {}",
                    method,
                    generated.url,
                    generated_items.join(" ")
                );
                println!("Generated command: {command_preview}");

                let mut synthetic = vec!["http".to_string(), method, generated.url];
                synthetic.extend(generated_items);
                merge_defaults(&config, &mut synthetic);
                args = parse_cli_from(synthetic).context("failed to parse AI-generated command")?;
            }
            Command::Diff {
                url_a,
                url_b,
                request,
            } => {
                let mut diff_cli = if request.is_empty() {
                    args.clone()
                } else {
                    cli_from_diff_tokens(&url_a, &request, &config)?
                };
                diff_cli.command = None;
                let result =
                    diff_requests(&url_a, &url_b, &diff_cli).context("diff command failed")?;
                let opts = build_print_opts(&diff_cli, &config);
                print_diff(&result, &opts.theme);
                return Ok(0);
            }
        }
    }

    if args.url.is_empty() {
        return Err(anyhow!("URL is required unless using plugin subcommands"));
    }

    let env_map = if let Some(path) = args.env_file.as_deref() {
        load_env_file(path).with_context(|| format!("failed to load env file: {path}"))?
    } else {
        HashMap::new()
    };

    let mut resolved = CliResolved {
        url: args.url.clone(),
        request_items: args.request_items.clone(),
        profile_headers: HashMap::new(),
        variables: env_map,
    };
    if let Some(profile_name) = args.env_profile.as_deref() {
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
    for (k, v) in &resolved.profile_headers {
        resolved_items.push(format!(
            "{}:{}",
            substitute_placeholders(k, &resolved.variables),
            substitute_placeholders(v, &resolved.variables)
        ));
    }
    if args.download && args.continue_download {
        if let Some(output_path) = args.output.as_deref() {
            if let Ok(meta) = std::fs::metadata(output_path) {
                let existing = meta.len();
                if existing > 0 {
                    resolved_items.push(format!("Range:bytes={existing}-"));
                }
            }
        }
    }

    let usable_url = zapreq::utils::normalize_url(&resolved_url, &args.default_scheme)
        .context("failed to build usable URL")?;

    let mut request_items =
        parse_request_items(&resolved_items).context("failed to parse REQUEST_ITEMS")?;

    let loaded_session =
        load_session(&usable_url, args.session.as_deref()).context("failed to load session")?;

    if let Some((_, session_data)) = &loaded_session {
        if args.verbose {
            eprintln!(
                "[session: loaded {} cookies, {} headers]",
                session_data.cookies.len(),
                session_data.headers.len()
            );
        }

        apply_session_to_request(
            &mut request_items,
            &mut args.auth_type,
            &mut args.auth,
            session_data,
        );
    }

    let registry = AuthRegistry::with_defaults();
    if args.auth.is_none() && !args.auth_type.eq_ignore_ascii_case("basic") {
        eprintln!(
            "warning: --auth-type={} provided without --auth; request sent without credentials",
            args.auth_type
        );
    }
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

    if args.offline {
        let prepared = engine
            .prepare(&args, &spec, auth_plugin.as_deref())
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
        return Ok(0);
    }

    if args.download {
        let started = Instant::now();
        let (trace, response) = engine
            .send_raw_for_download(&args, &spec, auth_plugin.as_deref())
            .context("download request failed")?;

        let download_result =
            download(response, &args, &print_opts.theme).context("download failed")?;

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
        return Ok(0);
    }

    let started = Instant::now();
    let (trace, response) = engine
        .send(&args, &spec, auth_plugin.as_deref())
        .context("request execution failed")?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if let Some((session_path, mut session_data)) = loaded_session {
        if !args.session_read_only {
            update_session_from_exchange(
                &mut session_data,
                &request_items,
                &args.auth_type,
                args.auth.as_deref(),
                &response,
            );
            save_session(&session_path, &session_data).context("failed to save session")?;
        }
    }

    render_exchange_from_cli(&trace, &response, &args, &config)
        .context("failed to render output")?;
    if args.verbose {
        if let Some(auth) = args.auth.as_deref() {
            eprintln!("Auth: {}", mask_auth(&args.auth_type, auth));
        }
    }

    if args.meta {
        print_meta_summary(
            &trace.method,
            &trace.url,
            response.status_code,
            &response.reason,
            elapsed_ms,
            &response,
            infer_ssl_label(&trace.url, args.ssl.as_deref()),
        );
    }

    if args.check_status && response.status_code >= 400 {
        return Ok(1);
    }

    Ok(0)
}

fn is_raw_subcommand_invocation(argv: &[String]) -> bool {
    matches!(
        argv.get(1).map(String::as_str),
        Some("plugins" | "save" | "run" | "list" | "delete" | "ai" | "diff")
    )
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
        out.insert(key.to_string(), v.trim().to_string());
    }

    Ok(out)
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").expect("regex should compile");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        vars.get(key)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    })
    .to_string()
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

fn cli_from_saved_request_tokens(
    request: &[String],
    config: &zapreq::config::Config,
) -> Result<CliArgs> {
    if request.is_empty() {
        return Err(anyhow!(
            "save requires request tokens: use `http save <alias> -- METHOD URL [ITEMS...]`"
        ));
    }
    let mut tokens = request.to_vec();
    if tokens.first().map(|t| t.as_str()) == Some("--") {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return Err(anyhow!("no request tokens supplied after `--`"));
    }

    let mut argv = vec!["http".to_string()];
    argv.extend(tokens);
    merge_defaults(config, &mut argv);
    let parsed = parse_cli_from(argv).context("failed to parse request tokens for save")?;
    if parsed.command.is_some() {
        return Err(anyhow!("nested subcommands are not allowed in `save`"));
    }
    Ok(parsed)
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

    let mut argv = vec!["http".to_string()];
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
