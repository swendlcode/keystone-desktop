//! One function per tool, and the table that reaches them.
//!
//! The tools are deliberately coarser than the Tauri commands they sit over.
//! A command exists to serve one control in the window, so `rename_tracks`
//! takes a plan the UI already previewed; an agent has no previewed plan and
//! no modal, so the tool here plans and applies in one call behind a
//! `dryRun` flag. Mapping tools one-to-one onto commands would have handed
//! the agent a protocol to orchestrate rather than a job to do.

use serde_json::{json, Value};

use super::McpContext;
use crate::error::{KeystoneError, Result};

mod collections;
mod files;
mod library;
mod tags;

pub fn dispatch(ctx: &McpContext, tool: &str, args: &Value) -> Result<Value> {
    match tool {
        // Reading the library
        "list_tracks" => library::list_tracks(ctx, args),
        "get_track" => library::get_track(ctx, args),
        "list_genres" => library::list_genres(ctx),
        "library_stats" => library::library_stats(ctx),
        "list_watched_folders" => library::list_watched_folders(ctx),

        // Correcting what the engine decided
        "set_key" => library::set_key(ctx, args),
        "set_tempo" => library::set_tempo(ctx, args),
        "set_energy" => library::set_energy(ctx, args),
        "set_rating" => library::set_rating(ctx, args),
        "analyze_tracks" => library::analyze_tracks(ctx, args),

        // Collections
        "list_collections" => collections::list(ctx),
        "create_collection" => collections::create(ctx, args),
        "rename_collection" => collections::rename(ctx, args),
        "delete_collection" => collections::delete(ctx, args),
        "add_to_collection" => collections::add_tracks(ctx, args),
        "remove_from_collection" => collections::remove_tracks(ctx, args),

        // Tags, written into the files themselves
        "get_tags" => tags::get_tags(ctx, args),
        "set_tags" => tags::set_tags(ctx, args),
        "find_replace_tags" => tags::find_replace(ctx, args),
        "remove_extra_spaces" => tags::remove_extra_spaces(ctx, args),
        "write_key_and_tempo_tags" => tags::retag(ctx, args),
        "list_tag_backups" => tags::list_backups(ctx, args),
        "restore_tag_backup" => tags::restore_backup(ctx, args),

        // Files on disk
        "rename_files" => files::rename(ctx, args),
        "export_files" => files::export(ctx, args),
        "export_collection_csv" => files::export_csv(ctx, args),

        other => Err(KeystoneError::NotFound(format!("no tool named {other}"))),
    }
}

// ---------------------------------------------------------------------------
// Argument reading
//
// An agent gets these wrong in ways a UI cannot: a number as a string, an id
// where a list belongs, a key omitted entirely. Every miss has to come back
// as a sentence naming the argument, because that message is the only thing
// the model has to correct itself with.
// ---------------------------------------------------------------------------

pub fn missing(key: &str) -> KeystoneError {
    KeystoneError::Other(format!("`{key}` is required"))
}

pub fn str_arg(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| missing(key))
}

pub fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
}

/// A list of ids, or a single id, which an agent will send roughly half the
/// time no matter how the schema is written.
pub fn ids_arg(args: &Value, key: &str) -> Result<Vec<String>> {
    match args.get(key) {
        Some(Value::Array(items)) => {
            let ids: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            if ids.is_empty() {
                return Err(KeystoneError::Other(format!("`{key}` is empty")));
            }
            Ok(ids)
        }
        Some(Value::String(one)) => Ok(vec![one.clone()]),
        _ => Err(missing(key)),
    }
}

pub fn opt_f64(args: &Value, key: &str) -> Option<f64> {
    let v = args.get(key)?;
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

pub fn f64_arg(args: &Value, key: &str) -> Result<f64> {
    opt_f64(args, key).ok_or_else(|| missing(key))
}

pub fn opt_u8(args: &Value, key: &str) -> Option<u8> {
    let v = args.get(key)?;
    let n = v.as_u64().or_else(|| v.as_str()?.trim().parse().ok())?;
    u8::try_from(n).ok()
}

pub fn opt_usize(args: &Value, key: &str) -> Option<usize> {
    let v = args.get(key)?;
    let n = v.as_u64().or_else(|| v.as_str()?.trim().parse().ok())?;
    usize::try_from(n).ok()
}

pub fn bool_arg(args: &Value, key: &str, default: bool) -> bool {
    match args.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => default,
    }
}

pub fn strings_arg(args: &Value, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(one)) => vec![one.clone()],
        _ => Vec::new(),
    }
}

/// `{ "ok": true }`, for a tool whose answer is that it worked.
pub fn ok() -> Result<Value> {
    Ok(json!({ "ok": true }))
}
