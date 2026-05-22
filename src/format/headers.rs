use colored::{Color, Colorize};

use crate::output::theme::Theme;

/// Formats request line (METHOD PATH VERSION) with method coloring.
pub fn format_request_line(method: &str, path: &str, version: &str, theme: &Theme) -> String {
    if !theme.use_color {
        return format!("{method} {path} {version}");
    }
    let method_colored = method.color(method_color(method, theme)).bold().to_string();
    let path_colored = path.white().to_string();
    let version_dim = version.dimmed().to_string();
    format!("{method_colored} {path_colored} {version_dim}")
}

/// Formats HTTP status line with class-based status coloring.
pub fn format_status_line(status: u16, reason: &str, theme: &Theme) -> String {
    if !theme.use_color {
        return format!("{status} {reason}");
    }
    let color = if (200..300).contains(&status) {
        theme.status_2xx
    } else if (300..400).contains(&status) {
        theme.status_3xx
    } else if (400..500).contains(&status) {
        theme.status_4xx
    } else {
        theme.status_5xx
    };

    format!("{} {}", status.to_string().color(color).bold(), reason)
}

/// Formats one header line using theme header colors.
pub fn format_header_line(name: &str, value: &str, theme: &Theme) -> String {
    if !theme.use_color {
        return format!("{name}: {value}");
    }
    format!(
        "{}: {}",
        name.color(theme.header_name).bold(),
        value.color(theme.header_value)
    )
}

/// Resolves method color mapping.
pub fn method_color(method: &str, theme: &Theme) -> Color {
    match method.trim().to_ascii_uppercase().as_str() {
        "GET" => theme.method_get,
        "POST" => theme.method_post,
        "PUT" => theme.method_put,
        "DELETE" => theme.method_delete,
        "PATCH" => theme.method_patch,
        "HEAD" => Color::Magenta,
        "OPTIONS" => Color::White,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::{format_status_line, method_color};
    use crate::output::theme::monokai;
    use colored::Colorize;

    #[test]
    fn status_200_uses_2xx_color() {
        let theme = monokai();
        let out = format_status_line(200, "OK", &theme);
        assert!(out.contains(&"200".color(theme.status_2xx).bold().to_string()));
    }

    #[test]
    fn status_404_uses_4xx_color() {
        let theme = monokai();
        let out = format_status_line(404, "Not Found", &theme);
        assert!(out.contains(&"404".color(theme.status_4xx).bold().to_string()));
    }

    #[test]
    fn status_301_uses_3xx_color() {
        let theme = monokai();
        let out = format_status_line(301, "Moved", &theme);
        assert!(out.contains(&"301".color(theme.status_3xx).bold().to_string()));
    }

    #[test]
    fn method_get_uses_theme_get_color() {
        let theme = monokai();
        assert_eq!(method_color("GET", &theme), theme.method_get);
    }
}
