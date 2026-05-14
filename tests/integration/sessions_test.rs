use crate::common;
use assert_cmd::Command;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn session_persists_cookie_for_followup_requests() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();

    let seed = server
        .mock("GET", "/seed")
        .with_status(200)
        .with_header("set-cookie", "sid=abc; Path=/")
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/seed", server.url()),
            "--session",
            "cookie",
        ])
        .assert()
        .success();
    seed.assert();

    let followup = server
        .mock("GET", "/profile")
        .match_header("cookie", "sid=abc")
        .with_status(200)
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/profile", server.url()),
            "--session",
            "cookie",
        ])
        .assert()
        .success();
    followup.assert();
}

#[test]
fn session_read_only_does_not_persist_cookie_changes() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();

    let seed = server
        .mock("GET", "/seed")
        .with_status(200)
        .with_header("set-cookie", "sid=one; Path=/")
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args(["GET", &format!("{}/seed", server.url()), "--session", "ro"])
        .assert()
        .success();
    seed.assert();

    let read_only = server
        .mock("GET", "/readonly")
        .match_header("cookie", "sid=one")
        .with_status(200)
        .with_header("set-cookie", "sid=two; Path=/")
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/readonly", server.url()),
            "--session",
            "ro",
            "--session-read-only",
        ])
        .assert()
        .success();
    read_only.assert();

    let after = server
        .mock("GET", "/after")
        .match_header("cookie", "sid=one")
        .with_status(200)
        .with_body("ok")
        .create();
    zapreq(&cfg)
        .args(["GET", &format!("{}/after", server.url()), "--session", "ro"])
        .assert()
        .success();
    after.assert();
}
