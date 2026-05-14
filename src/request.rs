use anyhow::{anyhow, Context, Result};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};

use crate::auth::AuthPlugin;
use crate::cli::CliArgs;
use crate::items::{collect_from_parsed, CollectedItems, RequestItem};
use crate::response::{RequestTrace, ResponseData};

/// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-05:
/// Request execution service for building and sending HTTP requests.
pub struct RequestEngine;

impl Default for RequestEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// CAUS-CORERUNTIM-01:
/// Execution input contract after CLI parsing.
#[derive(Clone, Debug)]
pub struct RequestSpec {
    pub method: String,
    pub url: String,
    pub items: Vec<RequestItem>,
}

/// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-03:
/// Prepared request contract used before network execution.
pub struct PreparedRequest {
    pub builder: RequestBuilder,
    pub method: String,
    pub url: String,
    pub body_preview: Option<serde_json::Value>,
    pub headers_preview: HeaderMap,
}

impl RequestEngine {
    /// CAUS-CORERUNTIM-03:
    /// Creates a stateless request engine.
    pub fn new() -> Self {
        Self
    }

    /// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-03:
    /// Prepares a request without sending it, for offline and preview flows.
    pub fn prepare(
        &self,
        args: &CliArgs,
        spec: &RequestSpec,
        auth: Option<&dyn AuthPlugin>,
    ) -> Result<PreparedRequest> {
        let client = build_client(args)?;
        let method = reqwest::Method::from_bytes(spec.method.as_bytes())
            .with_context(|| format!("invalid HTTP method: {}", spec.method))?;

        let collected = collect_from_parsed(&spec.items)?;
        self.prepare_request(&client, &method, spec, args, auth, &collected)
    }

    /// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-03, CAUS-CORERUNTIM-05:
    /// Sends request and returns normalized response payload for rendering.
    pub fn send(
        &self,
        args: &CliArgs,
        spec: &RequestSpec,
        auth: Option<&dyn AuthPlugin>,
    ) -> Result<(RequestTrace, ResponseData)> {
        let prepared = self.prepare(args, spec, auth)?;
        let trace = request_trace_from_prepared(&prepared);

        let retry_base = prepared.builder.try_clone();
        let mut response = prepared
            .builder
            .send()
            .with_context(|| format!("request failed for URL {}", spec.url))?;

        if response.status().as_u16() == 401 {
            if let (Some(plugin), Some(base_builder)) = (auth, retry_base) {
                if let Some(retry_builder) = plugin.handle_401(base_builder, &response) {
                    let retry_trace = trace.clone();
                    response = retry_builder
                        .send()
                        .with_context(|| format!("401 retry failed for URL {}", spec.url))?;
                    let parsed = parse_response(response)?;
                    return Ok((retry_trace, parsed));
                }
            }
        }

        let parsed = parse_response(response)?;
        Ok((trace, parsed))
    }

    /// CAUS-OUTPUT-13:
    /// Sends request and returns raw response stream for download mode.
    pub fn send_raw_for_download(
        &self,
        args: &CliArgs,
        spec: &RequestSpec,
        auth: Option<&dyn AuthPlugin>,
    ) -> Result<(RequestTrace, Response)> {
        let prepared = self.prepare(args, spec, auth)?;
        let trace = request_trace_from_prepared(&prepared);

        let response = prepared
            .builder
            .send()
            .with_context(|| format!("download request failed for URL {}", spec.url))?;
        Ok((trace, response))
    }

    /// CAUS-CORERUNTIM-01, CAUS-CORERUNTIM-03:
    /// Builds a fully prepared request from parsed CLI and collected request items.
    fn prepare_request(
        &self,
        client: &Client,
        method: &reqwest::Method,
        spec: &RequestSpec,
        args: &CliArgs,
        auth: Option<&dyn AuthPlugin>,
        collected: &CollectedItems,
    ) -> Result<PreparedRequest> {
        let mut builder = client.request(method.clone(), &spec.url);
        let mut headers_preview = HeaderMap::new();

        for (key, value) in &collected.headers {
            let name = HeaderName::from_bytes(key.as_bytes())
                .with_context(|| format!("invalid header name: {key}"))?;
            let val = HeaderValue::from_str(value)
                .with_context(|| format!("invalid header value for {key}"))?;
            headers_preview.append(name, val);
        }

        if !collected.query_params.is_empty() {
            builder = builder.query(&collected.query_params);
        }

        if !headers_preview.is_empty() {
            builder = builder.headers(headers_preview.clone());
        }

        if let Some(plugin) = auth {
            builder = plugin.apply(builder);
        }

        let mut body_preview: Option<serde_json::Value> = None;
        let has_file_uploads = !collected.files.is_empty();
        if args.multipart || has_file_uploads {
            let mut form = Form::new();
            for (k, v) in &collected.data_strings {
                form = form.text(k.clone(), v.clone());
            }
            for (k, v) in &collected.data_json {
                form = form.text(k.clone(), v.to_string());
            }
            for file in &collected.files {
                let mut part = Part::file(&file.path)
                    .with_context(|| format!("failed to open upload file: {}", file.path))?;
                if let Some(content_type) = &file.content_type {
                    part = part
                        .mime_str(content_type)
                        .with_context(|| format!("invalid MIME type: {content_type}"))?;
                }
                form = form.part(file.key.clone(), part);
            }

            body_preview = Some(serde_json::Value::String(
                "<multipart/form-data>".to_string(),
            ));
            builder = builder.multipart(form);
        } else if args.form {
            let mut pairs = collected.data_strings.clone();
            for (k, v) in &collected.data_json {
                pairs.push((k.clone(), v.to_string()));
            }
            if !pairs.is_empty() {
                body_preview = Some(serde_json::Value::Array(
                    pairs
                        .iter()
                        .map(|(k, v)| serde_json::Value::String(format!("{k}={v}")))
                        .collect(),
                ));
                builder = builder.form(&pairs);
            }
        } else {
            let mut map = serde_json::Map::new();
            for (k, v) in &collected.data_strings {
                map.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            for (k, v) in &collected.data_json {
                map.insert(k.clone(), v.clone());
            }
            if !map.is_empty() {
                let value = serde_json::Value::Object(map);
                body_preview = Some(value.clone());
                builder = builder.json(&value);
            }
        }

        Ok(PreparedRequest {
            builder,
            method: spec.method.clone(),
            url: spec.url.clone(),
            body_preview,
            headers_preview,
        })
    }
}

/// CAUS-CORERUNTIM-03:
/// Creates configured reqwest client for request execution.
fn build_client(args: &CliArgs) -> Result<Client> {
    let mut builder = Client::builder().user_agent("zapreq/0.1.0");

    if let Some(timeout_s) = args.timeout {
        builder = builder.timeout(std::time::Duration::from_secs_f64(timeout_s));
    }

    builder = builder.danger_accept_invalid_certs(!args.verify);

    if args.follow {
        let limit = args.max_redirects.unwrap_or(10);
        builder = builder.redirect(reqwest::redirect::Policy::limited(limit));
    } else {
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }

    for proxy_value in &args.proxy {
        let proxy_url = parse_proxy(proxy_value)?;
        let proxy = reqwest::Proxy::all(&proxy_url)
            .with_context(|| format!("invalid proxy value: {proxy_value}"))?;
        builder = builder.proxy(proxy);
    }

    if let Some(cert_path) = &args.cert {
        let cert_bytes = std::fs::read(cert_path)
            .with_context(|| format!("failed to read cert file: {cert_path}"))?;

        let key_path = args.cert_key.as_ref().ok_or_else(|| {
            anyhow!(
                "--cert was provided without --cert-key; provide both PEM cert and PEM private key"
            )
        })?;
        let key_bytes = std::fs::read(key_path)
            .with_context(|| format!("failed to read cert key file: {key_path}"))?;
        let identity =
            reqwest::Identity::from_pkcs8_pem(&cert_bytes, &key_bytes).with_context(|| {
                format!("failed to parse cert/key PEM pair: cert={cert_path}, key={key_path}")
            })?;

        builder = builder.identity(identity);
    } else if args.cert_key.is_some() {
        return Err(anyhow!(
            "--cert-key was provided without --cert; both are required for split key/cert"
        ));
    }

    builder.build().context("failed to build HTTP client")
}

/// CAUS-CORERUNTIM-03:
/// Parses proxy values as protocol:url pairs.
fn parse_proxy(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("proxy value cannot be empty"));
    }

    if let Some((proto, rest)) = trimmed.split_once(':') {
        let rhs = rest.trim_start_matches('/');
        if rhs.is_empty() {
            return Err(anyhow!("proxy URL is empty in '{raw}'"));
        }
        let normalized = if rhs.starts_with("http://") || rhs.starts_with("https://") {
            rhs.to_string()
        } else {
            format!("{proto}://{rhs}")
        };
        return Ok(normalized);
    }

    Err(anyhow!(
        "proxy must be protocol:url (e.g. http:http://127.0.0.1:8080)"
    ))
}

/// CAUS-CORERUNTIM-03:
/// Converts a prepared request into output trace preview data.
pub fn request_trace_from_prepared(prepared: &PreparedRequest) -> RequestTrace {
    let mut headers = Vec::new();
    for (name, value) in &prepared.headers_preview {
        let value_str = value.to_str().unwrap_or("<non-utf8>").to_string();
        headers.push((name.to_string(), value_str));
    }

    let body_preview = prepared.body_preview.as_ref().map(|v| v.to_string());

    RequestTrace {
        method: prepared.method.clone(),
        url: prepared.url.clone(),
        headers,
        body_preview,
    }
}

/// CAUS-CORERUNTIM-03:
/// Parses reqwest response into canonical ResponseData.
fn parse_response(mut response: Response) -> Result<ResponseData> {
    let status = response.status();
    let reason = status.canonical_reason().unwrap_or("UNKNOWN").to_string();
    let final_url = response.url().to_string();

    let mut headers = Vec::new();
    let mut content_type = None;
    for (k, v) in response.headers() {
        let value = v
            .to_str()
            .context("response header contains invalid UTF-8")?
            .to_string();
        if k == CONTENT_TYPE {
            content_type = Some(value.clone());
        }
        headers.push((k.to_string(), value));
    }

    let mut body = Vec::new();
    response
        .copy_to(&mut body)
        .context("failed to read response body")?;

    Ok(ResponseData {
        status_code: status.as_u16(),
        reason,
        final_url,
        headers,
        content_type,
        body,
    })
}
