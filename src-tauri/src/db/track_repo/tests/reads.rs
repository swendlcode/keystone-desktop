//! Lookups, the Camelot wheel filter and its per-segment counts.

use super::common::{query, wheel_library};
use crate::db::test_support::Harness;
use crate::models::{IndexStatus, TagSnapshot, TrackFilters};

#[test]
fn an_upserted_row_reads_back_through_every_lookup() {
    let h = Harness::new();
    let id = h.add_with("/m/track.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("Aphex Twin".into()),
            title: Some("Windowlicker".into()),
            ..TagSnapshot::default()
        });
        r.content_hash = Some("deadbeef".into());
    });

    let track = h.tracks.get_by_id(&id).unwrap().expect("by id");
    assert_eq!(track.path, "/m/track.mp3");
    assert_eq!(track.artist.as_deref(), Some("Aphex Twin"));
    assert_eq!(track.index_status, IndexStatus::Indexed);
    assert_eq!(track.analysis_version, 0);

    assert_eq!(
        h.tracks.get_by_path("/m/track.mp3").unwrap().unwrap().id,
        id
    );
    assert_eq!(h.tracks.get_many(&[id]).unwrap().len(), 1);
}

#[test]
fn a_re_walk_keeps_the_id_and_never_blanks_what_it_cannot_read() {
    let h = Harness::new();
    let id = h.add_with("/m/track.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("Floating Points".into()),
            ..TagSnapshot::default()
        });
    });
    h.set_analysis_columns(&id, "energy = 7, camelot = '8A', analysis_version = 3");

    // The second walk could not read tags at all, which is what a locked or
    // unreadable file looks like.
    let same = h.add_with("/m/track.mp3", |r| r.tags = None);
    assert_eq!(same, id, "a re-walk must not change a row's identity");

    let track = h.tracks.get_by_id(&id).unwrap().unwrap();
    assert_eq!(track.artist.as_deref(), Some("Floating Points"));
    assert_eq!(track.energy, Some(7), "the walk overwrote analysed energy");
    assert_eq!(track.analysis_version, 3);
    assert_eq!(h.tracks.count(&TrackFilters::default()).unwrap(), 1);
}

#[test]
fn the_wheel_query_returns_the_segment_its_relative_and_one_either_way() {
    let h = Harness::new();
    wheel_library(&h);

    let page = h
        .tracks
        .search(&query(TrackFilters {
            camelot: Some("8A".into()),
            camelot_include_compatible: true,
            ..TrackFilters::default()
        }))
        .unwrap();
    let mut got: Vec<String> = page.tracks.iter().filter_map(|t| t.camelot.clone()).collect();
    got.sort();
    assert_eq!(got, ["7A", "8A", "8B", "9A"]);
    assert_eq!(page.total, 4, "the count must match the page's filter");

    // Without the compatible set it is the one segment only.
    let page = h
        .tracks
        .search(&query(TrackFilters {
            camelot: Some("8A".into()),
            camelot_include_compatible: false,
            ..TrackFilters::default()
        }))
        .unwrap();
    assert_eq!(page.total, 1);
}

#[test]
fn the_wheel_query_uses_an_index_rather_than_scanning_the_table() {
    // The Camelot-leading partial indexes exist for exactly this query. If the
    // plan ever says SCAN, the predicate and the index have drifted apart.
    let h = Harness::new();
    wheel_library(&h);
    let (where_sql, params) = crate::db::query_builder::build_where(&TrackFilters {
        camelot: Some("8A".into()),
        camelot_include_compatible: true,
        ..TrackFilters::default()
    });

    let conn = h.db.get().unwrap();
    let mut stmt = conn
        .prepare(&format!(
            "EXPLAIN QUERY PLAN SELECT id FROM tracks WHERE {where_sql}"
        ))
        .unwrap();
    let plan: Vec<String> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            r.get::<_, String>(3)
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let plan = plan.join(" | ");
    assert!(
        plan.contains("idx_tracks_camelot"),
        "the wheel filter is not using a Camelot index: {plan}"
    );
}

#[test]
fn wheel_counts_are_all_twenty_four_segments_and_ignore_the_selection() {
    let h = Harness::new();
    wheel_library(&h);
    let unkeyed = h.add("/m/no-key.mp3");
    let _ = unkeyed;

    let counts = h
        .tracks
        .wheel_counts(&TrackFilters {
            // A selection must not empty the other segments, or clicking one
            // segment makes the rest of the wheel look like an empty library.
            camelot: Some("8A".into()),
            camelot_include_compatible: true,
            ..TrackFilters::default()
        })
        .unwrap();

    assert_eq!(counts.counts.len(), 24);
    assert_eq!(counts.counts[0].0, "1A");
    assert_eq!(counts.counts[23].0, "12B");
    let lookup = |label: &str| {
        counts
            .counts
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, n)| *n)
            .unwrap()
    };
    assert_eq!(lookup("8A"), 1);
    assert_eq!(lookup("5B"), 1);
    assert_eq!(lookup("11B"), 0);
    assert_eq!(counts.unkeyed, 1);
}

#[test]
fn the_walk_skips_a_file_whose_row_is_newer_than_the_file() {
    let h = Harness::new();
    h.add("/m/unchanged.mp3");
    h.add("/m/edited.mp3");

    let fresh = h
        .tracks
        .get_by_id(&crate::core::hasher::hash_path("/m/unchanged.mp3"))
        .unwrap()
        .unwrap()
        .updated_at;

    let skippable = h
        .tracks
        .unchanged_since(&[
            ("/m/unchanged.mp3".into(), fresh - 60),
            ("/m/edited.mp3".into(), fresh + 60),
            ("/m/never-seen.mp3".into(), 0),
        ])
        .unwrap();
    assert_eq!(skippable, ["/m/unchanged.mp3"]);
}

#[test]
fn needing_analysis_finds_the_stale_and_leaves_the_missing_alone() {
    let h = Harness::new();
    let fresh = h.add("/m/fresh.mp3");
    h.set_analysis_columns(&fresh, "analysis_version = 4");
    h.add("/m/stale.mp3");
    let gone = h.add("/m/gone.mp3");
    h.tracks
        .set_index_status(&gone, IndexStatus::Missing, None)
        .unwrap();

    let pending = h.tracks.needing_analysis(4, 100).unwrap();
    let paths: Vec<&str> = pending.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["/m/stale.mp3"]);
}
