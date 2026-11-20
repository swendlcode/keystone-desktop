//! Rebuilding artist and title from the filename.
//!
//! The library this exists for came off YouTube: the artist tag is the
//! uploader channel — `Defected Records`, `Becky Hill Official`, `DIRTYBIRD`
//! — the genre is `People & Blogs` or `Music`, and the real credit is in the
//! filename and nowhere else. The parsing is [`crate::tags::filename`]; this
//! is the pass that decides what to write and reports it.
//!
//! Three rules about what it will and will not do:
//!
//! * **The current filename is not necessarily the source.** Keystone's own
//!   renamer may have composed it from the key and tempo, so the stem goes
//!   through `strip_stem` first, which inverts that exactly when
//!   `original_filename` is recorded.
//! * **A name with no `Artist - Title` separator writes nothing**, unless the
//!   title tag has one — which is the other half of the same mess, where the
//!   whole credit ended up in the title. Guessing an artist from a name that
//!   does not contain one is how a title becomes an artist.
//! * **`dryRun` defaults to true**, and the dry run returns the proposals
//!   themselves rather than a count. A count tells the caller nothing about
//!   whether the parse was right, which is the only question worth asking
//!   before fifty files are rewritten.

use std::sync::Mutex;

use serde_json::{json, Value};

use super::{bool_arg, ids_arg, opt_usize, str_arg};
use crate::commands::tag_pass::all_track_ids;
use crate::error::Result;
use crate::mcp::McpContext;
use crate::models::{TagEdit, TagField, Track};
use crate::tags::filename::{self, ParsedName};
use crate::tags::rename::compose::{stem_of, strip_stem};

/// How many proposals come back by default. Enough to judge the parse on,
/// short of a page of JSON for a whole library.
const DEFAULT_SAMPLE: usize = 25;

pub fn run(ctx: &McpContext, args: &Value) -> Result<Value> {
    let dry_run = bool_arg(args, "dryRun", true);
    let from_title = bool_arg(args, "useTitleTag", true);
    let genre = super::opt_str(args, "genre");
    let sample_limit = opt_usize(args, "sample").unwrap_or(DEFAULT_SAMPLE);
    let ids = match args.get("trackIds") {
        Some(_) => ids_arg(args, "trackIds")?,
        None => all_track_ids(&ctx.tracks)?,
    };

    let proposals: Mutex<Vec<Value>> = Mutex::new(Vec::new());
    let unparsed: Mutex<Vec<String>> = Mutex::new(Vec::new());

    let report = super::tags::pass_for(ctx, dry_run).run(&ids, |track, snapshot| {
        let Some(parsed) = source_name(track, snapshot.title.as_deref(), from_title) else {
            let mut miss = unparsed.lock().expect("not poisoned");
            if miss.len() < sample_limit {
                miss.push(track.filename.clone());
            }
            return Ok(None);
        };
        let Some(edit) = edit_for(&parsed, snapshot, genre.as_deref()) else {
            return Ok(None);
        };
        let mut rows = proposals.lock().expect("not poisoned");
        if rows.len() < sample_limit {
            rows.push(json!({
                "trackId": track.id,
                "filename": track.filename,
                "artist": { "from": snapshot.artist, "to": parsed.artist },
                "title": { "from": snapshot.title, "to": parsed.title },
            }));
        }
        Ok(Some(edit))
    })?;

    let proposals = proposals.into_inner().expect("not poisoned");
    let unparsed = unparsed.into_inner().expect("not poisoned");
    let mut out = json!({
        "dryRun": dry_run,
        "considered": ids.len(),
        "wouldChange": report.written,
        "unchanged": report.skipped,
        "failed": report.failed,
        "proposals": proposals,
        "note": if dry_run {
            "nothing was written; check the proposals and call again with dryRun false"
        } else {
            "written into the files, with the replaced tag bytes kept as a backup"
        },
    });
    if !report.failures.is_empty() {
        out["failures"] = serde_json::to_value(report.failures)?;
    }
    if !unparsed.is_empty() {
        out["noSeparator"] = json!(unparsed);
        out["noSeparatorNote"] =
            json!("these carry no `Artist - Title`, so nothing was proposed for them");
    }
    Ok(out)
}

/// The name to parse: the filename, with the renamer's own additions removed,
/// and the title tag as a fallback when that yields no artist.
fn source_name(track: &Track, title_tag: Option<&str>, from_title: bool) -> Option<ParsedName> {
    let stem = strip_stem(stem_of(&track.filename), track.original_filename.as_deref());
    let parsed = filename::parse(&stem);
    if parsed.artist.is_some() {
        return Some(parsed);
    }
    if !from_title {
        return None;
    }
    // `Chris Lake - Turn Off The Lights (Official Video)` sitting in the
    // title tag is the same mess written the other way round.
    let from_tag = filename::parse(title_tag?);
    from_tag.artist.is_some().then_some(from_tag)
}

/// Only the fields that actually differ. An edit that rewrites a field to
/// what it already says would still cost a file write, and this pass runs
/// over a whole library.
fn edit_for(parsed: &ParsedName, snapshot: &crate::models::TagSnapshot, genre: Option<&str>) -> Option<TagEdit> {
    let mut edit = TagEdit::default();
    let mut changed = false;
    if let Some(artist) = &parsed.artist {
        if snapshot.artist.as_deref() != Some(artist.as_str()) {
            edit = edit.set(TagField::Artist, artist.clone());
            changed = true;
        }
    }
    if !parsed.title.is_empty() && snapshot.title.as_deref() != Some(parsed.title.as_str()) {
        edit = edit.set(TagField::Title, parsed.title.clone());
        changed = true;
    }
    if let Some(genre) = genre {
        if snapshot.genre.as_deref() != Some(genre) {
            edit = edit.set(TagField::Genre, genre.to_string());
            changed = true;
        }
    }
    changed.then_some(edit)
}

/// What a single name parses to, written nowhere.
///
/// The cheap way for an agent to check the parser against one awkward name
/// before turning it loose on a library, and it needs no track and no file.
pub fn preview(_ctx: &McpContext, args: &Value) -> Result<Value> {
    let name = str_arg(args, "filename")?;
    let parsed = filename::parse(stem_of(&name));
    Ok(json!({
        "filename": name,
        "artist": parsed.artist,
        "title": parsed.title,
    }))
}
