//! Write paths: the walk's upsert, analysis landing, provenance and the edits.

use crate::db::test_support::{analysis_result, Harness};
use crate::models::{KeyMode, TagSnapshot, ValueSource};

#[test]
fn a_user_override_survives_the_next_analysis() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.tracks
        .set_key(&id, 9, KeyMode::Minor, ValueSource::User)
        .unwrap();
    h.tracks.set_tempo(&id, 128.0, ValueSource::User).unwrap();

    h.tracks
        .apply_analysis(&id, &analysis_result("deadbeef", 0, KeyMode::Major, 174.0), (79.0, 192.0))
        .unwrap();

    let track = h.tracks.get_by_id(&id).unwrap().unwrap();
    assert_eq!(track.key_root, Some(9), "re-analysis overwrote a user key");
    assert_eq!(track.camelot.as_deref(), Some("8A"));
    assert_eq!(track.key_source, Some(ValueSource::User));
    assert_eq!(track.tempo_display, Some(128.0));
    assert_eq!(track.tempo_source, Some(ValueSource::User));
    // The engine's own opinion is still recorded, so the badge can admit doubt.
    assert_eq!(track.key_alt_root, Some(2));
    assert_eq!(track.analysis_version, 1);
}

#[test]
fn analysis_lands_on_a_track_with_no_overrides() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.tracks
        .apply_analysis(&id, &analysis_result("cafe", 0, KeyMode::Major, 174.0), (79.0, 192.0))
        .unwrap();

    let track = h.tracks.get_by_id(&id).unwrap().unwrap();
    assert_eq!(track.key_root, Some(0));
    assert_eq!(track.camelot.as_deref(), Some("8B"));
    assert_eq!(track.key_source, Some(ValueSource::Detected));
    assert_eq!(track.tempo_raw, Some(174.0));
    assert_eq!(track.tempo_display, Some(174.0), "174 is inside 79..192");
    assert_eq!(track.energy, Some(6));
}

#[test]
fn changing_the_tempo_range_refolds_from_the_database() {
    let h = Harness::new();
    let id = h.add("/m/dnb.mp3");
    h.tracks
        .apply_analysis(&id, &analysis_result("d1", 0, KeyMode::Minor, 174.0), (79.0, 192.0))
        .unwrap();
    assert_eq!(
        h.tracks.get_by_id(&id).unwrap().unwrap().tempo_display,
        Some(174.0)
    );

    // Narrowing the range must not touch `tempo_raw`: that is what makes the
    // setting free rather than a re-analysis.
    h.tracks.refold_tempo_display(70.0, 140.0).unwrap();
    let track = h.tracks.get_by_id(&id).unwrap().unwrap();
    assert_eq!(track.tempo_raw, Some(174.0));
    assert_eq!(track.tempo_display, Some(87.0));
    assert_eq!(track.tempo_octave_shift, Some(1));
}

#[test]
fn global_replace_counts_before_anything_is_written() {
    let h = Harness::new();
    for (path, artist) in [("/m/a.mp3", "DJ Sneak"), ("/m/b.mp3", "DJ Koze"), ("/m/c.mp3", "Pearson Sound")] {
        h.add_with(path, |r| {
            r.tags = Some(TagSnapshot {
                artist: Some(artist.into()),
                ..TagSnapshot::default()
            })
        });
    }
    use crate::models::TagField;
    assert_eq!(
        h.tracks.preview_replace(&[TagField::Artist], "DJ ", None).unwrap(),
        2
    );
    // Byte-exact, so a different case is a different string.
    assert_eq!(
        h.tracks.preview_replace(&[TagField::Artist], "dj ", None).unwrap(),
        0
    );
    assert_eq!(h.tracks.preview_replace(&[TagField::Artist], "", None).unwrap(), 0);
    assert_eq!(h.tracks.preview_replace(&[], "DJ ", None).unwrap(), 0);
}

#[test]
fn a_row_matching_two_columns_is_counted_once() {
    // The confirm dialog states how many files would be rewritten. A track whose
    // artist and title both carry the needle is one file, and a count of two is
    // a number the user cannot reconcile with the result they get.
    let h = Harness::new();
    h.add_with("/m/both.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("DJ Sneak".into()),
            title: Some("DJ Tool".into()),
            ..TagSnapshot::default()
        })
    });
    h.add_with("/m/title-only.mp3", |r| {
        r.tags = Some(TagSnapshot {
            artist: Some("Pearson Sound".into()),
            title: Some("DJ Edit".into()),
            ..TagSnapshot::default()
        })
    });
    use crate::models::TagField;
    assert_eq!(
        h.tracks
            .preview_replace(&[TagField::Artist, TagField::Title], "DJ ", None)
            .unwrap(),
        2
    );
}

#[test]
fn deleting_a_track_takes_its_cues_regions_and_grid_with_it() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.cues
        .upsert_user(&crate::models::CueEdit {
            track_id: id.clone(),
            slot: 0,
            position_ms: 1000.0,
            end_ms: None,
            kind: None,
            label: None,
            color: None,
        })
        .unwrap();
    h.cues
        .replace_beatgrid(
            &id,
            &[crate::models::BeatgridMarker {
                id: String::new(),
                track_id: id.clone(),
                position_ms: 0.0,
                bpm: 128.0,
                beats_per_bar: 4,
                beat_in_bar: 1,
            }],
        )
        .unwrap();

    h.tracks.delete(&id).unwrap();
    assert!(h.tracks.get_by_id(&id).unwrap().is_none());
    assert!(h.cues.list_for_track(&id).unwrap().is_empty());
    assert!(h.cues.beatgrid(&id).unwrap().is_empty());
}
