use crate::common;
use assert_cmd::Command;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn json_pretty_printed_with_indent() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_json(
        &mut server,
        "GET",
        "/json",
        200,
        serde_json::json!({"a":{"b":1}}),
    );
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/json", server.url())])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\n    \"b\""));
}

#[test]
fn xml_response_indented() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = server
        .mock("GET", "/xml")
        .with_status(200)
        .with_header("content-type", "application/xml")
        .with_body("<root><item>1</item></root>")
        .create();
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/xml", server.url())])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\n  <item>"));
}

#[test]
fn binary_response_shows_warning() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = server
        .mock("GET", "/bin")
        .with_status(200)
        .with_header("content-type", "application/octet-stream")
        .with_body("\0\0abc")
        .create();
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/bin", server.url())])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("[binary body"));
}

#[test]
fn meta_flag_shows_metadata_box() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_text(&mut server, "GET", "/meta", 200, "ok");
    let assert = zapreq(&cfg)
        .args(["GET", &format!("{}/meta", server.url()), "--meta"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("Method:"));
    assert!(stdout.contains("Status:"));
}

#[test]
fn style_dracula_changes_theme_no_crash() {
    let cfg = TempDir::new().expect("temp dir");
    let mut server = common::mock_server();
    let _m = common::mock_json(
        &mut server,
        "GET",
        "/theme",
        200,
        serde_json::json!({"ok":true}),
    );
    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/theme", server.url()),
            "--style",
            "dracula",
        ])
        .assert()
        .success();
}
