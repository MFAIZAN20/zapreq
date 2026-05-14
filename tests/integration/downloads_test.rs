use crate::common;
use assert_cmd::Command;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn download_writes_response_to_output_path() {
    let cfg = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("temp dir");
    let output = work.path().join("robots.txt");

    let mut server = common::mock_server();
    let download_mock = server
        .mock("GET", "/file")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("hello-download")
        .create();

    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/file", server.url()),
            "--download",
            "--output",
            output.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success();
    download_mock.assert();

    let written = std::fs::read_to_string(&output).expect("downloaded file");
    assert_eq!(written, "hello-download");
}

#[test]
fn continue_download_sends_range_and_appends_data() {
    let cfg = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("temp dir");
    let output = work.path().join("resume.bin");
    std::fs::write(&output, b"abc").expect("seed resume file");

    let mut server = common::mock_server();
    let resume_mock = server
        .mock("GET", "/resume")
        .match_header("range", "bytes=3-")
        .with_status(206)
        .with_header("content-type", "application/octet-stream")
        .with_header("content-length", "3")
        .with_body("def")
        .create();

    zapreq(&cfg)
        .args([
            "GET",
            &format!("{}/resume", server.url()),
            "--download",
            "--continue",
            "--output",
            output.to_str().expect("utf8 output path"),
        ])
        .assert()
        .success();
    resume_mock.assert();

    let written = std::fs::read(&output).expect("resumed file");
    assert_eq!(written, b"abcdef");
}
