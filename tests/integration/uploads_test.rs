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
fn form_file_upload_sends_multipart_body() {
    let cfg = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("temp dir");
    let file = work.path().join("upload.txt");
    std::fs::write(&file, "hello-file").expect("write upload fixture");

    let mut server = common::mock_server();
    let upload_mock = server
        .mock("POST", "/upload")
        .match_header(
            "content-type",
            Matcher::Regex("multipart/form-data".to_string()),
        )
        .match_body(Matcher::Regex(
            "(?s).*name=\\\"doc\\\"; filename=\\\"upload.txt\\\".*hello-file.*".to_string(),
        ))
        .with_status(200)
        .with_body("ok")
        .create();

    zapreq(&cfg)
        .args([
            "POST",
            &format!("{}/upload", server.url()),
            "--form",
            &format!("doc@{}", file.display()),
        ])
        .assert()
        .success();
    upload_mock.assert();
}

#[test]
fn data_from_file_operator_reads_text_payload() {
    let cfg = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("temp dir");
    let file = work.path().join("body.txt");
    std::fs::write(&file, "hello world from file").expect("write text fixture");

    let mut server = common::mock_server();
    let payload_mock = server
        .mock("POST", "/text")
        .match_header(
            "content-type",
            Matcher::Regex("application/json".to_string()),
        )
        .match_body(Matcher::JsonString(
            r#"{"note":"hello world from file"}"#.to_string(),
        ))
        .with_status(200)
        .create();

    zapreq(&cfg)
        .args([
            "POST",
            &format!("{}/text", server.url()),
            &format!("note=@{}", file.display()),
        ])
        .assert()
        .success();
    payload_mock.assert();
}

#[test]
fn json_from_file_operator_reads_json_payload() {
    let cfg = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("temp dir");
    let file = work.path().join("payload.json");
    std::fs::write(&file, r#"{"x":1,"ok":true}"#).expect("write json fixture");

    let mut server = common::mock_server();
    let payload_mock = server
        .mock("POST", "/json-file")
        .match_body(Matcher::PartialJsonString(
            r#"{"payload":{"x":1,"ok":true}}"#.to_string(),
        ))
        .with_status(200)
        .create();

    zapreq(&cfg)
        .args([
            "POST",
            &format!("{}/json-file", server.url()),
            &format!("payload:=@{}", file.display()),
        ])
        .assert()
        .success();
    payload_mock.assert();
}
