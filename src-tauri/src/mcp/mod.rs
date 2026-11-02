//! Keystone's agent-facing surface.
//!
//! The same tools are reachable two ways, and which one is used is decided by
//! whether the app happens to be running:
//!
//! - **Through the app.** `keystone-mcp` posts the call to the bridge the
//!   running app serves on loopback, so a tag written or a track filed shows
//!   up in the window without a refresh.
//! - **Straight at the database.** With the app closed there is nothing to
//!   post to, so the binary opens the library itself. WAL gives concurrent
//!   readers and a single writer, and `busy_timeout` is already 10s, so this
//!   is safe even if the app opens mid-session.
//!
//! Both land in [`dispatch`]. That is the point of [`McpContext`]: the tools
//! are written once against the repositories, and the only difference between
//! the transports is which context they are handed. Two dispatch tables would
//! be two chances for the bridge and the standalone path to disagree about
//! what a tool does.
//!
//! What the standalone context cannot do is anything that needs the app
//! itself — queueing analysis, emitting progress to a webview. Those tools
//! say so rather than half-working: see [`McpContext::require_live`].

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::db::{
    CollectionRepository, DatabasePool, LibraryRepository, TrackRepository,
};
use crate::error::{KeystoneError, Result};
use crate::state::jobs::AnalysisQueue;
use crate::models::Settings;

mod catalog;
mod stdio;
mod tools;

pub use catalog::{tool_list, TOOL_NAMES};
pub use stdio::serve_stdio;

/// The bundle identifier, which is also the data directory's name.
const IDENTIFIER: &str = "app.keystone.desktop";

/// What only exists while the app is running.
pub struct Live {
    pub app: tauri::AppHandle,
    pub queue: Arc<AnalysisQueue>,
}

/// Everything a tool is allowed to reach.
pub struct McpContext {
    pub db: Arc<DatabasePool>,
    pub tracks: Arc<TrackRepository>,
    pub collections: Arc<CollectionRepository>,
    pub library: Arc<LibraryRepository>,
    /// A snapshot. Tags are written in the shape the user configured, and a
    /// tool must not read a different one halfway through a pass.
    pub settings: Settings,
    pub backups_dir: PathBuf,
    /// `None` when the app is not running.
    pub live: Option<Live>,
}

impl McpContext {
    /// Inside the running app, over its own state.
    pub fn from_state(state: &crate::state::AppState) -> Self {
        Self {
            db: state.db.clone(),
            tracks: state.tracks.clone(),
            collections: state.collections.clone(),
            library: state.library.clone(),
            settings: state.settings.read().clone(),
            backups_dir: state.backups_dir.clone(),
            live: Some(Live {
                app: state.app.clone(),
                queue: state.queue.clone(),
            }),
        }
    }

    /// Standalone, against the library the app would have opened.
    ///
    /// Refuses to create one. A missing database means Keystone has never run
    /// on this machine, and answering that by migrating an empty file into
    /// place would leave an agent reporting an empty library rather than a
    /// missing one.
    pub fn standalone() -> Result<Self> {
        let data = data_dir()?;
        let db_path = data.join("keystone.db");
        if !db_path.exists() {
            return Err(KeystoneError::NotFound(format!(
                "no Keystone library at {}; open Keystone once to create it",
                db_path.display()
            )));
        }
        let db = DatabasePool::open(&db_path)?;
        Ok(Self {
            tracks: Arc::new(TrackRepository::new(db.clone())),
            db: db.clone(),
            collections: Arc::new(CollectionRepository::new(db.clone())),
            library: Arc::new(LibraryRepository::new(db.clone())),
            settings: crate::state::jobs::settings::load(&data.join("settings.json")),
            backups_dir: data.join("backups"),
            live: None,
        })
    }

    /// The app handle, or the reason there isn't one.
    pub fn require_live(&self, what: &str) -> Result<&Live> {
        self.live.as_ref().ok_or_else(|| {
            KeystoneError::Other(format!(
                "{what} needs Keystone to be running: open the app and try again"
            ))
        })
    }
}

/// Run one tool call.
///
/// Blocking throughout. Every tool is a database read, a file write or both,
/// and the callers — the bridge's handler and the stdio loop — each put this
/// on a blocking thread of their own.
pub fn dispatch(ctx: &McpContext, tool: &str, args: &Value) -> Result<Value> {
    tools::dispatch(ctx, tool, args)
}

/// Where Tauri would have put the app's data, worked out without Tauri.
///
/// Public because the binary needs it before it has a context: the handshake
/// file lives there too.
pub fn data_dir() -> Result<PathBuf> {
    let base: Option<PathBuf> = {
        #[cfg(target_os = "macos")]
        {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("APPDATA").map(PathBuf::from)
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".local").join("share"))
                })
        }
    };

    base.map(|b| b.join(IDENTIFIER)).ok_or_else(|| {
        KeystoneError::Other("could not work out the data directory".into())
    })
}
