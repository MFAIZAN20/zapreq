use crate::common;
use assert_cmd::Command;
use tempfile::TempDir;

fn ferrite(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("FERRITE_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn request_without_follow_does_not_hit_redirect_target() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let target = format!("{}/final", server.url());

    let redirect = server
        .mock("GET", "/start")
        .with_status(302)
        .with_header("location", &target)
        .create();
    let final_target = server
        .mock("GET", "/final")
        .with_status(200)
        .with_body("ok")
        .expect(0)
        .create();

    ferrite(&cfg)
        .args(["GET", &format!("{}/start", server.url())])
        .assert()
        .success();

    redirect.assert();
    final_target.assert();
}

#[test]
fn request_with_follow_reaches_redirect_target() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let target = format!("{}/final", server.url());

    let redirect = server
        .mock("GET", "/start")
        .with_status(302)
        .with_header("location", &target)
        .create();
    let final_target = server
        .mock("GET", "/final")
        .with_status(200)
        .with_body("redirect-ok")
        .create();

    let assert = ferrite(&cfg)
        .args(["GET", &format!("{}/start", server.url()), "--follow"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("redirect-ok"));
    redirect.assert();
    final_target.assert();
}
