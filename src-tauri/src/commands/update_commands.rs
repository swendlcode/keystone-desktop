use serde::{Deserialize, Serialize};

use crate::error::Result;

/// The legacy version ping. The real updater is `tauri-plugin-updater`, driven
/// from the frontend; this exists so an older build can still be told that a
/// newer one exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub notes: Option<String>,
}

/// Reports the running version and nothing else.
///
/// There is deliberately no HTTP client in the crate: the plugin already fetches
/// and verifies a signed `latest.json`, and a second, unsigned path to "is there
/// an update" would be a network call with none of the signature checking that
/// makes the real one safe. `latestVersion` stays null until the frontend has
/// asked the plugin.
#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo> {
    Ok(UpdateInfo {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: None,
        update_available: false,
        notes: None,
    })
}
