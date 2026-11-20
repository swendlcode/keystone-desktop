//! A library of four-minute records at known keys, tempi and energies, so a
//! chain can be read off the assertions by eye.

use super::*;
use crate::models::IndexStatus;

/// A track with only the columns a transition is decided on.
///
/// Built by hand rather than through the database harness: everything in this
/// module is pure, and a test that needed a migrated SQLite file to score two
/// records would be testing something else.
fn track(id: &str, camelot: &str, bpm: f64, energy: u8) -> Track {
    Track {
        id: id.to_string(),
        path: format!("/music/{id}.aiff"),
        filename: format!("{id}.aiff"),
        original_filename: None,
        rename_pattern: None,
        extension: "aiff".into(),
        content_hash: None,
        size_bytes: None,
        duration_ms: Some(240_000),
        sample_rate: None,
        channels: None,
        bit_depth: None,
        bitrate_kbps: None,
        codec: None,
        artist: Some("Artist".into()),
        title: Some(id.to_string()),
        album: None,
        genre: None,
        grouping: None,
        comment: None,
        year: None,
        tag_bpm: None,
        tag_key: None,
        has_cover_art: false,
        tempo_raw: Some(bpm),
        tempo_display: Some(bpm),
        tempo_octave_shift: Some(0),
        tempo_confidence: Some(0.9),
        tempo_residual_ms: Some(5.0),
        tempo_variable: false,
        key_root: None,
        key_mode: None,
        camelot: Some(camelot.to_string()),
        key_confidence: Some(0.9),
        key_alt_root: None,
        key_alt_mode: None,
        key_mode_ambiguous: false,
        tuning_cents: None,
        energy: Some(energy),
        energy_raw: None,
        peak_db: None,
        true_peak_db: None,
        lufs_integrated: None,
        clipped_peaks: None,
        rating: None,
        date_added: 0,
        last_played: None,
        play_count: 0,
        key_source: None,
        tempo_source: None,
        energy_source: None,
        analysis_version: 1,
        energy_model_version: 1,
        analyzed_at: Some(1),
        index_status: IndexStatus::Indexed,
        error_message: None,
        created_at: 0,
        updated_at: 0,
        last_seen_at: 0,
    }
}

fn keyless(id: &str, bpm: f64) -> Track {
    let mut t = track(id, "8A", bpm, 5);
    t.camelot = None;
    t
}

#[test]
fn a_key_off_the_wheel_is_not_offered() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![
        track("same", "8A", 128.0, 5),
        track("relative", "8B", 128.0, 5),
        track("up", "9A", 128.0, 5),
        track("down", "7A", 128.0, 5),
        track("far", "2A", 128.0, 5),
    ];
    let ranked = rank(&from, &pool, &Rules::default());
    let ids: Vec<&str> = ranked.iter().map(|c| c.track_id.as_str()).collect();
    assert!(!ids.contains(&"far"), "2A is not compatible with 8A");
    assert_eq!(ids.len(), 4);
    // The same key is the strongest of them.
    assert_eq!(ranked[0].track_id, "same");
    assert_eq!(ranked[0].key_relation, "same key");
}

#[test]
fn a_tempo_outside_the_drift_is_not_offered() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![
        track("near", "8A", 130.0, 5),
        // 6% of 128 is 7.68, so 136 is outside and 135 is inside.
        track("edge", "8A", 135.0, 5),
        track("beyond", "8A", 136.0, 5),
    ];
    let ids: Vec<String> = rank(&from, &pool, &Rules::default())
        .into_iter()
        .map(|c| c.track_id)
        .collect();
    assert!(ids.contains(&"near".to_string()));
    assert!(ids.contains(&"edge".to_string()));
    assert!(!ids.contains(&"beyond".to_string()));
}

/// Holding the tempo beats walking to the edge of what is mixable.
#[test]
fn the_closer_tempo_wins_when_the_keys_match() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![track("drifted", "8A", 134.0, 6), track("steady", "8A", 128.5, 6)];
    let ranked = rank(&from, &pool, &Rules::default());
    assert_eq!(ranked[0].track_id, "steady");
}

#[test]
fn a_set_that_climbs_prefers_one_level_up() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![
        track("up", "8A", 128.0, 6),
        track("flat", "8A", 128.0, 5),
        track("down", "8A", 128.0, 3),
    ];
    let ranked = rank(&from, &pool, &Rules::default());
    assert_eq!(ranked[0].track_id, "up");
    assert_eq!(ranked[0].energy_change, 1);
    // A fall is still offered: taking the room down is a choice.
    assert!(ranked.iter().any(|c| c.track_id == "down"));
}

#[test]
fn holding_still_can_be_asked_for_instead() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![track("up", "8A", 128.0, 6), track("flat", "8A", 128.0, 5)];
    let rules = Rules {
        climbing: false,
        ..Rules::default()
    };
    assert_eq!(rank(&from, &pool, &rules)[0].track_id, "flat");
}

/// An unanalysed track has no key, and whether that is allowed is the
/// caller's call — but it never outranks a real harmonic match.
#[test]
fn an_unknown_key_is_gated_by_the_rule_and_ranked_last_when_allowed() {
    let from = track("seed", "8A", 128.0, 5);
    let pool = vec![keyless("mystery", 128.0), track("match", "8A", 128.0, 6)];

    let strict = rank(&from, &pool, &Rules::default());
    assert_eq!(strict.len(), 1);
    assert_eq!(strict[0].track_id, "match");

    let loose = rank(
        &from,
        &pool,
        &Rules {
            require_key: false,
            ..Rules::default()
        },
    );
    assert_eq!(loose.len(), 2);
    assert_eq!(loose[0].track_id, "match");
    assert_eq!(loose[1].key_relation, "unknown");
}

/// The seed counts toward the length, and a track is never played twice.
#[test]
fn a_chain_reaches_the_asked_for_length_without_repeating_itself() {
    let seed = track("a", "8A", 128.0, 4);
    let pool = vec![
        track("b", "8A", 128.0, 5),
        track("c", "8B", 129.0, 6),
        track("d", "9B", 130.0, 7),
        track("e", "9A", 131.0, 8),
    ];
    // Four-minute records: 20 minutes is five of them, seed included.
    let chain = build(&seed, &pool, 20.0, &Rules::default());
    assert_eq!(chain.order.len(), 5);
    assert_eq!(chain.total_ms, 5 * 240_000);
    assert!(!chain.ran_out);
    let unique: std::collections::HashSet<&String> = chain.order.iter().collect();
    assert_eq!(unique.len(), chain.order.len(), "a track was played twice");
    assert_eq!(chain.transitions.len(), 4, "one transition per link");
}

/// Asking for more than the library can mix says so rather than looping.
#[test]
fn a_chain_that_runs_out_of_compatible_records_says_so() {
    let seed = track("a", "8A", 128.0, 5);
    let pool = vec![track("b", "8A", 128.0, 6), track("far", "3A", 128.0, 6)];
    let chain = build(&seed, &pool, 120.0, &Rules::default());
    assert!(chain.ran_out);
    assert_eq!(chain.order, vec!["a".to_string(), "b".to_string()]);
}

/// Built twice from the same library, a set is the same set.
#[test]
fn building_the_same_set_twice_gives_the_same_order() {
    let seed = track("a", "8A", 128.0, 5);
    let pool = vec![
        track("x", "8A", 128.0, 6),
        track("y", "8A", 128.0, 6),
        track("z", "8A", 128.0, 6),
    ];
    let first = build(&seed, &pool, 16.0, &Rules::default());
    let second = build(&seed, &pool, 16.0, &Rules::default());
    assert_eq!(first.order, second.order);
}

/// A track with no tempo cannot be mixed into or out of by this logic.
#[test]
fn a_track_with_no_tempo_is_not_offered() {
    let from = track("seed", "8A", 128.0, 5);
    let mut no_tempo = track("silent", "8A", 128.0, 5);
    no_tempo.tempo_display = None;
    no_tempo.tempo_raw = None;
    assert!(rank(&from, &[no_tempo], &Rules::default()).is_empty());
}

/// Zero minutes is the seed and nothing else, not an empty answer.
#[test]
fn a_zero_length_set_is_just_the_seed() {
    let seed = track("a", "8A", 128.0, 5);
    let chain = build(&seed, &[track("b", "8A", 128.0, 6)], 0.0, &Rules::default());
    assert_eq!(chain.order, vec!["a".to_string()]);
    assert!(chain.transitions.is_empty());
}
