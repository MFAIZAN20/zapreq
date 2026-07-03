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
    if !opts.request_headers && !opts.request_body {
        return;
    }

    if opts.request_headers {
        print_request_head(method, url, headers, opts);
        if opts.request_body {
            println!();
        }
    }

    if let Some(payload) = body.filter(|_| opts.request_body) {
        print_request_body(payload, opts);
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
    if !opts.response_headers && !opts.response_body {
        return;
    }

    if opts.response_headers {
        print_response_head(status, reason, headers, opts);
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

    print_response_body(body_bytes, content_type, opts);
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

fn print_request_head(method: &str, url: &str, headers: &HeaderMap, opts: &PrintOpts) {
    let path = request_path(url);
    println!(
        "{}",
        format_request_line(method, &path, "HTTP/1.1", &opts.theme)
    );
    print_headers(headers, opts);
}

fn print_response_head(status: u16, reason: &str, headers: &HeaderMap, opts: &PrintOpts) {
    println!("{}", format_status_line(status, reason, &opts.theme));
    print_headers(headers, opts);
}

fn print_headers(headers: &HeaderMap, opts: &PrintOpts) {
    for (name, value) in headers {
        let value = value.to_str().unwrap_or("<non-utf8>");
        println!(
            "{}",
            format_header_line(
                name.as_str(),
                &header_display_value(name.as_str(), value, opts),
                &opts.theme
            )
        );
    }
}

fn header_display_value(name: &str, value: &str, opts: &PrintOpts) -> String {
    if opts.show_secrets {
        value.to_string()
    } else {
        crate::headers::mask_header_value(name, value)
    }
}

fn print_request_body(payload: &Value, opts: &PrintOpts) {
    if matches!(opts.pretty, PrettyMode::None) {
        println!("{payload}");
        return;
    }

    println!(
        "{}",
        json::format_json(payload, &opts.theme, 4, opts.truncate)
    );
}

fn print_response_body(body_bytes: &[u8], content_type: &str, opts: &PrintOpts) {
    let ct = content_type.to_ascii_lowercase();
    if ct.contains("application/json") {
        print_json_or_plain(body_bytes, opts);
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

fn print_json_or_plain(body_bytes: &[u8], opts: &PrintOpts) {
    if let Ok(value) = serde_json::from_slice::<Value>(body_bytes) {
        println!(
            "{}",
            json::format_json(&value, &opts.theme, 4, opts.truncate)
        );
        return;
    }

    println!("{}", String::from_utf8_lossy(body_bytes));
}

#[allow(dead_code)]
pub fn _ensure_result_usage() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{header_display_value, request_path, PrettyMode, PrintOpts};
    use crate::output::theme::no_color;

    fn opts(show_secrets: bool) -> PrintOpts {
        PrintOpts {
            request_headers: true,
            request_body: true,
            response_headers: true,
            response_body: true,
            pretty: PrettyMode::None,
            theme: no_color(),
            stream: false,
            truncate: true,
            show_secrets,
        }
    }

    #[test]
    fn header_display_value_masks_secrets_when_hidden() {
        let rendered = header_display_value("Authorization", "Bearer token123456", &opts(false));
        assert_eq!(rendered, "Bearer token...****");
    }

    #[test]
    fn header_display_value_keeps_plain_values_visible() {
        let rendered = header_display_value("Accept", "application/json", &opts(false));
        assert_eq!(rendered, "application/json");
    }

    #[test]
    fn request_path_keeps_query_string() {
        assert_eq!(
            request_path("https://example.com/api/users?page=2"),
            "/api/users?page=2"
        );
    }
}
