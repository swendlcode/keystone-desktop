use crate::db::test_support::Harness;
use crate::models::IndexStatus;

#[test]
fn adding_a_root_twice_promotes_it_rather_than_failing() {
    let h = Harness::new();
    let first = h.library.add_watched("/Users/dj/Music/").unwrap();
    assert!(first.is_active);

    // Every migration path here is remove-then-re-add, so this has to be
    // idempotent rather than a UNIQUE violation.
    let second = h.library.add_watched("/Users/dj/Music/").unwrap();
    assert_eq!(second.path, first.path);
    assert_eq!(h.library.list_watched().unwrap().len(), 1);
}

#[test]
fn a_roots_count_covers_its_own_subtree_and_not_a_lookalike_sibling() {
    let h = Harness::new();
    h.library.add_watched("/Users/dj/Music/").unwrap();
    h.add("/Users/dj/Music/a.mp3");
    h.add("/Users/dj/Music/sub/b.mp3");
    h.add("/Users/dj/Musicals/c.mp3");

    let watched = h.library.list_watched().unwrap();
    assert_eq!(watched[0].track_count, 2);
}

#[test]
fn a_missing_row_does_not_count_towards_a_root() {
    let h = Harness::new();
    h.library.add_watched("/Users/dj/Music/").unwrap();
    let a = h.add("/Users/dj/Music/a.mp3");
    h.add("/Users/dj/Music/b.mp3");
    h.tracks
        .set_index_status(&a, IndexStatus::Missing, None)
        .unwrap();
    assert_eq!(h.library.list_watched().unwrap()[0].track_count, 1);
}

#[test]
fn removing_a_root_takes_its_tracks_and_leaves_the_rest_alone() {
    let h = Harness::new();
    h.library.add_watched("/Users/dj/Music/").unwrap();
    h.library.add_watched("/Volumes/USB/").unwrap();
    h.add("/Users/dj/Music/a.mp3");
    h.add("/Users/dj/Music/sub/b.mp3");
    let kept = h.add("/Volumes/USB/c.mp3");

    assert_eq!(h.library.remove_watched("/Users/dj/Music/").unwrap(), 2);
    assert_eq!(h.library.list_watched().unwrap().len(), 1);
    assert!(h.tracks.get_by_id(&kept).unwrap().is_some());
    assert_eq!(
        h.tracks.count(&crate::models::TrackFilters::default()).unwrap(),
        1
    );
}

#[test]
fn marking_missing_is_soft_and_scoped_to_one_subtree() {
    let h = Harness::new();
    let inside = h.add("/Volumes/USB/a.mp3");
    let outside = h.add("/Users/dj/Music/b.mp3");

    assert_eq!(h.library.mark_missing_under("/Volumes/USB/").unwrap(), 1);
    assert_eq!(
        h.tracks.get_by_id(&inside).unwrap().unwrap().index_status,
        IndexStatus::Missing
    );
    assert_eq!(
        h.tracks.get_by_id(&outside).unwrap().unwrap().index_status,
        IndexStatus::Indexed
    );
    // The row is still there: an unmounted drive is the ordinary cause, and a
    // DJ library living on one is normal.
    assert!(h.tracks.get_by_id(&inside).unwrap().is_some());

    // Running it twice reports nothing further to do.
    assert_eq!(h.library.mark_missing_under("/Volumes/USB/").unwrap(), 0);
}

#[test]
fn clean_cache_is_the_hard_pass_and_prunes_what_nothing_points_at() {
    let h = Harness::new();
    let dir = tempfile::TempDir::new().unwrap();

    let gone = h.add_with("/m/gone.mp3", |r| r.content_hash = Some("aaaa1111".into()));
    h.add_with("/m/kept.mp3", |r| r.content_hash = Some("bbbb2222".into()));
    for hash in ["aaaa1111", "bbbb2222"] {
        h.analysis
            .store(&crate::db::test_support::stub_analysis(hash))
            .unwrap();
        let path = crate::core::peaks::cache_path(dir.path(), hash);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"peaks").unwrap();
    }
    h.tracks
        .set_index_status(&gone, IndexStatus::Missing, None)
        .unwrap();

    let report = h.library.clean_cache(dir.path()).unwrap();
    assert_eq!(report.tracks_removed, 1);
    assert_eq!(report.analysis_rows_removed, 1);
    assert_eq!(report.peak_files_removed, 1);
    assert_eq!(report.bytes_freed, 5);

    assert!(h.analysis.lookup("bbbb2222", 1).unwrap().is_some());
    assert!(h.analysis.lookup("aaaa1111", 1).unwrap().is_none());
    assert!(crate::core::peaks::cache_path(dir.path(), "bbbb2222").exists());
}

#[test]
fn folders_with_counts_lists_the_parents_of_live_tracks() {
    let h = Harness::new();
    h.add("/Users/dj/Music/House/a.mp3");
    h.add("/Users/dj/Music/House/b.mp3");
    let gone = h.add("/Users/dj/Music/Techno/c.mp3");

    let folders = h.library.folders_with_counts().unwrap();
    assert_eq!(folders.len(), 2);
    assert_eq!(folders[0].path, "/Users/dj/Music/House");
    assert_eq!(folders[0].name, "House");
    assert_eq!(folders[0].track_count, 2);

    h.tracks
        .set_index_status(&gone, IndexStatus::Missing, None)
        .unwrap();
    let folders = h.library.folders_with_counts().unwrap();
    assert_eq!(folders.len(), 1);
}
