//! Finding and fixing tempi that are out by a ratio.
//!
//! The hint is the argument, not a setting. `bpmMin`/`bpmMax` say what this
//! material is — 115 to 135 for house, 165 to 180 for drum and bass — and
//! nothing is proposed for a track already inside it. Passing no hint uses
//! the user's own Tempo range from Settings, which is the display fold's
//! range and the only one the app itself believes in.
//!
//! Why this is not the Tempo range setting: that setting re-folds the whole
//! library by octaves at read time, so narrowing it to 115–135 would not fix
//! a 162.6 BPM track — there is no power of two between them — and would
//! quietly re-fold every drum and bass track in the library on the way past.
//! A ratio correction is per track and is written as the user's own value.

use serde_json::{json, Value};

use super::{bool_arg, ids_arg, opt_f64, opt_usize};
use crate::core::tempo_fix;
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::models::{SortDirection, Track, TrackFilters, TrackQuery, TrackSort, ValueSource};

/// How many tracks are examined when the caller names none. The whole library
/// in one pass is the normal case for a DJ library; this is the backstop for
/// a 160,000-row one.
const SCAN_LIMIT: usize = 5_000;

pub fn suggest(ctx: &McpContext, args: &Value) -> Result<Value> {
    let (lo, hi) = range(ctx, args)?;
    let apply = !bool_arg(args, "dryRun", true);
    let max_confidence = opt_f64(args, "maxConfidence");
    let tracks = chosen(ctx, args)?;

    let mut rows = Vec::new();
    let mut applied = 0usize;
    let mut ambiguous = 0usize;
    for track in &tracks {
        let Some(current) = track.tempo_display.or(track.tempo_raw) else {
            continue;
        };
        if max_confidence.is_some_and(|c| track.tempo_confidence.unwrap_or(0.0) > c) {
            continue;
        }
        let Some(s) = tempo_fix::suggest(current, lo, hi) else {
            continue;
        };
        if !s.also.is_empty() {
            ambiguous += 1;
        }
        // An ambiguous track is reported and never written: two ratios both
        // landing in range means the range did not decide it, and picking one
        // on the caller's behalf is how a correct 174 becomes 130.5.
        let wrote = apply && s.also.is_empty();
        if wrote {
            ctx.tracks.set_tempo(&track.id, s.bpm, ValueSource::User)?;
            applied += 1;
        }
        rows.push(json!({
            "trackId": track.id,
            "artist": track.artist,
            "title": track.title.clone().unwrap_or_else(|| track.filename.clone()),
            "detected": round2(current),
            "suggested": s.bpm,
            "ratio": s.ratio_name,
            "confidence": track.tempo_confidence,
            "alsoInRange": s.also,
            "applied": wrote,
        }));
    }

    let mut out = json!({
        "range": { "min": lo, "max": hi },
        "examined": tracks.len(),
        "suggested": rows.len(),
        "applied": applied,
        "tracks": rows,
    });
    if ambiguous > 0 {
        out["ambiguous"] = json!(ambiguous);
        out["ambiguousNote"] = json!(
            "more than one ratio lands in that range for these, so they were left \
alone; listen to one and call set_tempo, or narrow the range"
        );
    }
    out["note"] = json!(if apply {
        "written as your own tempo, so the next analysis pass will not revert it"
    } else {
        "nothing was written; call again with dryRun false to apply"
    });
    Ok(out)
}

/// The hint, or the user's configured fold range.
fn range(ctx: &McpContext, args: &Value) -> Result<(f64, f64)> {
    let (default_lo, default_hi) = ctx.settings.tempo_range();
    let lo = opt_f64(args, "bpmMin").unwrap_or(default_lo);
    let hi = opt_f64(args, "bpmMax").unwrap_or(default_hi);
    if !lo.is_finite() || lo <= 0.0 || hi < lo {
        return Err(KeystoneError::Other(format!(
            "{lo}-{hi} is not a tempo range"
        )));
    }
    // An open-ended range can never place a track outside itself, so it would
    // silently suggest nothing at all rather than fail.
    if !hi.is_finite() {
        return Err(KeystoneError::Other(
            "the Tempo range setting is open, so there is nothing to correct \
towards: pass bpmMin and bpmMax, such as 115 and 135 for house"
                .into(),
        ));
    }
    Ok((lo, hi))
}

fn chosen(ctx: &McpContext, args: &Value) -> Result<Vec<Track>> {
    if args.get("trackIds").is_some() {
        let ids = ids_arg(args, "trackIds")?;
        return ctx.tracks.get_many(&ids);
    }
    let limit = opt_usize(args, "limit").unwrap_or(SCAN_LIMIT).min(SCAN_LIMIT);
    Ok(ctx
        .tracks
        .search(&TrackQuery {
            filters: TrackFilters {
                analyzed: Some(true),
                ..Default::default()
            },
            sort: TrackSort::Tempo,
            direction: SortDirection::Asc,
            limit,
            offset: 0,
        })?
        .tracks)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
