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

/// Writes a collection out as an `.m3u8` playlist, in its stored order.
///
/// `#EXTM3U` with an `#EXTINF` per entry, and **absolute paths**. A relative
/// playlist is the portable one, but it is also what put `../` into fifty
/// stored paths when it came back in (see `core::paths`), and a playlist
/// written for a DJ to load on this machine is pointing at this machine's
/// library. `.m3u8` rather than `.m3u`: the extension is the only thing that
/// tells a player the file is UTF-8, and track titles here are full of
/// accents and `&`.
pub fn export_m3u(ctx: &McpContext, args: &Value) -> Result<Value> {
    use std::fmt::Write as _;

    let collection_id = str_arg(args, "collectionId")?;
    let dest = str_arg(args, "destinationPath")?;
    let collection = ctx
        .collections
        .get(&collection_id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no collection with id {collection_id}")))?;

    let ordered = ctx.collections.track_ids(&collection_id)?;
    let rows = ctx.tracks.get_many(&ordered)?;
    let by_id: std::collections::HashMap<&str, &crate::models::Track> =
        rows.iter().map(|t| (t.id.as_str(), t)).collect();

    let mut out = String::from("#EXTM3U\n");
    let mut written = 0usize;
    let mut missing = 0usize;
    for id in &ordered {
        let Some(track) = by_id.get(id.as_str()) else {
            missing += 1;
            continue;
        };
        let seconds = track.duration_ms.map(|ms| ms / 1000).unwrap_or(-1);
        let artist = track.artist.as_deref().unwrap_or("");
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| track.filename.clone());
        let label = playlist_label(artist, &title);
        // A newline inside a tag would split one entry into two lines and
        // desynchronise every entry after it.
        let label = label.replace(['\n', '\r'], " ");
        let _ = writeln!(out, "#EXTINF:{seconds},{label}");
        let _ = writeln!(out, "{}", track.path);
        written += 1;
    }

    let path = with_m3u8_extension(&dest);
    std::fs::write(&path, out)?;

    let mut answer = json!({
        "path": path,
        "tracks": written,
        "collection": collection.name,
    });
    if missing > 0 {
        answer["skipped"] = json!(missing);
        answer["note"] = json!("skipped entries are no longer in the library");
    }
    Ok(answer)
}

/// Appends the extension rather than replacing one: a set called
/// `Miami 2026 vol. 2` has a dot in it, and `set_extension` would cut it to
/// `Miami 2026 vol.m3u8`.
fn with_m3u8_extension(dest: &str) -> String {
    let lower = dest.to_lowercase();
    if lower.ends_with(".m3u8") || lower.ends_with(".m3u") {
        return dest.to_string();
    }
    format!("{dest}.m3u8")
}

/// `Artist - Title`, unless the title already says the artist.
///
/// A YouTube-sourced library has the uploader channel in the artist tag and
/// the whole `Artist - Title (Official Video)` in the title, so the naive join
/// produced `Chris Lake - Chris Lake - Turn Off The Lights ft. Alexis Roberts
/// (Official Video)` and `Defected Records - CamelPhat & Elderbrook 'Cola'`.
/// The prefix is dropped when the title already opens with it, compared
/// case-insensitively.
///
/// This is presentation only. Fixing the tags themselves is a different job:
/// it writes to the files, so it belongs behind a `dryRun` tool rather than
/// inside an export.
fn playlist_label(artist: &str, title: &str) -> String {
    if artist.is_empty() {
        return title.to_string();
    }
    // `get` rather than a slice: `title[..artist.len()]` panics when that
    // offset is not a char boundary, which an accented title makes reachable.
    let starts_with_artist = title
        .get(..artist.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(artist));
    if starts_with_artist {
        return title.to_string();
    }
    format!("{artist} - {title}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_that_already_names_the_artist_is_not_prefixed_again() {
        assert_eq!(
            playlist_label(
                "Chris Lake",
                "Chris Lake - Turn Off The Lights ft. Alexis Roberts (Official Video)"
            ),
            "Chris Lake - Turn Off The Lights ft. Alexis Roberts (Official Video)"
        );
    }

    #[test]
    fn an_unrelated_artist_tag_is_still_prefixed() {
        assert_eq!(
            playlist_label("Defected Records", "CamelPhat & Elderbrook 'Cola'"),
            "Defected Records - CamelPhat & Elderbrook 'Cola'"
        );
    }

    #[test]
    fn a_missing_artist_leaves_the_title_alone() {
        assert_eq!(playlist_label("", "Cola"), "Cola");
    }

    /// Case is not a reason to repeat the name.
    #[test]
    fn the_comparison_ignores_case() {
        assert_eq!(playlist_label("ANOTR", "AnotR - Like It"), "AnotR - Like It");
    }

    /// A title shorter than the artist tag must not panic on the slice.
    #[test]
    fn a_short_title_does_not_index_out_of_bounds() {
        assert_eq!(playlist_label("Becky Hill Official", "Cola"), "Becky Hill Official - Cola");
    }

    /// The artist tag's byte length can fall inside a multi-byte character of
    /// the title, which is a panic rather than a wrong answer.
    #[test]
    fn a_multibyte_title_does_not_panic() {
        // "Åre" is four bytes; slicing the title at four would land inside
        // the second character of "Ärger".
        assert_eq!(playlist_label("Åre", "Ärger"), "Åre - Ärger");
        assert_eq!(playlist_label("Röyksopp", "Röyksopp - Eple"), "Röyksopp - Eple");
        // Non-ASCII differing only by case is not matched by
        // `eq_ignore_ascii_case`, which is correct here: it falls back to
        // prefixing rather than guessing.
        assert_eq!(playlist_label("é", "É - x"), "é - É - x");
    }
}
