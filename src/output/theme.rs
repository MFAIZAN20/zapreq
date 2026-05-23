use colored::Color;

use crate::config::Config;

/// Theme colors used by formatters and output printing.
#[derive(Clone, Debug)]
pub struct Theme {
    pub json_key: Color,
    pub json_string: Color,
    pub json_number: Color,
    pub json_bool: Color,
    pub json_null: Color,
    pub json_brace: Color,
    pub header_name: Color,
    pub header_value: Color,
    pub status_2xx: Color,
    pub status_3xx: Color,
    pub status_4xx: Color,
    pub status_5xx: Color,
    pub method_get: Color,
    pub method_post: Color,
    pub method_put: Color,
    pub method_delete: Color,
    pub method_patch: Color,
    pub meta_border: Color,
    pub offline_msg: Color,
    pub use_color: bool,
}

/// Monokai-like default theme.
pub fn monokai() -> Theme {
    Theme {
        json_key: Color::BrightBlue,
        json_string: Color::Green,
        json_number: Color::Yellow,
        json_bool: Color::Magenta,
        json_null: Color::Red,
        json_brace: Color::White,
        header_name: Color::Cyan,
        header_value: Color::White,
        status_2xx: Color::Green,
        status_3xx: Color::Yellow,
        status_4xx: Color::Red,
        status_5xx: Color::BrightRed,
        method_get: Color::Green,
        method_post: Color::Yellow,
        method_put: Color::Blue,
        method_delete: Color::Red,
        method_patch: Color::Cyan,
        meta_border: Color::BrightBlack,
        offline_msg: Color::Yellow,
        use_color: true,
    }
}

/// Solarized-style theme.
pub fn solarized() -> Theme {
    Theme {
        json_key: Color::Blue,
        json_string: Color::Cyan,
        json_number: Color::Yellow,
        json_bool: Color::Magenta,
        json_null: Color::Red,
        json_brace: Color::BrightBlack,
        header_name: Color::Blue,
        header_value: Color::White,
        status_2xx: Color::Green,
        status_3xx: Color::Yellow,
        status_4xx: Color::Red,
        status_5xx: Color::BrightRed,
        method_get: Color::Green,
        method_post: Color::Yellow,
        method_put: Color::Blue,
        method_delete: Color::Red,
        method_patch: Color::Cyan,
        meta_border: Color::Blue,
        offline_msg: Color::Yellow,
        use_color: true,
    }
}

/// Dracula-style theme.
pub fn dracula() -> Theme {
    Theme {
        json_key: Color::Magenta,
        json_string: Color::Green,
        json_number: Color::Cyan,
        json_bool: Color::Yellow,
        json_null: Color::Red,
        json_brace: Color::White,
        header_name: Color::Magenta,
        header_value: Color::White,
        status_2xx: Color::Green,
        status_3xx: Color::Yellow,
        status_4xx: Color::Red,
        status_5xx: Color::BrightRed,
        method_get: Color::Green,
        method_post: Color::Yellow,
        method_put: Color::Cyan,
        method_delete: Color::Red,
        method_patch: Color::Magenta,
        meta_border: Color::Magenta,
        offline_msg: Color::Yellow,
        use_color: true,
    }
}

/// Warm autumn palette.
pub fn autumn() -> Theme {
    Theme {
        json_key: Color::Yellow,
        json_string: Color::BrightYellow,
        json_number: Color::Red,
        json_bool: Color::Magenta,
        json_null: Color::BrightRed,
        json_brace: Color::White,
        header_name: Color::Yellow,
        header_value: Color::White,
        status_2xx: Color::Green,
        status_3xx: Color::Yellow,
        status_4xx: Color::Red,
        status_5xx: Color::BrightRed,
        method_get: Color::Green,
        method_post: Color::Yellow,
        method_put: Color::Blue,
        method_delete: Color::Red,
        method_patch: Color::Magenta,
        meta_border: Color::Yellow,
        offline_msg: Color::Yellow,
        use_color: true,
    }
}

/// Colorless fallback theme for non-TTY and --pretty=none.
pub fn no_color() -> Theme {
    Theme {
        json_key: Color::White,
        json_string: Color::White,
        json_number: Color::White,
        json_bool: Color::White,
        json_null: Color::White,
        json_brace: Color::White,
        header_name: Color::White,
        header_value: Color::White,
        status_2xx: Color::White,
        status_3xx: Color::White,
        status_4xx: Color::White,
        status_5xx: Color::White,
        method_get: Color::White,
        method_post: Color::White,
        method_put: Color::White,
        method_delete: Color::White,
        method_patch: Color::White,
        meta_border: Color::White,
        offline_msg: Color::White,
        use_color: false,
    }
}

/// Resolves a theme by name.
pub fn get_theme(name: &str) -> Theme {
    match name.trim().to_ascii_lowercase().as_str() {
        "solarized" => solarized(),
        "dracula" => dracula(),
        "autumn" => autumn(),
        "none" | "no_color" => no_color(),
        _ => monokai(),
    }
}

/// Detects effective theme from config and terminal type.
pub fn detect_theme(config: &Config) -> Theme {
    if !atty::is(atty::Stream::Stdout) {
        return no_color();
    }
    get_theme(&config.output_theme)
}
