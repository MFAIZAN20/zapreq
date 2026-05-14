use crate::common;
use assert_cmd::Command;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn basic_auth_sends_authorization_header() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let token = BASE64.encode("user:pass");
    let m = server
        .mock("GET", "/basic")
        .match_header("authorization", format!("Basic {token}").as_str())
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/basic", server.url()),
            "--auth",
            "user:pass",
        ])
        .assert()
        .success();
    m.assert();
}

#[test]
fn bearer_auth_sends_header() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = server
        .mock("GET", "/bearer")
        .match_header("authorization", "Bearer mytoken")
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/bearer", server.url()),
            "--auth-type",
            "bearer",
            "--auth",
            "mytoken",
        ])
        .assert()
        .success();
    m.assert();
}

#[test]
fn missing_auth_with_auth_type_warns_user() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/warn", 200, "ok");
    let assert = zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/warn", server.url()),
            "--auth-type",
            "bearer",
        ])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("warning: --auth-type=bearer"));
}

#[test]
fn basic_auth_401_propagated_no_retry() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let m = common::mock_auth_401(&mut server, "/deny");
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/deny", server.url()),
            "--auth",
            "user:bad",
        ])
        .assert()
        .success();
    m.assert();
}

#[test]
fn auth_masked_in_verbose_output() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/verbose", 200, "ok");
    let assert = zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/verbose", server.url()),
            "--auth",
            "user:secret",
            "--verbose",
        ])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("Auth: user:****"));
}

#[test]
fn session_saves_and_restores_auth() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let token = BASE64.encode("user:pass");

    let first = server
        .mock("GET", "/login")
        .match_header("authorization", format!("Basic {token}").as_str())
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/login", server.url()),
            "--auth",
            "user:pass",
            "--session",
            "auth-test",
        ])
        .assert()
        .success();
    first.assert();

    let second = server
        .mock("GET", "/protected")
        .match_header("authorization", format!("Basic {token}").as_str())
        .with_status(200)
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/protected", server.url()),
            "--session",
            "auth-test",
        ])
        .assert()
        .success();
    second.assert();
}
