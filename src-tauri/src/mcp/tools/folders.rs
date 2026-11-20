//! Watched folders, and starting the app when a tool needs it.
//!
//! Scanning lives in the app: the indexer's two stages and the analysis queue
//! are owned by the window, and a scan that only wrote rows would leave the
//! library un-analysed while reporting success. So these go through
//! `require_live` and `launch_app` is how an agent satisfies that — the
//! sentence an unsatisfied call returns names it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{bool_arg, str_arg};
use crate::commands::library_commands::{spawn_analysis_after_indexing, spawn_scan, to_jobs};
use crate::core::Scanner;
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::state::AppState;

/// How long `launch_app` waits for the window it started to answer its bridge.
/// A cold launch opens a database, runs the migration check, reconciles and
/// paints; 30s is generous for that and still bounded.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const LAUNCH_POLL: Duration = Duration::from_millis(250);

/// Adds a root and indexes it, exactly as the Add Tracks button does.
pub fn add_watched(ctx: &McpContext, args: &Value) -> Result<Value> {
    let path = str_arg(args, "path")?;
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(KeystoneError::NotFound(format!("{path} is not a folder")));
    }
    let state = app_state(ctx, "adding a watched folder")?;

    // Before the upsert, the same order the command uses: a scan still in
    // flight for this root must not be excluded when we are about to add it.
    state.indexer.unexclude(&root);
    let folder = state.library.add_watched(&path)?;
    state.watcher.watch(&root)?;
    spawn_scan(state.indexer.clone(), root, state.queue.clone());

    Ok(json!({
        "folder": serde_json::to_value(folder)?,
        "note": "indexing started in the background; library_stats or list_tracks will fill in as it runs",
    }))
}

/// Removes a root. The tracks under it go missing rather than being deleted,
/// which is what the app does and what keeps ratings and cues for a drive that
/// comes back.
pub fn remove_watched(ctx: &McpContext, args: &Value) -> Result<Value> {
    let path = str_arg(args, "path")?;
    let state = app_state(ctx, "removing a watched folder")?;
    let root = PathBuf::from(&path);

    // `exclude` first: a scan started earlier can still have rows in flight,
    // and they would otherwise land after the delete and resurrect the folder.
    state.indexer.exclude(&root);
    state.watcher.unwatch(&root)?;
    let removed = state.library.remove_watched(&path)?;

    Ok(json!({
        "removed": removed,
        "note": "tracks under it are marked missing, not deleted; their ratings, cues and analysis are kept",
    }))
}

/// Walks a root again and queues what it finds.
///
/// This is the tool that was missing: a folder could be added by hand and then
/// nothing could make Keystone look at it again without the window.
pub fn rescan(ctx: &McpContext, args: &Value) -> Result<Value> {
    let state = app_state(ctx, "rescanning")?;

    let roots: Vec<String> = match super::opt_str(args, "path") {
        Some(one) => vec![one],
        None => state
            .library
            .list_watched()?
            .into_iter()
            .filter(|w| w.is_active)
            .map(|w| w.path)
            .collect(),
    };
    if roots.is_empty() {
        return Err(KeystoneError::NotFound(
            "no watched folders to rescan; add one with add_watched_folder".into(),
        ));
    }

    // The repair runs first for the same reason it does at launch: a row
    // stored with a `..` in its path sits outside the root that owns it, so a
    // scan would index the same file a second time rather than find the row.
    let repaired = crate::core::path_repair::run(&state.tracks)?;

    let mut found = 0usize;
    state.indexer.reset_counters();
    for root in &roots {
        let (files, _stats) = Scanner::scan(&PathBuf::from(root))?;
        let jobs = to_jobs(files);
        found += jobs.len();
        state.indexer.enqueue_batch(jobs);
    }
    spawn_analysis_after_indexing(state.indexer.clone(), state.queue.clone());

    let mut out = json!({ "folders": roots, "filesFound": found });
    if !repaired.is_empty() {
        out["pathsRepaired"] = json!(repaired.repaired);
        out["duplicatesDropped"] = json!(repaired.deduped);
    }
    Ok(out)
}

/// Starts Keystone and waits until its bridge answers.
///
/// The binary that serves these tools lives inside the app bundle, so the app
/// is found from this executable's own path rather than from a guess at
/// /Applications — a user who keeps it elsewhere, or a dev build, still works.
pub fn launch_app(ctx: &McpContext, args: &Value) -> Result<Value> {
    if ctx.live.is_some() {
        return Ok(json!({
            "running": true,
            "note": "Keystone is already running; this call went through it",
        }));
    }

    let target = app_executable()?;
    spawn(&target)?;

    if !bool_arg(args, "wait", true) {
        return Ok(json!({
            "running": false,
            "launched": target.to_string_lossy(),
            "note": "started without waiting; the next call may still find it coming up",
        }));
    }

    let clock = Instant::now();
    while clock.elapsed() < LAUNCH_TIMEOUT {
        if bridge_answers(&ctx.data_dir) {
            return Ok(json!({
                "running": true,
                "launched": target.to_string_lossy(),
                "readyInMs": clock.elapsed().as_millis(),
            }));
        }
        std::thread::sleep(LAUNCH_POLL);
    }

    Err(KeystoneError::Other(format!(
        "started {} but it did not answer within {}s",
        target.display(),
        LAUNCH_TIMEOUT.as_secs()
    )))
}

/// Is the app's loopback bridge up? The handshake file is a hint — the app can
/// die without removing it — so the port is probed rather than trusted.
fn bridge_answers(data_dir: &std::path::Path) -> bool {
    let Some(handshake) = crate::bridge::read_handshake(data_dir) else {
        return false;
    };
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], handshake.port));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

/// macOS: the `.app` two levels above this binary. Elsewhere, and for a dev
/// build, the main binary sitting beside it.
fn app_executable() -> Result<PathBuf> {
    let exe = std::env::current_exe()
        .map_err(|e| KeystoneError::Other(format!("cannot locate this executable: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| KeystoneError::Other("this executable has no folder".into()))?;

    #[cfg(target_os = "macos")]
    if let Some(bundle) = dir.parent().and_then(|c| c.parent()) {
        if bundle.extension().and_then(|e| e.to_str()) == Some("app") {
            return Ok(bundle.to_path_buf());
        }
    }

    let name = if cfg!(windows) { "keystone.exe" } else { "keystone" };
    let sibling = dir.join(name);
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err(KeystoneError::NotFound(format!(
        "could not find Keystone next to {}; open the app yourself",
        exe.display()
    )))
}

fn spawn(target: &std::path::Path) -> Result<()> {
    let mut command = if target.extension().and_then(|e| e.to_str()) == Some("app") {
        // `open` hands it to the window server, which is what gives a GUI app
        // a session; spawning the inner binary directly does not.
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg("-a").arg(target);
        c
    } else {
        std::process::Command::new(target)
    };

    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| KeystoneError::Other(format!("could not start {}: {e}", target.display())))?;
    Ok(())
}

/// The app's own state, or the sentence that says how to get it.
pub(super) fn app_state<'a>(ctx: &'a McpContext, what: &str) -> Result<tauri::State<'a, AppState>> {
    use tauri::Manager;
    let live = ctx.live.as_ref().ok_or_else(|| {
        KeystoneError::Other(format!(
            "{what} needs Keystone to be running: call launch_app first, or open the app"
        ))
    })?;
    Ok(live.app.state::<AppState>())
}
