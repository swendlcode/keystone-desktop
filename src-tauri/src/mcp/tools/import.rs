//! Bringing files into the library.
//!
//! The distinction this tool has to preserve is the one the window makes when
//! something is dropped on it: a **folder** becomes a watched root, because
//! that is how a library grows and the user expects it to keep tracking,
//! while a **file** is indexed on its own and its parent is left alone.
//! Watching the parent of one track pulled out of a Downloads folder would
//! adopt the other three hundred and ninety-nine sitting next to it.
//!
//! Analysis is started here rather than left to the caller. An import that
//! only wrote rows is the bug this pass fixes elsewhere: the tracks appear
//! with no key, tempo or energy, and nothing says why.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{bool_arg, strings_arg};
use crate::commands::library_commands::{spawn_analysis_after_indexing, spawn_scan};
use crate::core::{scanner, IndexJob, JobPriority};
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;

/// Queue files, folders, or a mix of both.
///
/// `recursive` decides what a folder means, and the default is the window's
/// behaviour: walk it and watch it. `recursive: false` takes only the audio
/// files sitting directly in it and watches nothing, which is the "import
/// this one album" case where adopting every subfolder would be wrong.
pub fn import_tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let paths = named_paths(args)?;
    let recursive = bool_arg(args, "recursive", true);
    let state = super::folders::app_state(ctx, "importing tracks")?;

    let mut watched: Vec<String> = Vec::new();
    let mut loose: Vec<IndexJob> = Vec::new();
    let mut shallow: Vec<PathBuf> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();

    for raw in &paths {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            if recursive {
                watched.push(raw.clone());
            } else {
                shallow.push(path);
            }
        } else if path.is_file() {
            // Non-audio is skipped rather than refused. An import is usually a
            // selection, and failing the whole call because one `.txt` came
            // along with forty tracks is the wrong trade — but what was
            // ignored is named, so a caller that passed a folder of `.flac`
            // by mistake is not told "0 imported" with no reason.
            if scanner::audio_extension(&path).is_some() {
                loose.push(job(path));
            } else {
                ignored.push(raw.clone());
            }
        } else {
            return Err(KeystoneError::NotFound(format!("{raw} is not there")));
        }
    }

    for dir in &shallow {
        let (found, skipped) = direct_children(dir)?;
        loose.extend(found);
        ignored.extend(skipped);
    }

    let queued = loose.len();
    if queued > 0 {
        state.indexer.enqueue_batch(loose);
        // Before the folder walks below: a handful of named files must not
        // wait behind a walk of twenty thousand.
        spawn_analysis_after_indexing(state.indexer.clone(), state.queue.clone());
    }

    let mut roots = Vec::new();
    for raw in &watched {
        let root = PathBuf::from(raw);
        state.indexer.unexclude(&root);
        let folder = state.library.add_watched(raw)?;
        state.watcher.watch(&root)?;
        spawn_scan(state.indexer.clone(), root, state.queue.clone());
        roots.push(serde_json::to_value(folder)?);
    }

    if queued == 0 && roots.is_empty() {
        return Err(KeystoneError::Other(format!(
            "nothing importable in those {} paths; audio files or folders of them",
            paths.len()
        )));
    }

    let mut out = json!({
        "filesQueued": queued,
        "foldersWatched": roots,
        "note": "indexing runs in the background and analysis follows it; \
get_analysis_status says how far along it is",
    });
    if !ignored.is_empty() {
        out["ignored"] = json!(ignored);
    }
    Ok(out)
}

/// `paths`, or `path` for the single-file call an agent writes half the time.
fn named_paths(args: &Value) -> Result<Vec<String>> {
    let mut paths = strings_arg(args, "paths");
    if paths.is_empty() {
        paths = strings_arg(args, "path");
    }
    if paths.is_empty() {
        return Err(KeystoneError::Other(
            "`paths` is the files or folders to import".into(),
        ));
    }
    Ok(paths)
}

/// The audio files directly inside a folder, and the names of what was passed
/// over. `Scanner::scan` is the recursive walk; this is deliberately not it.
fn direct_children(dir: &std::path::Path) -> Result<(Vec<IndexJob>, Vec<String>)> {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            skipped.push(path.to_string_lossy().into_owned());
        } else if scanner::audio_extension(&path).is_some() {
            found.push(job(path));
        }
    }
    Ok((found, skipped))
}

fn job(path: PathBuf) -> IndexJob {
    IndexJob {
        path,
        priority: JobPriority::Normal,
    }
}

/// Re-point a track at another file, keeping everything known about it.
///
/// The case it exists for: a track mastered in Capstone comes back as a new
/// AIFF beside the original, and the library should follow it rather than
/// index a second row and lose the rating, the cues and the analysis that
/// were attached to the first.
///
/// The id is kept, for the same reason the path repair keeps it: five tables
/// hang off `track_id`, so re-pointing the row carries the collections,
/// ratings, cues and beatgrid along with it. The **content hash is not**
/// kept. It is what the peaks cache and the analysis cache are keyed on, and
/// a mastered file is different audio — keeping the old hash would draw the
/// old waveform over the new file. The analysis columns stay as they are,
/// which is the whole point: the key and tempo of a master are the key and
/// tempo of what it was mastered from.
pub fn relink_track(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = super::str_arg(args, "trackId")?;
    let new_path = super::str_arg(args, "newPath")?;
    let path = crate::core::paths::normalize(std::path::Path::new(&new_path));
    if !path.is_file() {
        return Err(KeystoneError::NotFound(format!("{new_path} is not a file")));
    }
    if scanner::audio_extension(&path).is_none() {
        return Err(KeystoneError::Other(format!(
            "{new_path} is not an audio file Keystone can read"
        )));
    }
    let track = ctx
        .tracks
        .get_by_id(&id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))?;

    let text = path.to_string_lossy().to_string();
    // A row already at this path would be a second row for one file: the
    // upsert conflicts on `path`, so letting the write through would fail at
    // the database with a message about a constraint rather than about the
    // actual problem.
    if let Some(other) = ctx.tracks.get_by_path(&text)? {
        if other.id != id {
            return Err(KeystoneError::Other(format!(
                "{text} is already in the library as {} — {}",
                other.id,
                other.title.as_deref().unwrap_or(&other.filename)
            )));
        }
    }

    let hash = crate::core::hasher::hash_audio_region(&path).ok();
    let size = std::fs::metadata(&path).ok().map(|m| m.len() as i64);
    ctx.tracks.relink(&id, &text, hash.as_deref(), size)?;

    let updated = ctx
        .tracks
        .get_by_id(&id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))?;
    Ok(json!({
        "track": super::library::brief_of(&updated),
        "from": track.path,
        "to": text,
        "note": "the analysis, rating, cues and collection membership moved with it; \
the waveform is redrawn from the new file on first play",
    }))
}
