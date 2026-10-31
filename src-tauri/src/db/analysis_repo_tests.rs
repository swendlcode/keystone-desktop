//! Tests for [`super`]. Split out to keep the repository under the module
//! line cap; the `#[path]` attribute keeps it the same module.

use crate::db::test_support::{analysis_result, stub_analysis, Harness};
use crate::models::{DetectedRegion, KeyMode, RegionKind};

#[test]
fn a_stored_result_comes_back_whole() {
    let h = Harness::new();
    h.analysis.store(&stub_analysis("abc123")).unwrap();

    let got = h.analysis.lookup("abc123", 1).unwrap().expect("cache hit");
    assert_eq!(got.content_hash, "abc123");
    assert_eq!(got.key.camelot, "8B");
    assert_eq!(got.tempo.raw_bpm, 128.0);
    assert_eq!(got.energy.energy, 6);
    assert_eq!(got.levels.lufs_integrated, -8.2);
}

#[test]
fn a_different_analysis_version_is_a_miss_rather_than_a_stale_hit() {
    let h = Harness::new();
    h.analysis.store(&stub_analysis("abc123")).unwrap();
    assert!(h.analysis.lookup("abc123", 2).unwrap().is_none());
    assert!(h.analysis.lookup("other", 1).unwrap().is_none());
}

#[test]
fn a_duplicate_or_renamed_file_hits_the_cache_because_the_key_is_the_content() {
    // Two paths, one piece of audio. This is what makes a re-scan nearly
    // free and the rename feature free instead of a re-analysis storm.
    let h = Harness::new();
    h.add_with("/m/320.mp3", |r| r.content_hash = Some("same".into()));
    h.add_with("/m/promo/320.mp3", |r| r.content_hash = Some("same".into()));
    h.analysis.store(&stub_analysis("same")).unwrap();

    assert!(h.analysis.lookup("same", 1).unwrap().is_some());
}

#[test]
fn storing_the_same_key_twice_replaces_rather_than_failing() {
    let h = Harness::new();
    h.analysis.store(&stub_analysis("abc123")).unwrap();
    h.analysis
        .store(&analysis_result("abc123", 9, KeyMode::Minor, 174.0))
        .unwrap();
    let got = h.analysis.lookup("abc123", 1).unwrap().unwrap();
    assert_eq!(got.key.camelot, "8A");
    assert_eq!(got.tempo.raw_bpm, 174.0);
}

#[test]
fn a_payload_an_older_build_cannot_read_is_a_miss_not_a_crash() {
    let h = Harness::new();
    h.db.get()
        .unwrap()
        .execute(
            "INSERT INTO analysis_cache (content_hash, analysis_version, \
             energy_model_version, payload, created_at) VALUES ('x', 1, 1, '{\"nope\":1}', 0)",
            [],
        )
        .unwrap();
    // Re-analysing one track is cheaper than refusing to open the library.
    assert!(h.analysis.lookup("x", 1).unwrap().is_none());
}

#[test]
fn pruning_keeps_the_versions_it_was_told_to_keep() {
    let h = Harness::new();
    for version in [1, 2, 3] {
        let mut r = stub_analysis("abc123");
        r.analysis_version = version;
        h.analysis.store(&r).unwrap();
    }
    assert_eq!(h.analysis.prune(&[3]).unwrap(), 2);
    assert!(h.analysis.lookup("abc123", 3).unwrap().is_some());

    // An empty keep list would mean "throw the whole cache away", which is
    // never what a caller means.
    assert_eq!(h.analysis.prune(&[]).unwrap(), 0);
    assert!(h.analysis.lookup("abc123", 3).unwrap().is_some());
}

#[test]
fn orphaned_cache_rows_are_the_ones_no_track_points_at() {
    let h = Harness::new();
    h.add_with("/m/live.mp3", |r| r.content_hash = Some("live".into()));
    h.analysis.store(&stub_analysis("live")).unwrap();
    h.analysis.store(&stub_analysis("orphan")).unwrap();

    assert_eq!(h.analysis.prune_orphans().unwrap(), 1);
    assert!(h.analysis.lookup("live", 1).unwrap().is_some());
}

#[test]
fn regions_replace_whole_and_read_back_in_time_order() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    let region = |start_ms: f64, kind: RegionKind, energy: u8| DetectedRegion {
        start_ms,
        end_ms: start_ms + 30_000.0,
        bar_index: Some((start_ms / 2000.0) as i32),
        bars: Some(16),
        kind,
        energy: Some(energy),
        key_root: Some(9),
        key_mode: Some(KeyMode::Minor),
    };

    h.analysis
        .replace_regions(
            &id,
            &[
                region(60_000.0, RegionKind::Drop, 9),
                region(0.0, RegionKind::Intro, 3),
            ],
        )
        .unwrap();

    let regions = h.analysis.regions(&id).unwrap();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].kind, RegionKind::Intro);
    assert_eq!(regions[0].energy, Some(3));
    assert_eq!(regions[1].kind, RegionKind::Drop);
    assert_eq!(regions[1].key_mode, Some(KeyMode::Minor));

    h.analysis
        .replace_regions(&id, &[region(0.0, RegionKind::Breakdown, 2)])
        .unwrap();
    assert_eq!(h.analysis.regions(&id).unwrap().len(), 1);
}

#[test]
fn deleting_a_track_takes_its_regions() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.analysis
        .replace_regions(
            &id,
            &[DetectedRegion {
                start_ms: 0.0,
                end_ms: 1000.0,
                bar_index: None,
                bars: None,
                kind: RegionKind::Unknown,
                energy: None,
                key_root: None,
                key_mode: None,
            }],
        )
        .unwrap();
    h.tracks.delete(&id).unwrap();
    assert!(h.analysis.regions(&id).unwrap().is_empty());
}
