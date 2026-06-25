use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderSource {
    User,
    Auto,
    Preset,
    Environment,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    pub sensitive: bool,
    pub source: HeaderSource,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderWarning {
    pub name: Option<String>,
    pub message: String,
    pub severity: HeaderValidationSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderSuggestion {
    pub name: String,
    pub description: String,
    pub common_values: Vec<String>,
    pub sensitive_by_default: bool,
}

const HEADER_SUGGESTION_DATA: &[(&str, &str, &[&str], bool)] = &[
    (
        "Authorization",
        "Credentials for bearer, basic, and API token flows.",
        &["Bearer <token>", "Basic <base64>", "ApiKey <key>"],
        true,
    ),
    (
        "Content-Type",
        "Declares the request body media type.",
        &[
            "application/json",
            "application/x-www-form-urlencoded",
            "multipart/form-data",
            "text/plain",
            "application/xml",
        ],
        false,
    ),
    (
        "Accept",
        "Declares which response media types the client accepts.",
        &["application/json", "*/*", "text/plain", "application/xml"],
        false,
    ),
    (
        "User-Agent",
        "Identifies the client application to the server.",
        &["zapreq/<version>"],
        false,
    ),
    (
        "X-API-Key",
        "Carries an API key when bearer auth is not used.",
        &["<api-key>"],
        true,
    ),
    (
        "X-Request-ID",
        "Correlates a client request across services.",
        &["<request-id>"],
        false,
    ),
    (
        "X-Correlation-ID",
        "Correlates a workflow or trace across multiple requests.",
        &["<correlation-id>"],
        false,
    ),
    (
        "Idempotency-Key",
        "Prevents duplicate mutations for retryable writes.",
        &["<uuid>"],
        false,
    ),
    (
        "Cache-Control",
        "Controls cache behavior for requests and responses.",
        &["no-cache", "no-store", "max-age=0"],
        false,
    ),
    (
        "If-None-Match",
        "Sends an ETag for cache revalidation.",
        &["W/\"etag-value\"", "\"etag-value\""],
        false,
    ),
    (
        "If-Modified-Since",
        "Sends a timestamp for cache revalidation.",
        &["Wed, 21 Oct 2015 07:28:00 GMT"],
        false,
    ),
    (
        "Origin",
        "Identifies the browser origin for CORS requests.",
        &["https://app.example.com"],
        false,
    ),
    (
        "Referer",
        "Identifies the previous page or request source.",
        &["https://app.example.com/page"],
        false,
    ),
    (
        "Cookie",
        "Carries stateful cookie values to the server.",
        &["session=<token>"],
        true,
    ),
    (
        "Accept-Encoding",
        "Negotiates supported compression formats.",
        &["gzip, br, deflate"],
        false,
    ),
    (
        "Accept-Language",
        "Negotiates preferred response languages.",
        &["en-US,en;q=0.9"],
        false,
    ),
];

pub fn header_suggestions() -> Vec<HeaderSuggestion> {
    HEADER_SUGGESTION_DATA
        .iter()
        .map(|(name, description, values, sensitive)| HeaderSuggestion {
            name: (*name).to_string(),
            description: (*description).to_string(),
            common_values: values.iter().map(|value| (*value).to_string()).collect(),
            sensitive_by_default: *sensitive,
        })
        .collect()
}

pub fn common_header_names() -> Vec<String> {
    header_suggestions()
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

/// Dynamic value suggestions based on selected header key.
pub fn get_value_suggestions(name: &str) -> Vec<String> {
    header_suggestions()
        .into_iter()
        .find(|suggestion| suggestion.name.eq_ignore_ascii_case(name))
        .map(|suggestion| suggestion.common_values)
        .unwrap_or_default()
}

/// Checks if a header key is sensitive.
pub fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.trim(),
        "authorization"
            | "proxy-authorization"
            | "x-api-key"
            | "api-key"
            | "cookie"
            | "set-cookie"
            | "x-auth-token"
            | "x-csrf-token"
    )
}

/// Mask sensitive values in headers safely.
pub fn mask_header_value(name: &str, value: &str) -> String {
    if !is_sensitive_header(name) {
        return value.to_string();
    }
    if value.is_empty() {
        return String::new();
    }

    if let Some((scheme, rest)) = value.split_once(' ') {
        if !rest.is_empty() {
            return format!("{} {}", scheme, mask_raw_value(rest));
        }
    }

    if let Some((k, v)) = value.split_once('=') {
        if !v.is_empty() {
            return format!("{}={}", k, mask_raw_value(v));
        }
    }

    mask_raw_value(value)
}

fn mask_raw_value(value: &str) -> String {
    if value.len() <= 5 {
        "****".to_string()
    } else {
        format!("{}...****", &value[..5])
    }
}

pub fn headers_from_parsed_items(
    parsed_items: &[crate::items::RequestItem],
    source: HeaderSource,
) -> Vec<Header> {
    let mut headers = Vec::new();
    for item in parsed_items {
        if let crate::items::RequestItem::Header { key, value } = item {
            headers.push(Header {
                name: key.clone(),
                value: value.clone(),
                enabled: true,
                sensitive: is_sensitive_header(key),
                source: source.clone(),
            });
        }
    }
    headers
}

pub fn headers_from_curl_headers(curl_headers: &[String], source: HeaderSource) -> Vec<Header> {
    let mut headers = Vec::new();
    for raw in curl_headers {
        if let Some((key, value)) = raw.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            headers.push(Header {
                name: key.to_string(),
                value: value.to_string(),
                enabled: true,
                sensitive: is_sensitive_header(key),
                source: source.clone(),
            });
        }
    }
    headers
}

pub fn header_items(headers: &[Header]) -> Vec<String> {
    headers
        .iter()
        .filter(|header| header.enabled && !header.name.trim().is_empty())
        .map(|header| format!("{}:{}", header.name, header.value))
        .collect()
}

/// Auto headers generation for missing defaults.
pub fn get_auto_headers(body_type: &str) -> Vec<Header> {
    let mut auto = Vec::new();
    let version = env!("CARGO_PKG_VERSION");

    auto.push(Header {
        name: "User-Agent".to_string(),
        value: format!("zapreq/{}", version),
        enabled: true,
        sensitive: false,
        source: HeaderSource::Auto,
    });

    match body_type {
        "json" => {
            auto.push(Header {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
            auto.push(Header {
                name: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
        }
        "form" => {
            auto.push(Header {
                name: "Content-Type".to_string(),
                value: "application/x-www-form-urlencoded".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
            auto.push(Header {
                name: "Accept".to_string(),
                value: "*/*".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
        }
        "multipart" => {
            // Note: let the client library set the exact boundary, but we still define the MIME type
            auto.push(Header {
                name: "Content-Type".to_string(),
                value: "multipart/form-data".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
            auto.push(Header {
                name: "Accept".to_string(),
                value: "*/*".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
        }
        _ => {
            auto.push(Header {
                name: "Accept".to_string(),
                value: "*/*".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Auto,
            });
        }
    }

    auto
}

/// Merge order implementation: Preset -> Environment -> User -> Auto (when missing).
pub fn merge_headers(
    presets: &[Header],
    environments: &[Header],
    users: &[Header],
    autos: &[Header],
) -> Vec<Header> {
    let user_keys: HashSet<String> = users
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.name.to_ascii_lowercase())
        .collect();

    let env_keys: HashSet<String> = environments
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.name.to_ascii_lowercase())
        .collect();

    let preset_keys: HashSet<String> = presets
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.name.to_ascii_lowercase())
        .collect();

    let mut merged = Vec::new();

    // 1. Preset headers (if not overridden by User or Env)
    for h in presets {
        let key = h.name.to_ascii_lowercase();
        if !user_keys.contains(&key) && !env_keys.contains(&key) {
            let mut header = h.clone();
            header.source = HeaderSource::Preset;
            merged.push(header);
        }
    }

    // 2. Env headers (if not overridden by User)
    for h in environments {
        let key = h.name.to_ascii_lowercase();
        if !user_keys.contains(&key) {
            let mut header = h.clone();
            header.source = HeaderSource::Environment;
            merged.push(header);
        }
    }

    // 3. User headers
    for h in users {
        let mut header = h.clone();
        header.source = HeaderSource::User;
        merged.push(header);
    }

    // 4. Auto headers for missing defaults
    for h in autos {
        let key = h.name.to_ascii_lowercase();
        if !user_keys.contains(&key) && !env_keys.contains(&key) && !preset_keys.contains(&key) {
            let mut header = h.clone();
            header.source = HeaderSource::Auto;
            merged.push(header);
        }
    }

    merged
}

/// Validates list of headers and returns warnings/errors.
pub fn validate_headers(
    headers: &[Header],
    body_type: &str,
    body_content: Option<&str>,
    is_unencrypted: bool,
) -> Vec<HeaderWarning> {
    let mut warnings = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();

    // RFC 7230 token character check
    let token_re =
        Regex::new(r"^[A-Za-z0-9!#\$%&'\*\+\-\.\^_`\|~]+$").expect("regex should compile");

    for h in headers {
        if !h.enabled {
            continue;
        }

        let name_trimmed = h.name.trim();
        let key = name_trimmed.to_ascii_lowercase();
        *counts.entry(key.clone()).or_insert(0) += 1;

        // Empty header name
        if name_trimmed.is_empty() {
            warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: "Header name cannot be empty.".to_string(),
                severity: HeaderValidationSeverity::Error,
            });
            continue;
        }

        // Invalid characters in name
        if !token_re.is_match(name_trimmed) {
            warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: format!(
                    "Header name '{}' contains invalid characters.",
                    name_trimmed
                ),
                severity: HeaderValidationSeverity::Error,
            });
        }

        // Empty value check (unless allowed)
        if h.value.trim().is_empty() {
            warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: format!("Header '{}' is defined without a value.", h.name),
                severity: HeaderValidationSeverity::Warning,
            });
        }

        // Typo suggestions
        match key.as_str() {
            "contenttype" => warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: "Did you mean 'Content-Type'?".to_string(),
                severity: HeaderValidationSeverity::Warning,
            }),
            "authorisation" | "authentication" => warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: "Did you mean 'Authorization'?".to_string(),
                severity: HeaderValidationSeverity::Warning,
            }),
            "content-length" => warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: "Manually setting Content-Length is not recommended; let the HTTP client handle it.".to_string(),
                severity: HeaderValidationSeverity::Warning,
            }),
            _ => {}
        }

        // Unencrypted HTTP warning for sensitive secrets
        if is_unencrypted && is_sensitive_header(&h.name) {
            warnings.push(HeaderWarning {
                name: Some(h.name.clone()),
                message: format!(
                    "Sensitive header '{}' is being sent over unencrypted HTTP.",
                    h.name
                ),
                severity: HeaderValidationSeverity::Warning,
            });
        }
    }

    // Duplicate warnings
    for (key, count) in &counts {
        if *count > 1 {
            let orig_name = headers
                .iter()
                .find(|h| h.name.to_ascii_lowercase() == *key)
                .map(|h| h.name.as_str())
                .unwrap_or(key);
            match key.as_str() {
                "authorization" | "content-type" | "host" | "content-length" => {
                    warnings.push(HeaderWarning {
                        name: Some(orig_name.to_string()),
                        message: format!("Problematic duplicate header '{}' detected.", orig_name),
                        severity: HeaderValidationSeverity::Warning,
                    });
                }
                _ => {
                    warnings.push(HeaderWarning {
                        name: Some(orig_name.to_string()),
                        message: format!("Duplicate header '{}' detected.", orig_name),
                        severity: HeaderValidationSeverity::Info,
                    });
                }
            }
        }
    }

    // JSON body check
    let has_json_ct = headers.iter().any(|h| {
        h.enabled
            && h.name.eq_ignore_ascii_case("content-type")
            && h.value.to_ascii_lowercase().contains("application/json")
    });

    if body_type == "json" && !has_json_ct {
        warnings.push(HeaderWarning {
            name: None,
            message:
                "JSON body fields are present but Content-Type is not set to application/json."
                    .to_string(),
            severity: HeaderValidationSeverity::Warning,
        });
    }

    if has_json_ct {
        if let Some(body) = body_content {
            let body_trimmed = body.trim();
            if !body_trimmed.is_empty()
                && serde_json::from_str::<serde_json::Value>(body_trimmed).is_err()
            {
                warnings.push(HeaderWarning {
                    name: Some("Content-Type".to_string()),
                    message: "Content-Type is application/json but body content is not valid JSON."
                        .to_string(),
                    severity: HeaderValidationSeverity::Error,
                });
            }
        }
    }

    warnings
}

/// Builds request headers using the full CLI defaults, curl headers, presets, profile, and body content.
pub fn build_headers_from_cli(
    args: &crate::cli::CliArgs,
    parsed_items: &[crate::items::RequestItem],
    env_headers: &std::collections::HashMap<String, String>,
) -> anyhow::Result<Vec<Header>> {
    let mut preset_headers = Vec::new();
    for p in &args.preset {
        if let Ok(loaded) = crate::header_presets::load_preset(p) {
            preset_headers.extend(loaded);
        }
    }

    let mut environments = Vec::new();
    for (k, v) in env_headers {
        let sensitive = is_sensitive_header(k);
        environments.push(Header {
            name: k.clone(),
            value: v.clone(),
            enabled: true,
            sensitive,
            source: HeaderSource::Environment,
        });
    }

    let mut users = headers_from_parsed_items(parsed_items, HeaderSource::User);
    users.extend(headers_from_curl_headers(
        &args.curl_headers,
        HeaderSource::User,
    ));

    let collected = crate::items::collect_from_parsed(parsed_items)?;
    let has_file_uploads = !collected.files.is_empty();
    let body_type = if args.multipart || has_file_uploads {
        "multipart"
    } else if args.form {
        "form"
    } else if !collected.data_strings.is_empty() || !collected.data_json.is_empty() {
        "json"
    } else {
        "none"
    };

    let autos = get_auto_headers(body_type);

    Ok(merge_headers(
        &preset_headers,
        &environments,
        &users,
        &autos,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_header_value() {
        assert_eq!(
            mask_header_value("Authorization", "Bearer token123456"),
            "Bearer token...****"
        );
        assert_eq!(
            mask_header_value("X-API-Key", "sk_live_12345"),
            "sk_li...****"
        );
        assert_eq!(
            mask_header_value("Cookie", "session=abcdefg"),
            "session=abcde...****"
        );
        assert_eq!(
            mask_header_value("Accept", "application/json"),
            "application/json"
        );
    }

    #[test]
    fn test_header_suggestions_registry() {
        let suggestions = header_suggestions();
        let authorization = suggestions
            .iter()
            .find(|entry| entry.name == "Authorization")
            .expect("authorization suggestion should exist");
        assert!(authorization.sensitive_by_default);
        assert!(authorization
            .common_values
            .contains(&"Bearer <token>".to_string()));
        assert!(common_header_names().contains(&"Content-Type".to_string()));
    }

    #[test]
    fn test_merge_headers() {
        let presets = vec![
            Header {
                name: "X-Tag".to_string(),
                value: "preset".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Preset,
            },
            Header {
                name: "Accept".to_string(),
                value: "text/xml".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::Preset,
            },
        ];
        let environments = vec![Header {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
            enabled: true,
            sensitive: false,
            source: HeaderSource::Environment,
        }];
        let users = vec![Header {
            name: "X-My-Header".to_string(),
            value: "user".to_string(),
            enabled: true,
            sensitive: false,
            source: HeaderSource::User,
        }];
        let autos = vec![Header {
            name: "User-Agent".to_string(),
            value: "ZapReq/1.0".to_string(),
            enabled: true,
            sensitive: false,
            source: HeaderSource::Auto,
        }];

        let merged = merge_headers(&presets, &environments, &users, &autos);
        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].name, "X-Tag"); // Preset
        assert_eq!(merged[1].name, "Accept"); // Environment overrides Preset
        assert_eq!(merged[1].value, "application/json");
        assert_eq!(merged[2].name, "X-My-Header"); // User
        assert_eq!(merged[3].name, "User-Agent"); // Auto
    }

    #[test]
    fn test_validate_headers() {
        let headers = vec![
            Header {
                name: "ContentType".to_string(),
                value: "application/json".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::User,
            },
            Header {
                name: "Authorization".to_string(),
                value: "Bearer token".to_string(),
                enabled: true,
                sensitive: true,
                source: HeaderSource::User,
            },
            Header {
                name: "Authorization".to_string(),
                value: "Bearer token2".to_string(),
                enabled: true,
                sensitive: true,
                source: HeaderSource::User,
            },
        ];

        let warnings = validate_headers(&headers, "none", None, true);
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("Did you mean 'Content-Type'?")));
        assert!(warnings.iter().any(|w| w
            .message
            .contains("Problematic duplicate header 'Authorization' detected.")));
        assert!(warnings.iter().any(|w| w
            .message
            .contains("Sensitive header 'Authorization' is being sent over unencrypted HTTP.")));
    }

    #[test]
    fn test_header_items_preserve_enabled_order() {
        let headers = vec![
            Header {
                name: "X-Trace".to_string(),
                value: "one".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::User,
            },
            Header {
                name: "X-Trace".to_string(),
                value: "two".to_string(),
                enabled: true,
                sensitive: false,
                source: HeaderSource::User,
            },
            Header {
                name: "Disabled".to_string(),
                value: "ignored".to_string(),
                enabled: false,
                sensitive: false,
                source: HeaderSource::User,
            },
        ];

        assert_eq!(
            header_items(&headers),
            vec!["X-Trace:one".to_string(), "X-Trace:two".to_string()]
        );
    }
}
