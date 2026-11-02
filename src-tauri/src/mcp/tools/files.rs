//! The tools that touch files on disk: renaming in place, and copying out.

use serde_json::{json, Value};

use super::{bool_arg, ids_arg, str_arg};
use crate::commands::collection_commands::write_collection_csv;
use crate::commands::track_commands::copy_track_to;
use crate::core::renamer;
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;

/// Rename files to the user's configured pattern.
///
/// One tool rather than the window's two steps. `preview_rename` and
/// `rename_tracks` are separate commands because a modal shows the plan and a
/// button applies it; an agent has no modal, so this plans and then applies
/// behind `dryRun` — which defaults to true, because the plan is the thing
/// worth looking at before a hundred files move.
///
/// The plan is always returned, applied or not. Its `skipped` reasons are the
/// interesting half: a collision, an illegal component on Windows, or no key
/// and tempo to write yet.
pub fn rename(ctx: &McpContext, args: &Value) -> Result<Value> {
    let ids = ids_arg(args, "trackIds")?;
    let dry_run = bool_arg(args, "dryRun", true);

    let tracks = ctx.tracks.get_many(&ids)?;
    if tracks.is_empty() {
        return Err(KeystoneError::NotFound(
            "none of those track ids are in the library".into(),
        ));
    }
    let plan = renamer::plan(&tracks, &ctx.settings)?;

    let items: Vec<Value> = plan
        .items
        .iter()
        .map(|item| {
            json!({
                "trackId": item.track_id,
                "from": item.from_path,
                "to": item.to_path,
                "skipped": item.skipped,
            })
        })
        .collect();

    if dry_run {
        let would = plan.items.iter().filter(|i| i.skipped.is_none()).count();
        return Ok(json!({
            "dryRun": true,
            "wouldRename": would,
            "skipped": plan.items.len() - would,
            "plan": items,
            "note": "nothing moved; call again with dryRun false to apply",
        }));
    }

    let report = renamer::apply(&plan, &ctx.tracks)?;
    Ok(json!({
        "dryRun": false,
        "renamed": report.renamed,
        "skipped": report.skipped,
        "failed": report.failed,
        "failures": serde_json::to_value(report.failures)?,
        "plan": items,
    }))
}

/// Copy tracks into a folder, leaving the originals where they are.
///
/// Per-file failures do not abandon the rest: a copy is non-destructive, so
/// carrying on is the safe direction, and one unreadable file must not cost
/// the other four hundred.
pub fn export(ctx: &McpContext, args: &Value) -> Result<Value> {
    let ids = ids_arg(args, "trackIds")?;
    let dest = str_arg(args, "destinationFolder")?;
    if !std::path::Path::new(&dest).is_dir() {
        return Err(KeystoneError::NotFound(format!(
            "{dest} is not a folder that exists"
        )));
    }

    let mut copied = Vec::new();
    let mut failures = Vec::new();
    for id in &ids {
        match copy_track_to(&ctx.tracks, id, &dest) {
            Ok(path) => copied.push(path),
            Err(e) => failures.push(json!({ "trackId": id, "reason": e.to_string() })),
        }
    }

    Ok(json!({
        "copied": copied.len(),
        "failed": failures.len(),
        "files": copied,
        "failures": failures,
    }))
}

pub fn export_csv(ctx: &McpContext, args: &Value) -> Result<Value> {
    let collection_id = str_arg(args, "collectionId")?;
    let dest = str_arg(args, "destinationPath")?;
    let rows = write_collection_csv(&ctx.tracks, collection_id, &dest)?;
    Ok(json!({ "rows": rows, "path": dest }))
}
