use crate::common;
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn save_creates_collection_file() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args([
            "save",
            "login",
            "--",
            "POST",
            "https://example.com/login",
            "username=faizan",
        ])
        .assert()
        .success();

    let path = cfg.path().join("collections").join("login.json");
    assert!(
        path.exists(),
        "expected collection file at {}",
        path.display()
    );
}

#[test]
fn list_shows_saved_collections() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args(["save", "status", "--", "GET", "https://example.com/status"])
        .assert()
        .success();
    let assert = zapreq(&cfg).args(["list"]).assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("status"));
}

#[test]
fn run_executes_saved_request() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let run_mock = server.mock("GET", "/ping").with_status(200).create();
    zapreq(&cfg)
        .args([
            "save",
            "ping",
            "--",
            "GET",
            &format!("{}/ping", server.url()),
        ])
        .assert()
        .success();
    zapreq(&cfg).args(["run", "ping"]).assert().success();
    run_mock.assert();
}

#[test]
fn delete_removes_collection_file() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args(["save", "remove-me", "--", "GET", "https://example.com/one"])
        .assert()
        .success();
    let path = cfg.path().join("collections").join("remove-me.json");
    assert!(path.exists());
    zapreq(&cfg)
        .args(["delete", "remove-me"])
        .assert()
        .success();
    assert!(!path.exists());
}

#[test]
fn run_with_env_profile_applies_profile_variables() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let run_mock = server.mock("GET", "/users/42").with_status(200).create();

    zapreq(&cfg)
        .args(["save", "who", "--", "GET", "users/{USER_ID}"])
        .assert()
        .success();

    let env_dir = cfg.path().join("envs");
    fs::create_dir_all(&env_dir).expect("env dir");
    let profile = serde_json::json!({
        "base_url": server.url(),
        "headers": {},
        "variables": { "USER_ID": "42" }
    });
    fs::write(
        env_dir.join("dev.json"),
        serde_json::to_string_pretty(&profile).expect("profile json"),
    )
    .expect("profile write");

    zapreq(&cfg)
        .args(["run", "who", "--env-profile", "dev"])
        .assert()
        .success();
    run_mock.assert();
}
