//! Reading the library, and correcting what the engine decided about a track.

use serde_json::{json, Value};

use super::{bool_arg, f64_arg, ids_arg, opt_f64, opt_str, opt_u8, opt_usize, str_arg, strings_arg};
use crate::analysis::camelot::Camelot;
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::core::JobPriority;
use crate::models::{
    SortDirection, Track, TrackFilters, TrackQuery, TrackSort, ValueSource,
};

/// Rows per page when the caller does not say. Small on purpose: a page of
/// tracks is the agent's context, and 500 rows of it buys nothing a filter
/// would not have answered better.
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

/// The projection a list returns.
///
/// Not the whole row. A `Track` carries sixty fields — loudness, tuning
/// cents, four timestamps, every confidence — and a page of fifty would be
/// mostly numbers nothing asked for. These are the ones a decision gets made
/// on; `get_track` returns the rest when something actually needs them.
fn brief(track: &Track) -> Value {
    json!({
        "id": track.id,
        "artist": track.artist,
        "title": track.title,
        "album": track.album,
        "genre": track.genre,
        "bpm": track.tempo_display,
        "camelot": track.camelot,
        "energy": track.energy,
        "rating": track.rating,
        "durationMs": track.duration_ms,
        "analyzed": track.analyzed_at.is_some(),
        "path": track.path,
    })
}

pub fn list_tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let mut filters = TrackFilters {
        query: opt_str(args, "query").unwrap_or_default(),
        camelot: opt_str(args, "camelot"),
        camelot_include_compatible: bool_arg(args, "includeCompatibleKeys", false),
        collection_id: opt_str(args, "collectionId"),
        energy_min: opt_u8(args, "energyMin"),
        energy_max: opt_u8(args, "energyMax"),
        tempo_min: opt_f64(args, "bpmMin"),
        tempo_max: opt_f64(args, "bpmMax"),
        genres: strings_arg(args, "genres"),
        rating_min: opt_u8(args, "ratingMin"),
        path_prefix: opt_str(args, "pathPrefix"),
        improvable_only: bool_arg(args, "needsAttention", false),
        ..TrackFilters::default()
    };
    if let Some(analyzed) = args.get("analyzed").and_then(Value::as_bool) {
        filters.analyzed = Some(analyzed);
    }
    // A segment the caller cannot have meant is a mistake worth naming: left
    // unparsed it would silently widen to the whole library.
    if let Some(code) = &filters.camelot {
        if Camelot::parse(code).is_none() {
            return Err(KeystoneError::Other(format!(
                "`{code}` is not a Camelot segment; use 1A-12B"
            )));
        }
    }

    let query = TrackQuery {
        filters,
        sort: sort_from(opt_str(args, "sortBy").as_deref())?,
        direction: if bool_arg(args, "descending", false) {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        },
        limit: opt_usize(args, "limit").unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT),
        offset: opt_usize(args, "offset").unwrap_or(0),
    };

    let page = ctx.tracks.search(&query)?;
    Ok(json!({
        "total": page.total,
        "returned": page.tracks.len(),
        "offset": query.offset,
        "tracks": page.tracks.iter().map(brief).collect::<Vec<_>>(),
    }))
}

fn sort_from(name: Option<&str>) -> Result<TrackSort> {
    let Some(name) = name else {
        return Ok(TrackSort::Artist);
    };
    Ok(match name {
        "artist" => TrackSort::Artist,
        "title" => TrackSort::Title,
        "album" => TrackSort::Album,
        "genre" => TrackSort::Genre,
        "bpm" | "tempo" => TrackSort::Tempo,
        "key" => TrackSort::KeyResult,
        "energy" => TrackSort::Energy,
        "rating" => TrackSort::Rating,
        "dateAdded" => TrackSort::DateAdded,
        "year" => TrackSort::Year,
        "filename" => TrackSort::Filename,
        "path" => TrackSort::Path,
        other => {
            return Err(KeystoneError::Other(format!(
                "`{other}` is not a sort field"
            )))
        }
    })
}

pub fn get_track(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let track = ctx
        .tracks
        .get_by_id(&id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))?;
    Ok(serde_json::to_value(track)?)
}

pub fn list_genres(ctx: &McpContext) -> Result<Value> {
    Ok(json!({ "genres": ctx.tracks.distinct_genres()? }))
}

pub fn list_watched_folders(ctx: &McpContext) -> Result<Value> {
    Ok(serde_json::to_value(ctx.library.list_watched()?)?)
}

/// Counts, for an agent deciding what to work on.
///
/// Four one-row queries rather than one aggregate: `search` already returns a
/// total beside its page, so asking it four times reuses the indexed filter
/// paths instead of adding a fifth way to count the same table.
pub fn library_stats(ctx: &McpContext) -> Result<Value> {
    let count = |filters: TrackFilters| -> Result<usize> {
        Ok(ctx
            .tracks
            .search(&TrackQuery {
                filters,
                sort: TrackSort::Artist,
                direction: SortDirection::Asc,
                limit: 1,
                offset: 0,
            })?
            .total)
    };

    Ok(json!({
        "tracks": count(TrackFilters::default())?,
        "analyzed": count(TrackFilters { analyzed: Some(true), ..Default::default() })?,
        "unanalyzed": count(TrackFilters { analyzed: Some(false), ..Default::default() })?,
        "needsAttention": count(TrackFilters { improvable_only: true, ..Default::default() })?,
        "watchedFolders": ctx.library.list_watched()?.len(),
    }))
}

// ---------------------------------------------------------------------------
// Overrides
//
// Each stamps `ValueSource::User`, which is what stops the next analysis pass
// reverting it. That is the whole reason an agent is allowed to write these:
// a correction that a re-analysis undoes is worse than no correction.
// ---------------------------------------------------------------------------

pub fn set_key(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let code = str_arg(args, "camelot")?;
    let parsed = Camelot::parse(&code).ok_or_else(|| {
        KeystoneError::Other(format!("`{code}` is not a Camelot segment; use 1A-12B"))
    })?;
    let (root, mode) = parsed.to_key();
    ctx.tracks.set_key(&id, root, mode, ValueSource::User)?;
    updated(ctx, &id)
}

pub fn set_tempo(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let bpm = f64_arg(args, "bpm")?;
    if !bpm.is_finite() || !(20.0..=300.0).contains(&bpm) {
        return Err(KeystoneError::Other(format!(
            "{bpm} is not a tempo a track could have"
        )));
    }
    ctx.tracks.set_tempo(&id, bpm, ValueSource::User)?;
    updated(ctx, &id)
}

pub fn set_energy(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let energy = opt_u8(args, "energy").filter(|e| (1..=10).contains(e)).ok_or_else(|| {
        KeystoneError::Other("`energy` is 1-10".into())
    })?;
    ctx.tracks.set_energy(&id, energy, ValueSource::User)?;
    updated(ctx, &id)
}

pub fn set_rating(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let rating = opt_u8(args, "rating");
    if rating.is_some_and(|r| r > 5) {
        return Err(KeystoneError::Other("`rating` is 0-5".into()));
    }
    // 0 clears it, the way clicking the lit dot does in the table.
    ctx.tracks.set_rating(&id, rating.filter(|r| *r > 0))?;
    updated(ctx, &id)
}

/// Queue analysis. Needs the app: the workers live in it.
pub fn analyze_tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let ids = ids_arg(args, "trackIds")?;
    let live = ctx.require_live("queueing analysis")?;
    let queued = if bool_arg(args, "force", false) {
        live.queue.force_reanalyze(ids)?
    } else {
        live.queue.enqueue(ids, JobPriority::Normal)?
    };
    Ok(json!({ "queued": queued }))
}

fn updated(ctx: &McpContext, id: &str) -> Result<Value> {
    let track = ctx
        .tracks
        .get_by_id(id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))?;
    Ok(brief(&track))
}
