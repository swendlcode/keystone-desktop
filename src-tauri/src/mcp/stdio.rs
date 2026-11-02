//! MCP over stdin and stdout.
//!
//! Newline-delimited JSON-RPC 2.0, which is the whole of the stdio transport.
//! Hand-written rather than taken from a crate because the surface actually
//! used is four methods, and the one rule that matters is not in any of them:
//!
//! **nothing but protocol may be written to stdout.** A `println!` left in a
//! tool, or a tracing layer defaulting to stdout, corrupts the stream and the
//! client drops the connection with no useful error. Logging therefore goes
//! to stderr, which the client shows as the server's log.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use super::catalog;

/// The spec revision this speaks. Clients send their own; a mismatch is not
/// fatal, so the server answers with what it implements and lets the client
/// decide.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Read requests until stdin closes.
///
/// `call` is injected so the binary can decide per request whether to post to
/// the running app or to run the tool itself, without this loop knowing which
/// happened.
pub fn serve_stdio<F>(mut call: F) -> std::io::Result<()>
where
    F: FnMut(&str, &Value) -> Result<Value, String>,
{
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                // -32700, parse error. No id is knowable, so the spec says
                // answer with a null one rather than staying silent.
                write(&mut out, &error_response(Value::Null, -32700, &e.to_string()))?;
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        // A notification carries no id and must not be answered at all.
        let Some(id) = id else {
            continue;
        };

        let response = match method {
            "initialize" => success(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "keystone",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
            ),
            "ping" => success(id, json!({})),
            "tools/list" => success(id, json!({ "tools": catalog::tool_list() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match call(name, &args) {
                    Ok(value) => success(id, tool_result(&value, false)),
                    // A failed tool is a *result* with `isError`, not a
                    // JSON-RPC error: the model has to be able to read the
                    // sentence and try again, and a protocol error never
                    // reaches it.
                    Err(message) => success(id, tool_result(&Value::String(message), true)),
                }
            }
            other => error_response(
                id,
                -32601,
                &format!("method {other} is not implemented"),
            ),
        };

        write(&mut out, &response)?;
    }

    Ok(())
}

/// Tool output is text content. JSON is pretty-printed into it: the model
/// reads this, and a single-line blob of forty fields is measurably harder
/// for it to quote back correctly.
fn tool_result(value: &Value, is_error: bool) -> Value {
    let text = match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn write(out: &mut impl Write, value: &Value) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    // Unbuffered in effect: the client is waiting on this line before it
    // sends anything else, so a held buffer is a deadlock.
    out.flush()
}
