//! Row to [`Track`]. One function, because every read path projects
//! [`crate::db::query_builder::TRACK_COLUMNS`] in that exact order — and the
//! positional reads below are only correct while that stays true.
//!
//! It is the one place that runs past the fifty-line function cap, and
//! deliberately. `Track` is flat, so splitting this by column group means
//! helpers returning six- and eight-element tuples that the caller destructures
//! positionally — which is the exact mistake (a value landing in the wrong
//! field) that one-field-per-line reading prevents. Keep it as a single
//! mapping; if it needs changing, change `TRACK_COLUMNS` in the same commit.

use rusqlite::Row;

use crate::db::mapping::{index_status, value_source};
use crate::models::{KeyMode, Track};

pub fn row_to_track(r: &Row<'_>) -> rusqlite::Result<Track> {
    let mode = |i: usize| -> rusqlite::Result<Option<KeyMode>> {
        Ok(r.get::<_, Option<String>>(i)?.as_deref().and_then(KeyMode::parse))
    };
    let source = |i: usize| -> rusqlite::Result<Option<crate::models::ValueSource>> {
        Ok(value_source(r.get::<_, Option<String>>(i)?.as_deref()))
    };

    Ok(Track {
        id: r.get(0)?,
        path: r.get(1)?,
        filename: r.get(2)?,
        original_filename: r.get(3)?,
        rename_pattern: r.get(4)?,
        extension: r.get(5)?,
        content_hash: r.get(6)?,

        size_bytes: r.get(7)?,
        duration_ms: r.get(8)?,
        sample_rate: r.get(9)?,
        channels: r.get(10)?,
        bit_depth: r.get(11)?,
        bitrate_kbps: r.get(12)?,
        codec: r.get(13)?,

        artist: r.get(14)?,
        title: r.get(15)?,
        album: r.get(16)?,
        genre: r.get(17)?,
        grouping: r.get(18)?,
        comment: r.get(19)?,
        year: r.get(20)?,
        tag_bpm: r.get(21)?,
        tag_key: r.get(22)?,
        has_cover_art: r.get::<_, Option<bool>>(23)?.unwrap_or(false),

        tempo_raw: r.get(24)?,
        tempo_display: r.get(25)?,
        tempo_octave_shift: r.get(26)?,
        tempo_confidence: r.get(27)?,
        tempo_residual_ms: r.get(28)?,
        tempo_variable: r.get::<_, Option<bool>>(29)?.unwrap_or(false),

        key_root: r.get(30)?,
        key_mode: mode(31)?,
        camelot: r.get(32)?,
        key_confidence: r.get(33)?,
        key_alt_root: r.get(34)?,
        key_alt_mode: mode(35)?,
        key_mode_ambiguous: r.get::<_, Option<bool>>(36)?.unwrap_or(false),
        tuning_cents: r.get(37)?,

        energy: r.get(38)?,
        energy_raw: r.get(39)?,
        peak_db: r.get(40)?,
        true_peak_db: r.get(41)?,
        lufs_integrated: r.get(42)?,
        clipped_peaks: r.get(43)?,

        rating: r.get(44)?,
        date_added: r.get(45)?,
        last_played: r.get(46)?,
        play_count: r.get::<_, Option<i64>>(47)?.unwrap_or(0),

        key_source: source(48)?,
        tempo_source: source(49)?,
        energy_source: source(50)?,
        analysis_version: r.get::<_, Option<i32>>(51)?.unwrap_or(0),
        energy_model_version: r.get::<_, Option<i32>>(52)?.unwrap_or(0),
        analyzed_at: r.get(53)?,
        // DEFAULT 'pending' is not NOT NULL, so a hand-edited row can hold
        // NULL here and must not fail the whole page.
        index_status: index_status(r.get::<_, Option<String>>(54)?.as_deref().unwrap_or("pending")),
        error_message: r.get(55)?,

        created_at: r.get(56)?,
        updated_at: r.get(57)?,
        last_seen_at: r.get(58)?,
    })
}
