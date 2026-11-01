//! Tauri handlers. Thin by rule: validate input, call a core service or a repo,
//! return. No SQL and no business logic in this directory.

pub mod analysis_commands;
pub mod collection_commands;
pub mod cue_commands;
pub mod export_commands;
pub mod import_commands;
pub mod library_commands;
pub mod player_commands;
pub mod settings_commands;
pub mod tag_commands;
mod tag_pass;
pub mod track_commands;
pub mod update_commands;

use crate::error::{KeystoneError, Result};

/// Runs a command's blocking work off the async runtime.
///
/// Every database call and every file read goes through here. SQLite's busy
/// timeout is ten seconds, so one query run on the runtime's thread can stall
/// every other command behind it — which the user sees as a frozen window
/// rather than a slow one.
pub(crate) async fn blocking<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(f).await {
        Ok(inner) => inner,
        // The task panicked. Reporting it as an error keeps the panic inside
        // the one command that caused it instead of taking the window with it.
        Err(e) => Err(KeystoneError::Tauri(e)),
    }
}
