mod commands;
mod state;

use commands::*;

fn main() {
    ensure_supported_gtk_locale();

    tauri::Builder::default()
        .manage(state::AppState)
        .invoke_handler(tauri::generate_handler![
            get_workspaces,
            create_workspace,
            delete_workspace,
            delete_request,
            rename_workspace,
            get_collections,
            create_collection,
            get_request,
            save_request,
            send_request,
            get_environments,
            save_environment,
            get_reports,
            get_test_cases,
            run_test_case,
            run_test_suite,
            get_test_runs,
            get_app_settings,
            save_app_settings,
            import_workspace,
            export_workspace,
            get_secrets,
            set_secret,
            delete_secret,
            save_test_case,
            delete_test_case
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ZapReq Tauri desktop app");
}

#[cfg(target_os = "linux")]
fn ensure_supported_gtk_locale() {
    use std::process::Command;

    let current = std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default();

    let Ok(output) = Command::new("locale").arg("-a").output() else {
        return;
    };

    let available = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(normalize_locale)
        .collect::<Vec<_>>();

    if !current.trim().is_empty() && available.iter().any(|locale| *locale == normalize_locale(&current)) {
        return;
    }

    for fallback in ["C.UTF-8", "C.utf8", "C"] {
        if available.iter().any(|locale| *locale == normalize_locale(fallback)) {
            std::env::set_var("LC_ALL", fallback);
            return;
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn ensure_supported_gtk_locale() {}

#[cfg(target_os = "linux")]
fn normalize_locale(value: &str) -> String {
    value.trim().replace(['-', '_'], "").to_lowercase()
}
