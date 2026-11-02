pub mod analysis;
pub mod bridge;
pub mod commands;
pub mod core;
pub mod db;
pub mod error;
pub mod events;
pub mod export;
#[macro_use]
pub mod handlers;
pub mod mcp;
pub mod models;
pub mod state;
pub mod tags;

/// Dev-only. Never part of a shipped bundle.
#[cfg(feature = "eval")]
pub mod eval;

use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `tauri=warn` matters more than it looks. Tauri reports its own
    // failures through the `log` crate — the asset protocol's "not
    // configured to allow the path", "File does not exist at path" and
    // "Missing OS permission" among them — and a filter naming only this
    // crate threw all of them away before they reached the terminal. An
    // audio file that would not play was therefore silent on both sides:
    // nothing in the webview console, nothing in the dev log.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "keystone_lib=info,tauri=warn,wry=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::init(app.handle().clone())?;
            recover_renames(&state);
            resume_work(&state);
            // The agent-facing bridge. Loopback and token-guarded; failing to
            // bind is logged and never stops the window from opening.
            bridge::start(app.handle().clone(), state.data_dir.clone());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(keystone_invoke_handler!())
        .build(tauri::generate_context!())
        .expect("error while building keystone")
        // Built and then run, rather than `.run(context)`, only so there is
        // somewhere to hook Exit: the MCP bridge's handshake names a port
        // that stops answering when the app does, and leaving the token file
        // behind would point the next `keystone-mcp` at a dead socket. The
        // client probes anyway, so this is tidiness rather than correctness.
        .run(|handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(state) = handle.try_state::<AppState>() {
                    bridge::stop(&state.data_dir);
                }
            }
        });
}

/// Replays any rename journal left behind by a crash, before anything reads a
/// path. Renaming is the only irreversible operation in the app, so recovery
/// runs first or the table points at files that have already moved.
fn recover_renames(state: &AppState) {
    let tracks = state.tracks.clone();
    // Long work started from `setup` uses Tauri's managed runtime.
    // `tokio::spawn` here panics with "no reactor running": the Tokio runtime is
    // not live yet at this point in startup.
    tauri::async_runtime::spawn(async move {
        let joined =
            tauri::async_runtime::spawn_blocking(move || core::renamer::recover_journal(&tracks))
                .await;
        match joined {
            Ok(Ok(n)) if n > 0 => tracing::info!("recovered {} rename(s)", n),
            Ok(Err(e)) => tracing::warn!("rename recovery failed: {}", e),
            Err(e) => tracing::warn!("rename recovery panicked: {}", e),
            _ => {}
        }
    });
}

/// Reconciles against the filesystem, then refills the analysis queue.
///
/// The queue is resumable across restarts because a 20,000-track library is
/// hours of work and quitting the app must not mean starting again. Resume is a
/// single indexed query, so it costs nothing at launch.
fn resume_work(state: &AppState) {
    let reconciler = state.reconciler.clone();
    let library = state.library.clone();
    let queue = state.queue.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let roots: Vec<String> = library
                .list_watched()
                .unwrap_or_default()
                .into_iter()
                .filter(|w| w.is_active)
                .map(|w| w.path)
                .collect();
            if let Err(e) = reconciler.run(&roots) {
                tracing::warn!("reconciliation failed: {}", e);
            }
            if let Err(e) = queue.enqueue_pending() {
                tracing::warn!("queue resume failed: {}", e);
            }
        })
        .await;
    });
}
