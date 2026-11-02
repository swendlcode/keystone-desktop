//! `keystone-mcp` — Keystone's library, as MCP tools.
//!
//! Register it with any MCP client:
//!
//! ```json
//! { "mcpServers": { "keystone": { "command": "/path/to/keystone-mcp" } } }
//! ```
//!
//! Each call takes whichever route is available, decided per call rather than
//! once at startup — the user can open or quit Keystone mid-session and
//! nothing here needs restarting:
//!
//! 1. **The running app**, over its loopback bridge, so writes land in the
//!    open window rather than behind it.
//! 2. **The database directly**, when nothing answers the probe.
//!
//! Everything diagnostic goes to stderr. stdout carries the protocol and
//! nothing else; a stray line on it corrupts the stream and the client drops
//! the connection without saying why.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{json, Value};

use keystone_lib::bridge::{self, Handshake};
use keystone_lib::mcp::{self, McpContext};

/// Long enough for a tag pass over a few hundred files, short enough that a
/// wedged app does not hang the client for ever.
const CALL_TIMEOUT: Duration = Duration::from_secs(300);
/// The probe is only asking whether anything is listening.
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

fn main() {
    // stderr, deliberately. See the module note.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "keystone_lib=warn,keystone_mcp=info".into()),
        )
        .init();

    let data_dir = match mcp::data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("keystone-mcp: {e}");
            std::process::exit(1);
        }
    };

    // Built lazily and kept: opening the pool costs a migration check, and a
    // session that never needs the fallback should never pay for it.
    let mut local: Option<McpContext> = None;

    let result = mcp::serve_stdio(|tool, args| {
        if let Some(handshake) = bridge::read_handshake(&data_dir).filter(reachable) {
            match remote_call(&handshake, tool, args) {
                Ok(value) => return Ok(value),
                Err(e) => {
                    // The app answered the probe and then failed the call.
                    // Falling through to the database would be worse than
                    // saying so: the two paths disagree about whether the
                    // open window has seen the change.
                    tracing::warn!("bridge call failed: {e}");
                    return Err(format!("Keystone is running but refused the call: {e}"));
                }
            }
        }

        if local.is_none() {
            tracing::info!("Keystone is not running; opening the library directly");
            local = Some(McpContext::standalone().map_err(|e| e.to_string())?);
        }
        let ctx = local.as_ref().expect("just built");
        mcp::dispatch(ctx, tool, args).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        tracing::error!("stdio transport stopped: {e}");
        std::process::exit(1);
    }
}

/// Is anything actually listening on the port the handshake names?
fn reachable(handshake: &Handshake) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], handshake.port));
    TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok()
}

/// One POST to the bridge.
///
/// Written against `TcpStream` rather than pulling in an HTTP client. The
/// request goes to loopback, the server is this same crate, and the response
/// shape is known — so the parts of HTTP that justify a dependency (TLS,
/// redirects, chunked transfer from an unknown peer, connection reuse) are
/// all absent. The server never chunks: `axum::Json` sets a content length.
fn remote_call(handshake: &Handshake, tool: &str, args: &Value) -> Result<Value, String> {
    let body = serde_json::to_vec(&json!({ "tool": tool, "args": args }))
        .map_err(|e| e.to_string())?;

    let addr = SocketAddr::from(([127, 0, 0, 1], handshake.port));
    let mut stream =
        TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(CALL_TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(CALL_TIMEOUT))
        .map_err(|e| e.to_string())?;

    let request = format!(
        "POST /mcp/call HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Authorization: Bearer {token}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n",
        port = handshake.port,
        token = handshake.token,
        len = body.len(),
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .and_then(|()| stream.flush())
        .map_err(|e| e.to_string())?;

    // `Connection: close` means end-of-stream is end-of-body, so the length
    // header does not have to be parsed back out.
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;

    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the bridge sent no headers")?;
    let status = std::str::from_utf8(&raw[..split])
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let payload: Value =
        serde_json::from_slice(&raw[split + 4..]).map_err(|e| format!("{status}: {e}"))?;

    if let Some(message) = payload.get("error").and_then(Value::as_str) {
        return Err(message.to_owned());
    }
    payload
        .get("result")
        .cloned()
        .ok_or_else(|| format!("the bridge answered without a result ({status})"))
}
