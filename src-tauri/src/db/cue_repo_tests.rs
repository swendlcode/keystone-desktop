use crate::db::test_support::Harness;
use crate::models::{BeatgridMarker, CueEdit, CueKind, CuePoint, CueSource, DetectedCue};

fn detected(slot: u8, kind: CueKind, position_ms: f64) -> DetectedCue {
    DetectedCue {
        slot,
        kind,
        position_ms,
        end_ms: None,
        bar_index: Some(slot as i32 * 16),
        phrase_level: Some(16),
        label: Some(format!("{kind:?}")),
        confidence: 0.7,
    }
}

fn edit(track_id: &str, slot: u8, position_ms: f64) -> CueEdit {
    CueEdit {
        track_id: track_id.to_string(),
        slot,
        position_ms,
        end_ms: None,
        kind: None,
        label: None,
        color: None,
    }
}

#[test]
fn detected_cues_land_in_slot_order() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    let written = h
        .cues
        .replace_detected(
            &id,
            &[
                detected(2, CueKind::Breakdown, 60_000.0),
                detected(0, CueKind::Intro, 0.0),
                detected(1, CueKind::FirstBeat, 8_000.0),
            ],
        )
        .unwrap();
    assert_eq!(written, 3);

    let cues = h.cues.list_for_track(&id).unwrap();
    assert_eq!(cues.iter().map(|c| c.slot).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(cues[0].kind, CueKind::Intro);
    assert_eq!(cues[0].source, CueSource::Detected);
    assert_eq!(cues[2].phrase_level, Some(16));
}

#[test]
fn re_analysis_never_moves_a_marker_the_dj_dragged() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.cues
        .replace_detected(&id, &[detected(0, CueKind::Intro, 0.0), detected(1, CueKind::Drop, 60_000.0)])
        .unwrap();

    // The DJ drags CUE 2 somewhere they actually want it.
    let moved = h.cues.list_for_track(&id).unwrap()[1].id.clone();
    let after = h.cues.move_cue(&moved, 61_234.5).unwrap();
    assert_eq!(after.source, CueSource::User);
    assert_eq!(after.position_ms, 61_234.5);
    // The grid metadata described the detector's position, not this one.
    assert_eq!(after.bar_index, None);
    assert_eq!(after.confidence, None);

    // A second pass detects something different for the same slot.
    let written = h
        .cues
        .replace_detected(
            &id,
            &[detected(0, CueKind::Intro, 500.0), detected(1, CueKind::Drop, 90_000.0)],
        )
        .unwrap();
    assert_eq!(written, 1, "the user-held slot must be skipped, not refilled");

    let cues = h.cues.list_for_track(&id).unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].position_ms, 500.0);
    assert_eq!(
        cues[1].position_ms, 61_234.5,
        "re-analysis overwrote a user cue"
    );
    assert_eq!(cues[1].source, CueSource::User);
}

#[test]
fn a_slot_holds_one_cue_and_a_second_write_replaces_it() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    let first = h.cues.upsert_user(&edit(&id, 3, 1_000.0)).unwrap();
    assert_eq!(first.kind, CueKind::User);
    let second = h.cues.upsert_user(&edit(&id, 3, 2_000.0)).unwrap();

    assert_eq!(h.cues.list_for_track(&id).unwrap().len(), 1);
    assert_eq!(second.position_ms, 2_000.0);
    assert_eq!(second.slot, 3);
}

#[test]
fn removing_cue_points_is_scoped_by_who_put_them_there() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.cues
        .replace_detected(&id, &[detected(0, CueKind::Intro, 0.0)])
        .unwrap();
    h.cues.upsert_user(&edit(&id, 1, 1_000.0)).unwrap();

    // What a re-analysis does: clears only its own work.
    assert_eq!(h.cues.delete_for_track(&id, false).unwrap(), 1);
    let left = h.cues.list_for_track(&id).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].source, CueSource::User);

    // What the context menu's Remove Cue Points does.
    assert_eq!(h.cues.delete_for_track(&id, true).unwrap(), 1);
    assert!(h.cues.list_for_track(&id).unwrap().is_empty());
}

#[test]
fn cue_counts_come_back_in_one_query_for_the_visible_rows() {
    let h = Harness::new();
    let a = h.add("/m/a.mp3");
    let b = h.add("/m/b.mp3");
    let c = h.add("/m/c.mp3");
    h.cues
        .replace_detected(&a, &[detected(0, CueKind::Intro, 0.0), detected(1, CueKind::Drop, 1.0)])
        .unwrap();
    h.cues.upsert_user(&edit(&b, 0, 10.0)).unwrap();

    let mut counts = h
        .cues
        .counts_for_tracks(&[a.clone(), b.clone(), c.clone()])
        .unwrap();
    counts.sort();
    // A track with no cues is simply absent, which is what lets the table treat
    // a missing entry as zero without a row per track in the result.
    assert_eq!(counts.len(), 2);
    assert_eq!(counts.iter().map(|(_, n)| *n).sum::<usize>(), 3);
    assert!(!counts.iter().any(|(id, _)| *id == c));
}

#[test]
fn moving_a_cue_that_is_gone_is_an_error_rather_than_a_silent_no_op() {
    let h = Harness::new();
    assert!(h.cues.move_cue("not-a-cue", 100.0).is_err());
}

#[test]
fn the_beatgrid_is_replaced_whole_and_read_back_in_time_order() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    let marker = |position_ms: f64, bpm: f64| BeatgridMarker {
        id: String::new(),
        track_id: id.clone(),
        position_ms,
        bpm,
        beats_per_bar: 4,
        beat_in_bar: 1,
    };

    // Two markers for a constant tempo, not one per beat.
    h.cues
        .replace_beatgrid(&id, &[marker(240_000.0, 128.0), marker(120.5, 128.0)])
        .unwrap();
    let grid = h.cues.beatgrid(&id).unwrap();
    assert_eq!(grid.len(), 2);
    assert_eq!(grid[0].position_ms, 120.5);
    assert!(!grid[0].id.is_empty(), "the repository assigns the id");

    h.cues.replace_beatgrid(&id, &[marker(0.0, 174.0)]).unwrap();
    let grid = h.cues.beatgrid(&id).unwrap();
    assert_eq!(grid.len(), 1);
    assert_eq!(grid[0].bpm, 174.0);
}

#[test]
fn a_cue_point_round_trips_every_field_the_player_draws() {
    let h = Harness::new();
    let id = h.add("/m/track.mp3");
    h.cues
        .replace_detected(&id, &[detected(5, CueKind::Build, 123_456.75)])
        .unwrap();
    let cue: CuePoint = h.cues.list_for_track(&id).unwrap().remove(0);
    assert_eq!(cue.track_id, id);
    assert_eq!(cue.slot, 5);
    assert_eq!(cue.kind, CueKind::Build);
    // Milliseconds are REAL on purpose: rounding here loses positions on the
    // way out to rekordbox's 1/150 s frames.
    assert_eq!(cue.position_ms, 123_456.75);
    assert_eq!(cue.confidence, Some(0.7));
    assert!(cue.label.is_some());
}
