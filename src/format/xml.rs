use colored::Colorize;
use regex::Regex;
use std::io::Cursor;
use xmltree::EmitterConfig;

use crate::output::theme::Theme;

/// CAUS-OUTPUT-15:
/// Formats XML with indentation and optional syntax coloring.
pub fn format_xml(raw: &str, theme: &Theme) -> String {
    if raw.trim().is_empty() {
        return raw.to_string();
    }

    let element = match xmltree::Element::parse(Cursor::new(raw.as_bytes())) {
        Ok(el) => el,
        Err(_) => return raw.to_string(),
    };

    let mut out = Vec::new();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("  ")
        .write_document_declaration(false);
    if element.write_with_config(&mut out, config).is_err() {
        return raw.to_string();
    }

    let formatted = String::from_utf8(out).unwrap_or_else(|_| raw.to_string());
    if !theme.use_color {
        return formatted;
    }
    if !atty::is(atty::Stream::Stdout) {
        return formatted;
    }

    colorize_xml(&formatted, theme)
}

fn colorize_xml(input: &str, theme: &Theme) -> String {
    let tag_re = Regex::new(r"<(/?)([A-Za-z_][\w:.-]*)([^>]*)>").expect("tag regex should compile");
    let attr_re = Regex::new(r#"([A-Za-z_][\w:.-]*)\s*=\s*("[^"]*"|'[^']*')"#)
        .expect("attr regex should compile");

    let mut out = String::new();
    for line in input.lines() {
        let mut rendered = line.to_string();
        rendered = tag_re
            .replace_all(&rendered, |caps: &regex::Captures<'_>| {
                let slash = &caps[1];
                let name = &caps[2];
                let attrs = &caps[3];
                let attrs_colored = attr_re
                    .replace_all(attrs, |acaps: &regex::Captures<'_>| {
                        format!(
                            "{}={}",
                            acaps[1].color(theme.header_name),
                            acaps[2].color(theme.json_string)
                        )
                    })
                    .to_string();
                format!("<{}{}{}>", slash, name.color(theme.json_key), attrs_colored)
            })
            .to_string();

        if !line.contains('<') || !line.contains('>') {
            rendered = rendered.color(theme.json_string).to_string();
        }
        out.push_str(&rendered);
        out.push('\n');
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::format_xml;
    use crate::output::theme::no_color;

    #[test]
    fn valid_xml_formatted() {
        let raw = "<root><a x=\"1\">v</a></root>";
        let out = format_xml(raw, &no_color());
        assert!(out.contains("<root>"));
        assert!(out.contains("<a"));
        assert!(out.contains('\n'));
    }

    #[test]
    fn malformed_xml_passthrough() {
        let raw = "<root><a></root>";
        let out = format_xml(raw, &no_color());
        assert_eq!(out, raw);
    }

    #[test]
    fn empty_xml_document() {
        let raw = "";
        let out = format_xml(raw, &no_color());
        assert_eq!(out, raw);
    }
}
