use anyhow::Result;
use colored::control;
use reqwest::header::HeaderMap;
use serde_json::Value;

use crate::cli::{CliArgs, PrettyMode as CliPrettyMode, StyleTheme};
use crate::config::Config;
use crate::format::headers::{format_header_line, format_request_line, format_status_line};
use crate::format::{html, json, xml};
use crate::output::theme::{detect_theme, get_theme, no_color, Theme};
use crate::utils::is_binary;

/// Output print behavior resolved from CLI and config values.
#[derive(Clone)]
pub struct PrintOpts {
    pub request_headers: bool,
    pub request_body: bool,
    pub response_headers: bool,
    pub response_body: bool,
    pub pretty: PrettyMode,
    pub theme: Theme,
    pub stream: bool,
    pub truncate: bool,
    pub show_secrets: bool,
}

/// Pretty-print behavior.
#[derive(Clone)]
pub enum PrettyMode {
    All,
    Colors,
    Format,
    None,
}

/// Parses --print flag characters to section booleans.
pub fn parse_print_flag(flag: &str) -> (bool, bool, bool, bool) {
    let effective = if flag.trim().is_empty() { "hb" } else { flag };
    (
        effective.contains('H'),
        effective.contains('B'),
        effective.contains('h'),
        effective.contains('b'),
    )
}

/// Builds print options from CLI and config precedence.
pub fn build_print_opts(cli: &CliArgs, config: &Config) -> PrintOpts {
    let pretty = match cli.pretty.as_ref() {
        Some(CliPrettyMode::All) => PrettyMode::All,
        Some(CliPrettyMode::Colors) => PrettyMode::Colors,
        Some(CliPrettyMode::Format) => PrettyMode::Format,
        Some(CliPrettyMode::None) => PrettyMode::None,
        None => match config.pretty.trim().to_ascii_lowercase().as_str() {
            "colors" => PrettyMode::Colors,
            "format" => PrettyMode::Format,
            "none" => PrettyMode::None,
            _ => PrettyMode::All,
        },
    };

    let mut theme = if let Some(style) = cli.style.as_ref() {
        match style {
            StyleTheme::Monokai => get_theme("monokai"),
            StyleTheme::Solarized => get_theme("solarized"),
            StyleTheme::Dracula => get_theme("dracula"),
            StyleTheme::Autumn => get_theme("autumn"),
        }
    } else {
        detect_theme(config)
    };

    if matches!(pretty, PrettyMode::None) || !atty::is(atty::Stream::Stdout) {
        control::set_override(false);
        theme = no_color();
    } else {
        control::unset_override();
    }

    let (mut req_h, mut req_b, mut res_h, mut res_b) =
        parse_print_flag(cli.print.as_deref().unwrap_or("hb"));
    if cli.verbose {
        req_h = true;
        req_b = true;
        res_h = true;
        res_b = true;
    } else if cli.headers {
        req_h = false;
        req_b = false;
        res_h = true;
        res_b = false;
    } else if cli.body {
        req_h = false;
        req_b = false;
        res_h = false;
        res_b = true;
    }

    PrintOpts {
        request_headers: req_h,
        request_body: req_b,
        response_headers: res_h,
        response_body: res_b,
        pretty,
        theme,
        stream: cli.stream,
        truncate: true,
        show_secrets: cli.show_secrets,
    }
}

/// Prints request sections according to PrintOpts.
pub fn print_request(
    method: &str,
    url: &str,
    headers: &HeaderMap,
    body: Option<&Value>,
    opts: &PrintOpts,
) {
    let _ = opts.theme.meta_border;
    if opts.request_headers {
        let path = request_path(url);
        println!(
            "{}",
            format_request_line(method, &path, "HTTP/1.1", &opts.theme)
        );
        for (name, value) in headers {
            let value = value.to_str().unwrap_or("<non-utf8>");
            let value_str = if opts.show_secrets {
                value.to_string()
            } else {
                crate::headers::mask_header_value(name.as_str(), value)
            };
            println!(
                "{}",
                format_header_line(name.as_str(), &value_str, &opts.theme)
            );
        }
        if opts.request_body {
            println!();
        }
    }

    if opts.request_body {
        if let Some(payload) = body {
            if matches!(opts.pretty, PrettyMode::None) {
                println!("{payload}");
            } else {
                println!(
                    "{}",
                    json::format_json(payload, &opts.theme, 4, opts.truncate)
                );
            }
        }
    }
}

/// Prints response sections according to PrintOpts.
pub fn print_response(
    status: u16,
    reason: &str,
    headers: &HeaderMap,
    body_bytes: &[u8],
    content_type: &str,
    opts: &PrintOpts,
) {
    let _ = opts.stream;
    if opts.response_headers {
        println!("{}", format_status_line(status, reason, &opts.theme));
        for (name, value) in headers {
            let value = value.to_str().unwrap_or("<non-utf8>");
            let value_str = if opts.show_secrets {
                value.to_string()
            } else {
                crate::headers::mask_header_value(name.as_str(), value)
            };
            println!(
                "{}",
                format_header_line(name.as_str(), &value_str, &opts.theme)
            );
        }
        if opts.response_body {
            println!();
        }
    }

    if !opts.response_body {
        return;
    }

    if is_binary(body_bytes) {
        println!("[binary body, {} bytes — use --download]", body_bytes.len());
        return;
    }

    let ct = content_type.to_ascii_lowercase();
    if ct.contains("application/json") {
        if let Ok(value) = serde_json::from_slice::<Value>(body_bytes) {
            println!(
                "{}",
                json::format_json(&value, &opts.theme, 4, opts.truncate)
            );
        } else {
            println!("{}", String::from_utf8_lossy(body_bytes));
        }
    } else if ct.contains("text/xml") || ct.contains("application/xml") {
        println!(
            "{}",
            xml::format_xml(&String::from_utf8_lossy(body_bytes), &opts.theme)
        );
    } else if ct.contains("text/html") {
        println!(
            "{}",
            html::format_html(&String::from_utf8_lossy(body_bytes))
        );
    } else {
        println!("{}", String::from_utf8_lossy(body_bytes));
    }
}

fn request_path(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        let mut out = parsed.path().to_string();
        if let Some(q) = parsed.query() {
            out.push('?');
            out.push_str(q);
        }
        if out.is_empty() {
            "/".to_string()
        } else {
            out
        }
    } else {
        url.to_string()
    }
}

#[allow(dead_code)]
pub fn _ensure_result_usage() -> Result<()> {
    Ok(())
}
