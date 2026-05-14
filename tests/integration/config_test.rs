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
fn config_default_options_are_applied() {
    let cfg = TempDir::new().expect("temp dir");
    std::fs::write(
        cfg.path().join("config.json"),
        r#"{"default_options":["--form"]}"#,
    )
    .expect("config write");

    let mut server = common::mock_server();
    let form_mock = server
        .mock("POST", "/cfg-form")
        .match_header(
            "content-type",
            Matcher::Regex("application/x-www-form-urlencoded".to_string()),
        )
        .match_body(Matcher::Regex("a=1".to_string()))
        .with_status(200)
        .create();

    zapreq(&cfg)
        .args(["POST", &format!("{}/cfg-form", server.url()), "a=1"])
        .assert()
        .success();
    form_mock.assert();
}

#[test]
fn env_default_options_override_config_defaults() {
    let cfg = TempDir::new().expect("temp dir");
    std::fs::write(
        cfg.path().join("config.json"),
        r#"{"default_options":["--form"]}"#,
    )
    .expect("config write");

    let mut server = common::mock_server();
    let json_mock = server
        .mock("POST", "/cfg-json")
        .match_header(
            "content-type",
            Matcher::Regex("application/json".to_string()),
        )
        .match_body(Matcher::JsonString(r#"{"a":"1"}"#.to_string()))
        .with_status(200)
        .create();

    zapreq(&cfg)
        .env("ZAPREQ_DEFAULT_OPTIONS", "--json")
        .args(["POST", &format!("{}/cfg-json", server.url()), "a=1"])
        .assert()
        .success();
    json_mock.assert();
}
