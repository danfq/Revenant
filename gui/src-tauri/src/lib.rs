//! desktop backend for revenant
//!
//! thin bridge over revenant-core: each recovery step runs on a blocking
//! thread and streams its progress lines back to the webview as `rv:log` events

use std::path::PathBuf;

use revenant_core::commands::{self, Ctx};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

/// run configuration sent from the frontend for a single invocation
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunParams {
    backup: String,
    workspace: String,
    password: String,
    step: String,
    count_only: bool,
    force: bool,
    include_all: bool,
}

/// run one step (or the full pipeline) off the ui thread
///
/// progress lines arrive as `rv:log` events; the returned result reports the
/// final outcome so the caller can await success or an error message
#[tauri::command]
async fn run_recovery(app: AppHandle, params: RunParams) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || run_blocking(app, params))
        .await
        .map_err(|e| e.to_string())?
}

fn run_blocking(app: AppHandle, params: RunParams) -> Result<(), String> {
    let sink = app.clone();
    let ctx = Ctx {
        backup: PathBuf::from(&params.backup),
        work: PathBuf::from(&params.workspace),
        password: params.password,
        count_only: params.count_only,
        force: params.force,
        include_all: params.include_all,
        report: Some(Box::new(move |line: &str| {
            let _ = sink.emit("rv:log", line.to_string());
        })),
    };

    let result = match params.step.as_str() {
        "decrypt" => commands::decrypt(&ctx),
        "carve" => commands::carve(&ctx),
        "thumbnails" => commands::thumbnails(&ctx),
        "media" => commands::media(&ctx),
        "all" => commands::decrypt(&ctx)
            .and_then(|_| commands::carve(&ctx))
            .and_then(|_| commands::thumbnails(&ctx))
            .and_then(|_| commands::media(&ctx)),
        other => return Err(format!("unknown step: {other}")),
    };
    result.map_err(|e| format!("{e:#}"))
}

/// a sensible default workspace under the user's home directory
#[tauri::command]
fn default_workspace(app: AppHandle) -> Result<String, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    Ok(home.join("Revenant").to_string_lossy().into_owned())
}

/// reveal a recovered file or folder in the system file manager
#[tauri::command]
fn reveal(app: AppHandle, path: String) -> Result<(), String> {
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![run_recovery, default_workspace, reveal])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
