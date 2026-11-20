//! What to play next, and a set built from it.
//!
//! The scoring is [`crate::core::set_builder`]; this chooses the pool and
//! shapes the answer. Two things about the pool are decisions rather than
//! details:
//!
//! * **A set is built from a collection when one is named.** Picking the next
//!   record out of the whole library is a different job from picking it out of
//!   the crate somebody packed for tonight, and the second is nearly always
//!   what is wanted.
//! * **Unanalysed tracks are excluded.** They have no key and no tempo, so
//!   every one of them would be scored as "unknown" and the answer would be a
//!   list of records nobody can tell you anything about.

use serde_json::{json, Value};

use super::{bool_arg, opt_f64, opt_str, opt_u8, opt_usize, str_arg};
use crate::core::set_builder::{self, Rules};
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::models::{SortDirection, Track, TrackFilters, TrackQuery, TrackSort};

/// Candidates returned by default.
const DEFAULT_LIMIT: usize = 10;
/// The ceiling on how much of the library is scored in one call.
const POOL_LIMIT: usize = 2_000;

pub fn suggest_next(ctx: &McpContext, args: &Value) -> Result<Value> {
    let from = track(ctx, &str_arg(args, "trackId")?)?;
    let rules = rules(args);
    let pool = pool(ctx, args)?;
    let limit = opt_usize(args, "limit").unwrap_or(DEFAULT_LIMIT);

    let ranked = set_builder::rank(&from, &pool, &rules);
    let by_id = index(&pool);
    let rows: Vec<Value> = ranked
        .iter()
        .take(limit)
        .filter_map(|c| by_id.get(c.track_id.as_str()).map(|t| candidate(c, t)))
        .collect();

    Ok(json!({
        "from": super::library::brief_of(&from),
        "considered": pool.len(),
        "compatible": ranked.len(),
        "candidates": rows,
        "rules": described(&rules),
        "note": if rows.is_empty() {
            "nothing in that pool mixes out of this track under these rules; \
widen bpmDrift, or allow tracks with no key"
        } else {
            "ranked by key, then how little the tempo has to move, then energy"
        },
    }))
}

/// A playable order, long enough to fill the time asked for.
///
/// Returns the chain rather than writing it: a set is a judgement, and the
/// agent can put it in a collection with `create_collection` once somebody
/// has looked at it.
pub fn build_set(ctx: &McpContext, args: &Value) -> Result<Value> {
    let minutes = opt_f64(args, "minutes").unwrap_or(60.0);
    if !(1.0..=600.0).contains(&minutes) {
        return Err(KeystoneError::Other(
            "`minutes` is the length of the set, between 1 and 600".into(),
        ));
    }
    let rules = rules(args);
    let pool = pool(ctx, args)?;
    let seed = match opt_str(args, "startTrackId") {
        Some(id) => track(ctx, &id)?,
        None => opening(&pool, args)?,
    };

    let chain = set_builder::build(&seed, &pool, minutes, &rules);
    let by_id = index(&pool);
    let mut order = Vec::with_capacity(chain.order.len());
    for (position, id) in chain.order.iter().enumerate() {
        let row = if *id == seed.id {
            Some(&seed)
        } else {
            by_id.get(id.as_str()).copied()
        };
        let Some(row) = row else { continue };
        let mut entry = json!({
            "position": position + 1,
            "track": super::library::brief_of(row),
        });
        // The transition that chose this track, which is the part worth
        // reading: a set with no account of why each record follows the last
        // cannot be argued with.
        if position > 0 {
            if let Some(c) = chain.transitions.get(position - 1) {
                entry["from_previous"] = json!({
                    "keyRelation": c.key_relation,
                    "bpmChange": c.bpm_change,
                    "energyChange": c.energy_change,
                    "score": (c.score * 100.0).round() / 100.0,
                });
            }
        }
        order.push(entry);
    }

    let mut out = json!({
        "tracks": order.len(),
        "minutes": (chain.total_ms as f64 / 60_000.0 * 10.0).round() / 10.0,
        "requestedMinutes": minutes,
        "order": order,
        "rules": described(&rules),
    });
    if chain.ran_out {
        out["note"] = json!(
            "the pool ran out of compatible records before the time was filled; \
widen bpmDrift, analyse more tracks, or pass a larger collection"
        );
    }
    Ok(out)
}

/// The first record: the lowest-energy track in the pool, since a set that
/// opens on its biggest record has nowhere to go.
fn opening(pool: &[Track], args: &Value) -> Result<Track> {
    let target = opt_f64(args, "startBpm");
    pool.iter()
        .min_by(|a, b| {
            let key = |t: &Track| {
                let energy = t.energy.unwrap_or(5);
                let distance = target
                    .and_then(|want| t.tempo_display.or(t.tempo_raw).map(|b| (b - want).abs()))
                    .unwrap_or(0.0);
                (energy, (distance * 100.0) as i64, t.id.clone())
            };
            key(a).cmp(&key(b))
        })
        .cloned()
        .ok_or_else(|| {
            KeystoneError::NotFound(
                "nothing analysed to build a set from; run the analysis first".into(),
            )
        })
}

fn rules(args: &Value) -> Rules {
    Rules {
        drift: opt_f64(args, "bpmDrift")
            .map(|d| if d > 1.0 { d / 100.0 } else { d })
            .unwrap_or(set_builder::DEFAULT_DRIFT),
        require_key: bool_arg(args, "requireCompatibleKey", true),
        climbing: bool_arg(args, "climbing", true),
    }
}

fn described(rules: &Rules) -> Value {
    json!({
        "bpmDriftPercent": (rules.drift * 1000.0).round() / 10.0,
        "requireCompatibleKey": rules.require_key,
        "energy": if rules.climbing { "climbing" } else { "held" },
    })
}

/// Analysed tracks from the named collection, or from the library.
fn pool(ctx: &McpContext, args: &Value) -> Result<Vec<Track>> {
    let filters = TrackFilters {
        analyzed: Some(true),
        collection_id: opt_str(args, "collectionId"),
        rating_min: opt_u8(args, "minRating"),
        genres: super::strings_arg(args, "genres"),
        ..TrackFilters::default()
    };
    Ok(ctx
        .tracks
        .search(&TrackQuery {
            filters,
            sort: TrackSort::Tempo,
            direction: SortDirection::Asc,
            limit: POOL_LIMIT,
            offset: 0,
        })?
        .tracks)
}

fn track(ctx: &McpContext, id: &str) -> Result<Track> {
    ctx.tracks
        .get_by_id(id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))
}

fn index(pool: &[Track]) -> std::collections::HashMap<&str, &Track> {
    pool.iter().map(|t| (t.id.as_str(), t)).collect()
}

fn candidate(c: &set_builder::Candidate, track: &Track) -> Value {
    json!({
        "track": super::library::brief_of(track),
        "score": (c.score * 100.0).round() / 100.0,
        "keyRelation": c.key_relation,
        "bpmChange": c.bpm_change,
        "energyChange": c.energy_change,
    })
}
