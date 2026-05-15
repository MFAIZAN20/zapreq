use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn zapreq(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("http").expect("binary should build");
    cmd.env("ZAPREQ_CONFIG_DIR", config_dir.path());
    cmd
}

#[test]
fn collections_new_creates_workspace_file() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args(["collections", "new", "team"])
        .assert()
        .success();
    let path = cfg.path().join("workspaces").join("team.json");
    assert!(
        path.exists(),
        "workspace file should exist at {}",
        path.display()
    );
}

#[test]
fn requests_save_and_list_roundtrip() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args([
            "requests",
            "save",
            "api",
            "list-users",
            "--",
            "GET",
            "https://example.com/users",
        ])
        .assert()
        .success();

    let assert = zapreq(&cfg)
        .args(["requests", "list", "api"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("list-users"));
}

#[test]
fn collections_migrate_imports_legacy_aliases() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args(["save", "old-one", "--", "GET", "https://example.com/legacy"])
        .assert()
        .success();
    zapreq(&cfg)
        .args(["collections", "migrate", "--workspace", "legacy"])
        .assert()
        .success();
    let assert = zapreq(&cfg)
        .args(["requests", "list", "legacy"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("old-one"));
}

#[test]
fn collections_export_and_import_zapreq() {
    let cfg = TempDir::new().expect("temp dir");
    zapreq(&cfg)
        .args([
            "requests",
            "save",
            "source",
            "req-a",
            "--",
            "GET",
            "https://example.com/a",
        ])
        .assert()
        .success();

    let export_path = cfg.path().join("export.json");
    zapreq(&cfg)
        .args([
            "collections",
            "export",
            "source",
            export_path.to_str().expect("utf8 path"),
            "--format",
            "zapreq",
        ])
        .assert()
        .success();
    assert!(export_path.exists());

    zapreq(&cfg)
        .args([
            "collections",
            "import",
            "target",
            export_path.to_str().expect("utf8 path"),
        ])
        .assert()
        .success();

    let assert = zapreq(&cfg)
        .args(["requests", "list", "target"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("req-a"));

    let imported = fs::read_to_string(cfg.path().join("workspaces").join("target.json"))
        .expect("target workspace should be readable");
    assert!(imported.contains("\"name\": \"target\""));
}
