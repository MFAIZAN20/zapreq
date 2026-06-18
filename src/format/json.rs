use colored::Colorize;
use serde_json::Value;

use crate::output::theme::Theme;

/// CAUS-OUTPUT-12:
/// Formats JSON values with recursive pretty printing and syntax colors.
pub fn format_json(value: &Value, theme: &Theme, indent: usize, truncate: bool) -> String {
    let prepared = if truncate {
        truncate_value(value, 200)
    } else {
        value.clone()
    };

    if !atty::is(atty::Stream::Stdout) {
        return serde_json::to_string_pretty(&prepared).unwrap_or_else(|_| prepared.to_string());
    }

    let step = if indent == 0 { 4 } else { indent };
    render(&prepared, theme, 0, step, false)
}

fn render(value: &Value, theme: &Theme, level: usize, step: usize, truncate: bool) -> String {
    match value {
        Value::Null => paint("null", theme.json_null, theme.use_color),
        Value::Bool(v) => paint(&v.to_string(), theme.json_bool, theme.use_color),
        Value::Number(v) => paint(&v.to_string(), theme.json_number, theme.use_color),
        Value::String(v) => {
            let shown = if truncate && v.chars().count() > 200 {
                format!("{}…", v.chars().take(200).collect::<String>())
            } else {
                v.clone()
            };
            let quoted = serde_json::to_string(&shown).unwrap_or_else(|_| format!("\"{shown}\""));
            paint(&quoted, theme.json_string, theme.use_color)
        }
        Value::Array(arr) => render_array(arr, theme, level, step, truncate),
        Value::Object(map) => {
            if map.is_empty() {
                return format!(
                    "{}{}",
                    paint("{", theme.json_brace, theme.use_color),
                    paint("}", theme.json_brace, theme.use_color)
                );
            }

            let mut out = String::new();
            out.push_str(&paint("{", theme.json_brace, theme.use_color));
            out.push('\n');

            let mut iter = map.iter().peekable();
            while let Some((k, v)) = iter.next() {
                out.push_str(&" ".repeat((level + 1) * step));
                let key = serde_json::to_string(k).unwrap_or_else(|_| format!("\"{k}\""));
                out.push_str(&paint(&key, theme.json_key, theme.use_color));
                out.push_str(&paint(": ", theme.json_brace, theme.use_color));
                out.push_str(&render(v, theme, level + 1, step, truncate));
                if iter.peek().is_some() {
                    out.push_str(&paint(",", theme.json_brace, theme.use_color));
                }
                out.push('\n');
            }

            out.push_str(&" ".repeat(level * step));
            out.push_str(&paint("}", theme.json_brace, theme.use_color));
            out
        }
    }
}

fn render_array(
    values: &[Value],
    theme: &Theme,
    level: usize,
    step: usize,
    truncate: bool,
) -> String {
    if values.is_empty() {
        return format!(
            "{}{}",
            paint("[", theme.json_brace, theme.use_color),
            paint("]", theme.json_brace, theme.use_color)
        );
    }

    let mut out = String::new();
    out.push_str(&paint("[", theme.json_brace, theme.use_color));
    out.push('\n');
    let mut iter = values.iter().peekable();
    while let Some(v) = iter.next() {
        out.push_str(&" ".repeat((level + 1) * step));
        out.push_str(&render(v, theme, level + 1, step, truncate));
        if iter.peek().is_some() {
            out.push_str(&paint(",", theme.json_brace, theme.use_color));
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(level * step));
    out.push_str(&paint("]", theme.json_brace, theme.use_color));
    out
}

fn paint(value: &str, color: colored::Color, use_color: bool) -> String {
    if use_color {
        value.color(color).to_string()
    } else {
        value.to_string()
    }
}

fn truncate_value(value: &Value, limit: usize) -> Value {
    match value {
        Value::String(s) => {
            if s.chars().count() > limit {
                Value::String(format!("{}…", s.chars().take(limit).collect::<String>()))
            } else {
                Value::String(s.clone())
            }
        }
        Value::Array(arr) => Value::Array(arr.iter().map(|v| truncate_value(v, limit)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), truncate_value(v, limit)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_json;
    use crate::output::theme::no_color;
    use serde_json::json;

    #[test]
    fn empty_object() {
        let out = format_json(&json!({}), &no_color(), 4, false);
        assert!(out.contains("{}") || out.contains("{\n}"));
    }

    #[test]
    fn nested_object() {
        let out = format_json(&json!({"a":{"b":1}}), &no_color(), 4, false);
        assert!(out.contains("\"a\""));
        assert!(out.contains("\"b\""));
    }

    #[test]
    fn array_of_strings() {
        let out = format_json(&json!(["x", "y"]), &no_color(), 4, false);
        assert!(out.contains("\"x\""));
        assert!(out.contains("\"y\""));
    }

    #[test]
    fn mixed_types() {
        let out = format_json(&json!({"n":1,"b":true,"s":"x"}), &no_color(), 4, false);
        assert!(out.contains("true"));
        assert!(out.contains("1"));
    }

    #[test]
    fn null_value() {
        let out = format_json(&json!({"x":null}), &no_color(), 4, false);
        assert!(out.contains("null"));
    }

    #[test]
    fn unicode_preserved() {
        let out = format_json(&json!({"u":"سلام"}), &no_color(), 4, false);
        assert!(out.contains("سلام"));
    }

    #[test]
    fn truncation_at_200() {
        let long = "a".repeat(250);
        let out = format_json(&json!({"x": long}), &no_color(), 4, true);
        assert!(out.contains('…') || out.contains("..."));
    }

    #[test]
    fn non_tty_plain_json() {
        let out = format_json(&json!({"x":1}), &no_color(), 4, false);
        assert!(!out.contains("\u{1b}["));
        assert!(out.contains("\"x\""));
    }
}
