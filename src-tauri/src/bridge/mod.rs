//! The loopback bridge the MCP server talks to while the app is running.
//!
//! Why it exists at all: `keystone-mcp` can open the library directly, and
//! does when the app is closed. But a tag written straight into the database
//! while the window is open would not appear in it — react-query has no idea
//! anything changed — and the user would be looking at a stale row with no
//! way to know. Routing through the running app means the same invalidation
//! and progress events a click produces.
//!
//! It binds `127.0.0.1` only, and still requires a token. Loopback is not a
//! permission boundary on a shared machine: every local process can reach it,
//! and these tools move files. The token lives in the app's data directory at
//! 0600, which is the same bar the user's own library sits behind.
//!
//! The handshake file is a *hint*, not a promise. The app can die without
//! removing it, so the client probes `/mcp/health` and falls back to opening
//! the database itself when nothing answers.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::Result;
use crate::mcp::{self, McpContext};

/// Fixed rather than ephemeral, so the handshake file is a convenience and
/// not the only way to find the port.
const DEFAULT_PORT: u16 = 9871;
const HANDSHAKE_FILE: &str = "mcp_bridge.json";

#[derive(Serialize, Deserialize)]
pub struct Handshake {
    pub port: u16,
    pub token: String,
}

struct Bridge {
    app: AppHandle,
    token: String,
}

/// Start the bridge. Failing to bind is logged and otherwise ignored: another
/// Keystone already holding the port, or a locked-down machine, must not stop
/// the app from opening.
pub fn start(app: AppHandle, data_dir: PathBuf) {
    let token = new_token();
    let state = Arc::new(Bridge {
        app: app.clone(),
        token: token.clone(),
    });

    tauri::async_runtime::spawn(async move {
        let router = Router::new()
            .route("/mcp/health", get(health))
            .route("/mcp/call", post(call))
            .with_state(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("mcp bridge could not bind {addr}: {e}");
                return;
            }
        };

        if let Err(e) = write_handshake(&data_dir, DEFAULT_PORT, &token) {
            tracing::warn!("mcp bridge handshake could not be written: {e}");
        }
        tracing::info!("mcp bridge listening on {addr}");

        if let Err(e) = axum::serve(listener, router).await {
            tracing::warn!("mcp bridge stopped: {e}");
        }
    });
}

/// Remove the handshake, so a later `keystone-mcp` does not try a dead port
/// before falling back.
pub fn stop(data_dir: &Path) {
    let _ = std::fs::remove_file(data_dir.join(HANDSHAKE_FILE));
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "app": "keystone", "version": env!("CARGO_PKG_VERSION") }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallBody {
    tool: String,
    #[serde(default)]
    args: Value,
}

async fn call(
    State(bridge): State<Arc<Bridge>>,
    headers: HeaderMap,
    Json(body): Json<CallBody>,
) -> (StatusCode, Json<Value>) {
    if !authorized(&headers, &bridge.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "bad or missing token" })),
        );
    }

    let app = bridge.app.clone();
    // Every tool is a database read or a file write, so none of them belong on
    // the async runtime's threads.
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::state::AppState>();
        let ctx = McpContext::from_state(&state);
        mcp::dispatch(&ctx, &body.tool, &body.args).map_err(|e| e.to_string())
    })
    .await;

    match outcome {
        Ok(Ok(value)) => (StatusCode::OK, Json(json!({ "result": value }))),
        // A tool that refused is a 200 with an error in it: the refusal is the
        // answer, and the client turns it into something the model can read.
        Ok(Err(message)) => (StatusCode::OK, Json(json!({ "error": message }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("tool panicked: {e}") })),
        ),
    }
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let presented = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-keystone-token").and_then(|v| v.to_str().ok()));

    // Length-first, then a byte-by-byte compare that does not return early.
    // The token is long and random so a timing attack on it is theoretical,
    // but `==` on a secret is the habit worth not having.
    match presented {
        Some(p) if p.len() == token.len() => p
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0,
        _ => false,
    }
}

fn new_token() -> String {
    // Two v4 UUIDs: 256 bits from the same CSPRNG `getrandom` gives, without
    // adding a dependency for it.
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn write_handshake(data_dir: &Path, port: u16, token: &str) -> Result<()> {
    let path = data_dir.join(HANDSHAKE_FILE);
    std::fs::write(
        &path,
        serde_json::to_vec(&Handshake {
            port,
            token: token.to_owned(),
        })?,
    )?;
    restrict(&path);
    Ok(())
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {
    // Windows: the app data directory is already per-user.
}

/// Read the handshake, for the client side.
pub fn read_handshake(data_dir: &Path) -> Option<Handshake> {
    let raw = std::fs::read(data_dir.join(HANDSHAKE_FILE)).ok()?;
    serde_json::from_slice(&raw).ok()
}
