//! The search box, the sortable columns, the path scope and `Improve Tracks`.

use super::common::query;
use crate::db::test_support::Harness;
use crate::models::{TagSnapshot, TrackFilters};

#[test]
fn search_sorts_pages_and_counts_consistently() {
    let h = Harness::new();
    for (path, artist) in [
        ("/m/a.mp3", "zz top"),
        ("/m/b.mp3", "Aphex Twin"),
        ("/m/c.mp3", "moderat"),
    ] {
        h.add_with(path, |r| {
            r.tags = Some(TagSnapshot {
                artist: Some(artist.into()),
                ..TagSnapshot::default()
            })
        });
    }

    let page = h.tracks.search(&query(TrackFilters::default())).unwrap();
    let artists: Vec<&str> = page.tracks.iter().filter_map(|t| t.artist.as_deref()).collect();
    // NOCASE, so lower-case `zz top` sorts last rather than first.
    assert_eq!(artists, ["Aphex Twin", "moderat", "zz top"]);
    assert_eq!(page.total, 3);

    let mut q = query(TrackFilters::default());
    q.limit = 2;
    q.offset = 2;
    let page = h.tracks.search(&q).unwrap();
    assert_eq!(page.tracks.len(), 1);
    assert_eq!(page.total, 3, "the total is the filter's, not the page's");
}

#[test]
fn full_text_search_finds_a_track_by_a_prefix_of_any_indexed_field() {
    let h = Harness::new();
    h.add_with("/m/one.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("Four Tet".into()),
            title: Some("Baby".into()),
            ..TagSnapshot::default()
        })
    });
    h.add_with("/m/two.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("Burial".into()),
            title: Some("Archangel".into()),
            ..TagSnapshot::default()
        })
    });

    for (q, expected) in [("four", 1), ("arch", 1), ("four tet baby", 1), ("nothing", 0)] {
        let page = h
            .tracks
            .search(&query(TrackFilters {
                query: q.into(),
                ..TrackFilters::default()
            }))
            .unwrap();
        assert_eq!(page.total, expected, "query {q:?}");
    }
}

#[test]
fn a_path_scope_reads_one_root_and_not_its_lookalike_sibling() {
    let h = Harness::new();
    h.add("/m/Music/a.mp3");
    h.add("/m/Music/deep/b.mp3");
    h.add("/m/Musicals/c.mp3");

    let page = h
        .tracks
        .search(&query(TrackFilters {
            path_prefix: Some("/m/Music/".into()),
            ..TrackFilters::default()
        }))
        .unwrap();
    assert_eq!(page.total, 2);

    let paged = h.tracks.existing_paths("/m/Music/", 0, 10).unwrap();
    assert_eq!(paged, ["/m/Music/a.mp3", "/m/Music/deep/b.mp3"]);
    assert_eq!(h.tracks.existing_paths("/m/Music/", 1, 10).unwrap().len(), 1);
}

#[test]
fn the_improvable_filter_is_what_the_improve_tracks_view_shows() {
    let h = Harness::new();
    let unsure = h.add("/m/unsure.mp3");
    h.set_analysis_columns(&unsure, "analysis_version = 1, key_confidence = 0.4");
    let ambiguous = h.add("/m/ambiguous.mp3");
    h.set_analysis_columns(
        &ambiguous,
        "analysis_version = 1, key_confidence = 0.9, key_mode_ambiguous = 1",
    );
    let clipped = h.add("/m/clipped.mp3");
    h.set_analysis_columns(
        &clipped,
        "analysis_version = 1, key_confidence = 0.9, clipped_peaks = 12",
    );
    let fine = h.add("/m/fine.mp3");
    h.set_analysis_columns(&fine, "analysis_version = 1, key_confidence = 0.95");
    // A track nothing has analysed yet is improvable by definition.
    h.add("/m/unanalysed.mp3");

    let n = h
        .tracks
        .count(&TrackFilters {
            improvable_only: true,
            ..TrackFilters::default()
        })
        .unwrap();
    assert_eq!(n, 4);
}
