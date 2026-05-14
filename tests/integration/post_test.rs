use crate::common;
use assert_cmd::Command;
use mockito::Matcher;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn post_with_equals_sends_json_body() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("POST", "/post")
        .match_header(
            "content-type",
            Matcher::Regex("application/json".to_string()),
        )
        .match_body(Matcher::JsonString(r#"{"key":"value"}"#.to_string()))
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["POST", &format!("{}/post", server.url()), "key=value"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn post_with_colon_equals_sends_raw_json_value() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("POST", "/post")
        .match_body(Matcher::PartialJsonString(
            r#"{"payload":{"x":1}}"#.to_string(),
        ))
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args([
            "POST",
            &format!("{}/post", server.url()),
            "payload:={\"x\":1}",
        ])
        .assert()
        .success();
    m.assert();
}

#[test]
fn post_form_sends_urlencoded() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("POST", "/form")
        .match_header(
            "content-type",
            Matcher::Regex("application/x-www-form-urlencoded".to_string()),
        )
        .match_body(Matcher::Regex("a=1".to_string()))
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["POST", &format!("{}/form", server.url()), "--form", "a=1"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn content_type_json_set_automatically() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("POST", "/ct")
        .match_header(
            "content-type",
            Matcher::Regex("application/json".to_string()),
        )
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["POST", &format!("{}/ct", server.url()), "a=1"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn inferred_post_when_body_items_present() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server.mock("POST", "/infer").with_status(200).create();
    zapreq(&cfg)
        .args([&format!("{}/infer", server.url()), "a=1"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn multiple_equals_fields_merged_json() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("POST", "/merge")
        .match_body(Matcher::JsonString(r#"{"a":"1","b":"2"}"#.to_string()))
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["POST", &format!("{}/merge", server.url()), "a=1", "b=2"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn verbose_prints_request_and_response() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "POST", "/v", 200, "ok");
    let assert = zapreq(&cfg)
        .args(["POST", &format!("{}/v", server.url()), "-v", "a=1"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("HTTP/1.1"));
    assert!(stdout.contains("200 OK"));
}

#[test]
fn empty_post_sends_no_body() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server.mock("POST", "/empty").with_status(200).create();
    zapreq(&cfg)
        .args(["POST", &format!("{}/empty", server.url())])
        .assert()
        .success();
    m.assert();
}
