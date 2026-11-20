//! The rekordbox document, for USB prep.
//!
//! The exporter itself is [`crate::export`] and is not duplicated here: the
//! refusal while rekordbox is running, the dated backup and the atomic write
//! are in one place on purpose, and a second path into the writer would be a
//! second chance to skip them.
//!
//! Two decisions specific to the agent surface:
//!
//! * **The destination is always named by the caller.** The window's export
//!   can fall back to the detected rekordbox library, which is the right
//!   default for a cue export a user asked for by pressing a button. A tool
//!   that did the same could overwrite a DJ's actual collection on a call
//!   that read like a harmless export, so there is no fallback here.
//! * **The playlist is named after the collection.** A set arriving in
//!   rekordbox as `Keystone Cue Points` is no use to somebody prepping four
//!   of them for a USB stick.

use std::path::Path;

use serde_json::{json, Value};

use super::{bool_arg, ids_arg, opt_str, str_arg};
use crate::analysis::camelot;
use crate::error::{KeystoneError, Result};
use crate::export::{self, ExportContext, ExportRequest, ExportTarget, ExportTrack};
use crate::mcp::McpContext;

pub fn export_xml(ctx: &McpContext, args: &Value) -> Result<Value> {
    let dest = with_xml_extension(&str_arg(args, "destinationPath")?);
    let (ids, name) = scope(ctx, args)?;
    if ids.is_empty() {
        return Err(KeystoneError::NotFound(
            "nothing to export: name a collection with tracks in it, or trackIds".into(),
        ));
    }

    let tracks = assemble(ctx, &ids)?;
    let request = ExportRequest {
        target: ExportTarget::Rekordbox,
        track_ids: ids.clone(),
        overwrite_existing: true,
        dry_run: bool_arg(args, "dryRun", false),
        output_path: Some(dest.clone()),
        run_id: uuid::Uuid::new_v4().to_string(),
        playlist_name: opt_str(args, "playlistName").or(name),
        include_energy: bool_arg(args, "includeEnergy", true),
    };
    let context = ExportContext {
        backups_dir: &ctx.backups_dir,
        settings: &ctx.settings,
    };

    let report = if request.dry_run {
        export::dry_run(&request, &tracks, &context)?
    } else {
        export::export(&request, &tracks, &context)?
    };

    let mut out = json!({
        "path": report.destination.clone().unwrap_or(dest),
        "dryRun": request.dry_run,
        "tracks": report.written.max(report.tracks_matched),
        "cues": report.cues_to_write,
        "playlist": request.playlist_name,
        "energyInComments": request.include_energy,
        "note": "import it in rekordbox with File, Import Collection — rekordbox \
cannot be written into directly, so nothing here touches its library",
    });
    if report.tracks_missing > 0 {
        out["skipped"] = json!(report.tracks_missing);
        out["skippedNote"] = json!("these tracks' files are no longer at their stored paths");
        out["failures"] = serde_json::to_value(&report.failures)?;
    }
    if let Some(backup) = report.backup_path {
        out["backupOfPreviousFile"] = json!(backup);
    }
    Ok(out)
}

/// Which tracks, and what the playlist should be called.
///
/// A collection exports in **playing order**, which is the order somebody put
/// it in and the only order worth sending to a USB stick.
fn scope(ctx: &McpContext, args: &Value) -> Result<(Vec<String>, Option<String>)> {
    if let Some(id) = opt_str(args, "collectionId") {
        let collection = ctx
            .collections
            .get(&id)?
            .ok_or_else(|| KeystoneError::NotFound(format!("no collection with id {id}")))?;
        return Ok((ctx.collections.track_ids(&id)?, Some(collection.name)));
    }
    Ok((ids_arg(args, "trackIds")?, None))
}

/// The projection the exporters read. The same assembly the window's command
/// does, including formatting the key once under the user's Key Notation
/// setting so no exporter re-decides notation.
fn assemble(ctx: &McpContext, ids: &[String]) -> Result<Vec<ExportTrack>> {
    let mut out = Vec::with_capacity(ids.len());
    // `get_many` does not preserve the order it was given, and the playlist
    // order is the point here, so the rows are put back in order of ids.
    let rows = ctx.tracks.get_many(ids)?;
    let by_id: std::collections::HashMap<&str, &crate::models::Track> =
        rows.iter().map(|t| (t.id.as_str(), t)).collect();

    for id in ids {
        let Some(track) = by_id.get(id.as_str()) else {
            continue;
        };
        let key_string = match (track.key_root, track.key_mode) {
            (Some(root), Some(mode)) => camelot::key_string(
                root,
                mode,
                ctx.settings.key_notation,
                ctx.settings.pad_camelot_numbers,
            ),
            _ => String::new(),
        };
        out.push(ExportTrack {
            cues: ctx.cues.list_for_track(&track.id)?,
            beatgrid: ctx.cues.beatgrid(&track.id)?,
            key_string,
            track: (*track).clone(),
        });
    }
    Ok(out)
}

/// Appends rather than replaces, for the same reason the m3u export does: a
/// set called `Kava Villa Nov 19 vol. 2` has a dot in it and `set_extension`
/// would cut the name off at it.
fn with_xml_extension(dest: &str) -> String {
    if Path::new(dest)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
    {
        return dest.to_string();
    }
    format!("{dest}.xml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_appended_not_substituted() {
        assert_eq!(
            with_xml_extension("/sets/Kava Villa vol. 2"),
            "/sets/Kava Villa vol. 2.xml"
        );
        assert_eq!(with_xml_extension("/sets/set.xml"), "/sets/set.xml");
        assert_eq!(with_xml_extension("/sets/set.XML"), "/sets/set.XML");
    }
}
