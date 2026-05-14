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
fn get_sends_user_agent() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("GET", "/ua")
        .match_header("user-agent", "zapreq/0.1.0")
        .with_status(200)
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args(["GET", &format!("{}/ua", server.url())])
        .assert()
        .success();
    m.assert();
}

#[test]
fn get_with_query_param_in_url() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("GET", "/search?q=rust")
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["GET", &format!("{}/search", server.url()), "q==rust"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn get_with_custom_header() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("GET", "/hdr")
        .match_header("x-test", "abc")
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args(["GET", &format!("{}/hdr", server.url()), "X-Test:abc"])
        .assert()
        .success();
    m.assert();
}

#[test]
fn status_200_exits_zero() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/ok", 200, "ok");
    zapreq(&cfg)
        .args(["GET", &format!("{}/ok", server.url())])
        .assert()
        .success();
}

#[test]
fn status_404_without_check_status_exits_zero() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/missing", 404, "not found");
    zapreq(&cfg)
        .args(["GET", &format!("{}/missing", server.url())])
        .assert()
        .success();
}

#[test]
fn status_404_with_check_status_exits_one() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/missing", 404, "not found");
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/missing", server.url()),
            "--check-status",
        ])
        .assert()
        .code(1);
}

#[test]
fn pretty_none_has_no_ansi() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_json(
        &mut server,
        "GET",
        "/json",
        200,
        serde_json::json!({"hello":"world"}),
    );
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/json", server.url()), "--pretty", "none"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(!stdout.contains("\u{1b}["));
}

#[test]
fn print_h_only_response_headers() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/h", 200, "body-text");
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/h", server.url()), "--print", "h"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("200 OK"));
    assert!(!stdout.contains("body-text"));
}

#[test]
fn print_b_only_response_body() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/b", 200, "body-text");
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/b", server.url()), "--print", "b"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("body-text"));
    assert!(!stdout.contains("200 OK"));
}

#[test]
fn offline_prints_request_without_sending() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = server
        .mock("GET", Matcher::Any)
        .with_status(500)
        .with_body("should-not-hit")
        .expect(0)
        .create();
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/offline", server.url()), "--offline"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("[offline mode"));
}
