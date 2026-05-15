use serde::Serialize;
use serde_json::Value;

use crate::response::ResponseData;

/// Assertion options for the `http test` command.
#[derive(Clone, Debug, Default)]
pub struct TestOptions {
    pub expect_status: Option<u16>,
    pub expect_headers: Vec<String>,
    pub expect_json: Vec<String>,
    pub expect_body_contains: Vec<String>,
    pub max_time_ms: Option<u64>,
}

/// One evaluated assertion.
#[derive(Clone, Debug, Serialize)]
pub struct AssertionResult {
    pub assertion: String,
    pub passed: bool,
    pub details: String,
}

/// Structured report for test execution.
#[derive(Clone, Debug, Serialize)]
pub struct TestReport {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub passed: bool,
    pub assertions: Vec<AssertionResult>,
}

/// Evaluates response assertions and returns a structured report.
pub fn evaluate_response(
    method: &str,
    url: &str,
    response: &ResponseData,
    elapsed_ms: u64,
    opts: &TestOptions,
) -> TestReport {
    let mut assertions = Vec::new();

    if let Some(expected) = opts.expect_status {
        let passed = response.status_code == expected;
        assertions.push(AssertionResult {
            assertion: format!("status == {expected}"),
            passed,
            details: if passed {
                format!("status matched {expected}")
            } else {
                format!("got {}, expected {}", response.status_code, expected)
            },
        });
    }

    for raw in &opts.expect_headers {
        assertions.push(evaluate_header_assertion(response, raw));
    }

    for raw in &opts.expect_body_contains {
        let body = String::from_utf8_lossy(&response.body);
        let passed = body.contains(raw);
        assertions.push(AssertionResult {
            assertion: format!("body contains {:?}", raw),
            passed,
            details: if passed {
                "substring found".to_string()
            } else {
                "substring not found".to_string()
            },
        });
    }

    if !opts.expect_json.is_empty() {
        match serde_json::from_slice::<Value>(&response.body) {
            Ok(json) => {
                for raw in &opts.expect_json {
                    assertions.push(evaluate_json_assertion(&json, raw));
                }
            }
            Err(err) => assertions.push(AssertionResult {
                assertion: "response body is JSON".to_string(),
                passed: false,
                details: format!("failed to parse response JSON: {err}"),
            }),
        }
    }

    if let Some(limit) = opts.max_time_ms {
        let passed = elapsed_ms <= limit;
        assertions.push(AssertionResult {
            assertion: format!("elapsed_ms <= {limit}"),
            passed,
            details: if passed {
                format!("{elapsed_ms}ms <= {limit}ms")
            } else {
                format!("{elapsed_ms}ms > {limit}ms")
            },
        });
    }

    let passed = assertions.iter().all(|a| a.passed);

    TestReport {
        method: method.to_string(),
        url: url.to_string(),
        status: response.status_code,
        elapsed_ms,
        passed,
        assertions,
    }
}

/// Renders a test report as human-readable text.
pub fn render_text_report(report: &TestReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} {} -> {} in {}ms\n",
        report.method, report.url, report.status, report.elapsed_ms
    ));
    for assertion in &report.assertions {
        let marker = if assertion.passed { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "[{marker}] {} ({})\n",
            assertion.assertion, assertion.details
        ));
    }
    out.push_str(&format!(
        "Result: {}\n",
        if report.passed { "PASSED" } else { "FAILED" }
    ));
    out
}

fn evaluate_header_assertion(response: &ResponseData, raw: &str) -> AssertionResult {
    let token = raw.trim();
    if token.is_empty() {
        return AssertionResult {
            assertion: "header assertion".to_string(),
            passed: false,
            details: "empty header assertion token".to_string(),
        };
    }

    if let Some((name, expected_part)) = token.split_once('~') {
        let name = name.trim();
        let expected_part = expected_part.trim();
        let value = header_value(response, name);
        let passed = value
            .as_ref()
            .map(|v| v.contains(expected_part))
            .unwrap_or(false);
        return AssertionResult {
            assertion: format!("header {name} contains {expected_part:?}"),
            passed,
            details: match value {
                Some(v) if passed => format!("found value {v:?}"),
                Some(v) => format!("value {v:?} does not contain {expected_part:?}"),
                None => "header missing".to_string(),
            },
        };
    }

    if let Some((name, expected)) = token.split_once('=') {
        let name = name.trim();
        let expected = expected.trim();
        let value = header_value(response, name);
        let passed = value.as_deref() == Some(expected);
        return AssertionResult {
            assertion: format!("header {name} == {expected:?}"),
            passed,
            details: match value {
                Some(_v) if passed => "exact match".to_string(),
                Some(v) => format!("got {v:?}"),
                None => "header missing".to_string(),
            },
        };
    }

    let name = token;
    let value = header_value(response, name);
    AssertionResult {
        assertion: format!("header {name} exists"),
        passed: value.is_some(),
        details: value
            .map(|v| format!("present with value {v:?}"))
            .unwrap_or_else(|| "header missing".to_string()),
    }
}

fn evaluate_json_assertion(json: &Value, raw: &str) -> AssertionResult {
    let token = raw.trim();
    let Some((path, expected_raw)) = token.split_once('=') else {
        return AssertionResult {
            assertion: format!("json {token}"),
            passed: false,
            details: "expected format path=value".to_string(),
        };
    };

    let path = path.trim();
    let expected_raw = expected_raw.trim();
    let pointer = match dot_path_to_pointer(path) {
        Ok(v) => v,
        Err(err) => {
            return AssertionResult {
                assertion: format!("json {path}"),
                passed: false,
                details: err,
            };
        }
    };

    let Some(actual) = json.pointer(&pointer) else {
        return AssertionResult {
            assertion: format!("json {path} == {expected_raw}"),
            passed: false,
            details: "path not found".to_string(),
        };
    };

    let expected = parse_expected_json_value(expected_raw);
    let passed = actual == &expected;
    AssertionResult {
        assertion: format!("json {path} == {expected_raw}"),
        passed,
        details: if passed {
            "value matched".to_string()
        } else {
            format!("got {}", actual)
        },
    }
}

fn header_value(response: &ResponseData, name: &str) -> Option<String> {
    response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

fn parse_expected_json_value(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn dot_path_to_pointer(path: &str) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err("empty JSON path".to_string());
    }
    if path.starts_with('/') {
        return Ok(path.to_string());
    }

    let mut pointer = String::new();
    for segment in path.split('.') {
        if segment.is_empty() {
            return Err("invalid JSON path: empty segment".to_string());
        }
        append_segment(&mut pointer, segment)?;
    }
    Ok(pointer)
}

fn append_segment(pointer: &mut String, segment: &str) -> Result<(), String> {
    let mut rest = segment;
    while !rest.is_empty() {
        if let Some(idx) = rest.find('[') {
            let key = &rest[..idx];
            if !key.is_empty() {
                pointer.push('/');
                pointer.push_str(&escape_pointer_token(key));
            }
            let after = &rest[idx + 1..];
            let Some(close) = after.find(']') else {
                return Err(format!("invalid JSON path segment: {segment}"));
            };
            let index = &after[..close];
            if index.is_empty() || !index.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("invalid JSON array index in segment: {segment}"));
            }
            pointer.push('/');
            pointer.push_str(index);
            rest = &after[close + 1..];
        } else {
            pointer.push('/');
            pointer.push_str(&escape_pointer_token(rest));
            rest = "";
        }
    }
    Ok(())
}

fn escape_pointer_token(input: &str) -> String {
    input.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::{dot_path_to_pointer, evaluate_response, TestOptions};
    use crate::response::ResponseData;

    #[test]
    fn path_conversion_supports_arrays() {
        let p = dot_path_to_pointer("users[0].name").expect("valid path");
        assert_eq!(p, "/users/0/name");
    }

    #[test]
    fn basic_assertions_pass() {
        let response = ResponseData {
            status_code: 200,
            reason: "OK".to_string(),
            final_url: "https://example.com".to_string(),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            content_type: Some("application/json".to_string()),
            body: br#"{"user":{"id":42,"name":"faizan"}}"#.to_vec(),
        };

        let opts = TestOptions {
            expect_status: Some(200),
            expect_headers: vec!["content-type~json".to_string()],
            expect_json: vec!["user.id=42".to_string()],
            expect_body_contains: vec!["faizan".to_string()],
            max_time_ms: Some(500),
        };

        let report = evaluate_response("GET", "https://example.com", &response, 80, &opts);
        assert!(report.passed);
    }
}
