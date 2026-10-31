//! Builds the library query. Two rules hold this file together, and both were
//! paid for in Stack:
//!
//! 1. Spell a partial index's predicate identically in the WHERE clause. A
//!    query filtering `index_status != 'missing'` against an index that does
//!    not carry it makes SQLite fetch every row back to re-check the predicate.
//! 2. Never `LIKE 'prefix%'` on a path. SQLite's LIKE is case-insensitive by
//!    default, which disables the prefix optimisation and forces a full scan.
//!    Use the range form via [`path_upper_bound`].

use rusqlite::types::Value;

use crate::analysis::camelot::Camelot;
use crate::db::mapping::{index_status_str, placeholders};
use crate::models::{SortDirection, TrackFilters, TrackSort};

/// Repeated verbatim in every partial index in migration 001. Changing one
/// without the other is the 320 ms -> 30 ms regression, backwards.
pub const LIVE_PREDICATE: &str = "index_status != 'missing'";

/// Below this, the two key voters disagreed enough that the letter on the badge
/// is a guess. It is one of the four things that put a track in the spec's
/// `Improve Tracks` collection.
pub const IMPROVABLE_KEY_CONFIDENCE: f64 = 0.6;

/// The SELECT list, so every read path projects the same columns in the same
/// order and `TrackRepository::row_to_track` stays a single function.
pub const TRACK_COLUMNS: &str = "\
id, path, filename, original_filename, rename_pattern, extension, content_hash, \
size_bytes, duration_ms, sample_rate, channels, bit_depth, bitrate_kbps, codec, \
artist, title, album, genre, grouping, comment, year, tag_bpm, tag_key, has_cover_art, \
tempo_raw, tempo_display, tempo_octave_shift, tempo_confidence, tempo_residual_ms, tempo_variable, \
key_root, key_mode, camelot, key_confidence, key_alt_root, key_alt_mode, key_mode_ambiguous, tuning_cents, \
energy, energy_raw, peak_db, true_peak_db, lufs_integrated, clipped_peaks, \
rating, date_added, last_played, play_count, \
key_source, tempo_source, energy_source, analysis_version, energy_model_version, analyzed_at, \
index_status, error_message, created_at, updated_at, last_seen_at";

/// Builds the WHERE clause body and its bound parameters, with no leading
/// `WHERE` — callers compose it, because `wheel_counts` needs the same body
/// under a `GROUP BY` and the count needs it under a `COUNT(*)`.
///
/// Never empty: the status predicate is always present, which is what keeps the
/// partial indexes in play.
pub fn build_where(filters: &TrackFilters) -> (String, Vec<Value>) {
    let mut c: Vec<String> = Vec::new();
    let mut p: Vec<Value> = Vec::new();

    push_status(filters, &mut c, &mut p);
    push_text(filters, &mut c, &mut p);

    if let Some(selected) = filters.camelot.as_deref().and_then(Camelot::parse) {
        let (frag, mut bound) = camelot_clause(&selected, filters.camelot_include_compatible);
        c.push(frag);
        p.append(&mut bound);
    }
    if let Some(id) = &filters.collection_id {
        // A membership subquery rather than a join: the outer query already has
        // its index chosen by the Camelot or status predicate, and a join here
        // tempts the planner into driving from collection_tracks instead.
        c.push("id IN (SELECT track_id FROM collection_tracks WHERE collection_id = ?)".into());
        p.push(Value::Text(id.clone()));
    }

    push_ranges(filters, &mut c, &mut p);
    push_scope(filters, &mut c, &mut p);

    if filters.improvable_only {
        // The spec's `Improve Tracks`: voter disagreement, a key the engine is
        // not sure of, clipping Capstone would fix, or nothing analysed at all.
        c.push(format!(
            "(key_mode_ambiguous = 1 OR key_confidence < {IMPROVABLE_KEY_CONFIDENCE} \
             OR clipped_peaks > 0 OR analysis_version = 0)"
        ));
    }

    (c.join(" AND "), p)
}

fn push_status(f: &TrackFilters, c: &mut Vec<String>, p: &mut Vec<Value>) {
    if f.index_statuses.is_empty() {
        c.push(LIVE_PREDICATE.to_string());
        return;
    }
    // An explicit status filter is the one caller that wants missing rows, so
    // it replaces the live predicate rather than being ANDed with it.
    c.push(format!(
        "index_status IN ({})",
        placeholders(f.index_statuses.len())
    ));
    for s in &f.index_statuses {
        p.push(Value::Text(index_status_str(*s).into()));
    }
}

fn push_text(f: &TrackFilters, c: &mut Vec<String>, p: &mut Vec<Value>) {
    let query = f.query.trim();
    if query.is_empty() {
        return;
    }
    match fts_match_expression(query) {
        Some(expr) => {
            c.push("rowid IN (SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?)".into());
            p.push(Value::Text(expr));
        }
        None => {
            // The only case allowed to fall back: a query the tokenizer cannot
            // index at all, such as punctuation on its own.
            c.push(
                "(instr(lower(filename), ?) > 0 OR instr(lower(COALESCE(artist, '')), ?) > 0 \
                 OR instr(lower(COALESCE(title, '')), ?) > 0)"
                    .into(),
            );
            let needle = query.to_lowercase();
            for _ in 0..3 {
                p.push(Value::Text(needle.clone()));
            }
        }
    }
}

fn push_ranges(f: &TrackFilters, c: &mut Vec<String>, p: &mut Vec<Value>) {
    let mut num = |col: &str, op: &str, v: Option<f64>| {
        if let Some(v) = v {
            c.push(format!("{col} {op} ?"));
            p.push(Value::Real(v));
        }
    };
    num("energy", ">=", f.energy_min.map(f64::from));
    num("energy", "<=", f.energy_max.map(f64::from));
    // The table shows `tempo_display`, and `idx_tracks_camelot_tempo` carries
    // it. Filtering `tempo_raw` would quietly disagree with the column a DJ is
    // reading, since a folded 174 and a raw 87 are the same track.
    num("tempo_display", ">=", f.tempo_min);
    num("tempo_display", "<=", f.tempo_max);
    num("rating", ">=", f.rating_min.map(f64::from));

    if !f.genres.is_empty() {
        c.push(format!(
            "genre COLLATE NOCASE IN ({})",
            placeholders(f.genres.len())
        ));
        for g in &f.genres {
            p.push(Value::Text(g.clone()));
        }
    }
    if let Some(analyzed) = f.analyzed {
        c.push(if analyzed {
            "analysis_version > 0".into()
        } else {
            "analysis_version = 0".to_string()
        });
    }
}

fn push_scope(f: &TrackFilters, c: &mut Vec<String>, p: &mut Vec<Value>) {
    if let Some(prefix) = &f.path_prefix {
        if prefix.is_empty() {
            return;
        }
        // Range form, not LIKE. See the module header.
        c.push("(path >= ? AND path < ?)".into());
        p.push(Value::Text(prefix.clone()));
        p.push(Value::Text(path_upper_bound(prefix)));
    }
}

/// `ORDER BY` for a sort column, always with a tiebreak on `id`: without one,
/// two rows equal on the sort key can swap between pages and a virtualized
/// table shows the same track twice while hiding another.
pub fn build_order(sort: TrackSort, direction: SortDirection) -> String {
    let dir = match direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    if matches!(sort, TrackSort::KeyResult) {
        // `camelot` is text, so a plain sort puts 10A before 1A. Sorting the
        // number numerically costs the index, which is the right trade: the
        // wheel order *is* the meaning of this column, and getting it wrong is
        // visible on every screen.
        return format!(
            "ORDER BY camelot IS NULL, CAST(substr(camelot, 1, length(camelot) - 1) AS INTEGER) \
             {dir}, substr(camelot, -1) {dir}, id ASC"
        );
    }
    let key = match sort {
        // NOCASE on exactly the columns whose index carries it.
        TrackSort::Artist => "artist COLLATE NOCASE",
        TrackSort::Title => "title COLLATE NOCASE",
        TrackSort::Album => "album COLLATE NOCASE",
        TrackSort::Genre => "genre COLLATE NOCASE",
        TrackSort::Filename => "filename COLLATE NOCASE",
        TrackSort::Path => "path",
        TrackSort::Tempo => "tempo_display",
        TrackSort::Energy => "energy",
        TrackSort::DateAdded => "date_added",
        TrackSort::Rating => "rating",
        TrackSort::Bitrate => "bitrate_kbps",
        TrackSort::Year => "year",
        TrackSort::CuePointCount => {
            "(SELECT COUNT(*) FROM cue_points cp WHERE cp.track_id = tracks.id)"
        }
        TrackSort::KeyResult => unreachable!("handled above"),
    };
    format!("ORDER BY {key} {dir}, id ASC")
}

/// The four-element wheel filter: the segment itself, its relative across the
/// ring, and ±1 around the ring.
pub fn camelot_clause(selected: &Camelot, include_compatible: bool) -> (String, Vec<Value>) {
    let set: Vec<String> = if include_compatible {
        selected
            .compatible()
            .iter()
            .map(|c| c.label(false))
            .collect()
    } else {
        vec![selected.label(false)]
    };
    // Stored unpadded, so the query must not pad either: the zero-padding
    // setting is a display choice and `08A` would match nothing.
    let frag = format!("camelot IN ({})", placeholders(set.len()));
    (frag, set.into_iter().map(Value::Text).collect())
}

/// Turns a user query into one FTS5 MATCH expression of prefix groups.
///
/// Returns `None` when the query has no indexable token, which is the only case
/// that may fall back to LIKE. Deliberately does **not** expand terms by edit
/// distance: rewriting a user's word into other words is how `crowd` becomes
/// `(chord*) OR (cold*) OR (crowd*)`. Typo tolerance belongs in a suggestion
/// the user can accept, not in the live query.
pub fn fts_match_expression(query: &str) -> Option<String> {
    let groups: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| {
            // Quoted, so a term that happens to be AND, OR, NOT or NEAR is a
            // word rather than an operator, and a stray quote cannot end the
            // string early.
            format!("\"{}\"*", t.to_lowercase().replace('"', "\"\""))
        })
        .collect();
    if groups.is_empty() {
        return None;
    }
    // AND between terms: two words typed together narrow, they do not widen.
    Some(groups.join(" AND "))
}

/// Exclusive upper bound for a path prefix range scan: the prefix with its last
/// byte incremented, so everything starting with `prefix` sorts below it.
pub fn path_upper_bound(prefix: &str) -> String {
    let mut bytes = prefix.as_bytes().to_vec();
    // Walk back over any 0xFF bytes, which cannot be incremented in place.
    while let Some(last) = bytes.pop() {
        if last < 0xFF {
            bytes.push(last + 1);
            return String::from_utf8_lossy(&bytes).into_owned();
        }
    }
    // An all-0xFF prefix has no finite upper bound; fall back to the max char.
    format!("{}\u{10FFFF}", prefix)
}

#[cfg(test)]
#[path = "query_builder_tests.rs"]
mod tests;
