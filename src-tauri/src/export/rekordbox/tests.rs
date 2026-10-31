use std::path::{Path, PathBuf};

use super::*;
use crate::export::testkit;
use crate::export::{ExportTarget, ExportTrack};
use crate::models::CueKind;

/// A track whose file actually exists, because the exporter refuses to write an
/// entry whose `Location` points at nothing.
fn on_disk(dir: &Path, name: &str) -> ExportTrack {
    let path = dir.join(name);
    std::fs::write(&path, b"audio").unwrap();
    let mut t = testkit::export_track(path.to_str().unwrap());
    t.track.path = path.to_string_lossy().into_owned();
    t
}

fn write_to(dir: &Path, tracks: &[ExportTrack]) -> (PathBuf, String) {
    let dest = dir.join("keystone-rekordbox.xml");
    write_collection(&dest, tracks).unwrap();
    let text = std::fs::read_to_string(&dest).unwrap();
    (dest, text)
}

#[test]
fn the_location_is_a_percent_encoded_file_localhost_uri() {
    let uri = location_uri(Path::new("/Users/dj/Music/Night Shift (Club Mix).mp3"));
    assert_eq!(
        uri,
        "file://localhost/Users/dj/Music/Night%20Shift%20%28Club%20Mix%29.mp3"
    );
    // The separator and the Windows drive marker survive, or rekordbox silently
    // ignores the entry.
    let win = location_uri(Path::new(r"D:\Music\track.mp3"));
    assert_eq!(win, "file://localhost/D:/Music/track.mp3");
    // Non-ASCII is encoded byte by byte, so a UTF-8 filename survives.
    assert_eq!(
        location_uri(Path::new("/m/Café.mp3")),
        "file://localhost/m/Caf%C3%A9.mp3"
    );
}

#[test]
fn the_rating_is_one_of_rekordboxs_six_values() {
    let got: Vec<u16> = (0..=5).map(rating_attr).collect();
    assert_eq!(got, vec![0, 51, 102, 153, 204, 255]);
    // Out of range cannot produce an out-of-range attribute.
    assert_eq!(rating_attr(9), 255);
}

#[test]
fn a_constant_tempo_gets_two_markers_a_whole_number_of_bars_apart() {
    let t = testkit::export_track("/m/a.mp3");
    let markers = tempo_markers(&t);
    assert_eq!(markers.len(), 2, "one marker leaves rekordbox to infer the grid");
    assert_eq!(markers[0].bpm, markers[1].bpm);
    assert_eq!(markers[0].battito, markers[1].battito);
    assert_eq!(markers[0].metro, "4/4");

    let bar_s = 60.0 / 128.0 * 4.0;
    let gap = markers[1].inizio_s - markers[0].inizio_s;
    let bars = gap / bar_s;
    assert!((bars - bars.round()).abs() < 1e-9, "{gap} is not a bar line");
    // And the second marker stays inside the track.
    assert!(markers[1].inizio_s <= 192.078);
}

#[test]
fn no_grid_means_no_tempo_element_at_all() {
    let mut t = testkit::export_track("/m/a.mp3");
    t.beatgrid.clear();
    assert!(tempo_markers(&t).is_empty());
    let dir = tempfile::tempdir().unwrap();
    let (_, text) = write_to(dir.path(), &[t]);
    assert!(
        !text.contains("<TEMPO"),
        "a guessed grid at Inizio=0 drifts every cue"
    );
}

#[test]
fn a_variable_tempo_writes_one_marker_per_segment() {
    let mut t = testkit::export_track("/m/a.mp3");
    t.beatgrid = vec![testkit::grid(0.0, 128.0), testkit::grid(60_000.0, 140.0)];
    let markers = tempo_markers(&t);
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[1].bpm, 140.0);
}

#[test]
fn the_document_carries_a_collection_and_one_named_playlist_node() {
    let dir = tempfile::tempdir().unwrap();
    let tracks = vec![on_disk(dir.path(), "a.mp3"), on_disk(dir.path(), "b.mp3")];
    let (dest, text) = write_to(dir.path(), &tracks);

    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(text.contains("<DJ_PLAYLISTS Version=\"1.0.0\">"));
    assert!(text.contains("<COLLECTION Entries=\"2\">"));
    assert!(text.contains(&format!("Name=\"{PLAYLIST_NAME}\"")));
    assert!(text.contains("KeyType=\"0\""));
    // The playlist references the collection by TrackID.
    assert!(text.contains("<TRACK Key=\"1\"/>"));
    assert!(text.contains("<TRACK Key=\"2\"/>"));
    // Four cues over two tracks, and the document verifies against its own
    // counts rather than being trusted.
    assert_eq!(writer::verify_document(&dest, 2, 4).unwrap(), 2);
}

#[test]
fn cue_positions_are_written_in_seconds_with_the_slot_as_num() {
    let dir = tempfile::tempdir().unwrap();
    let tracks = vec![on_disk(dir.path(), "a.mp3")];
    let (_, text) = write_to(dir.path(), &tracks);
    // 15,234 ms is 15.234 s. A cue written in milliseconds would land at the
    // head of the track, which is the failure this assertion exists for.
    assert!(text.contains("Start=\"15.234\""), "{text}");
    assert!(text.contains("Num=\"0\""));
    assert!(text.contains("Num=\"1\""));
    assert!(text.contains("Name=\"Intro\""));
    assert!(text.contains("Name=\"Drop\""));
    assert!(text.contains("Type=\"0\""));
    assert!(text.contains("Tonality=\"8A\""));
    assert!(text.contains("AverageBpm=\"128.00\""));
    assert!(text.contains("Rating=\"153\""));
    // The undocumented colour attributes, written only because we have one.
    assert!(text.contains("Red=\"153\" Green=\"242\" Blue=\"63\""));
}

#[test]
fn a_loop_is_a_type_four_mark_with_an_end() {
    let dir = tempfile::tempdir().unwrap();
    let mut t = on_disk(dir.path(), "a.mp3");
    t.cues = vec![{
        let mut c = testkit::cue(2, 30_000.0, CueKind::Loop);
        c.end_ms = Some(45_000.0);
        c
    }];
    let (_, text) = write_to(dir.path(), &[t]);
    assert!(text.contains("Type=\"4\""));
    assert!(text.contains("End=\"45.000\""));
}

#[test]
fn metadata_is_escaped_rather_than_breaking_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut t = on_disk(dir.path(), "a.mp3");
    t.track.artist = Some("Bells & \"Whistles\" <Remix>".into());
    let (dest, text) = write_to(dir.path(), &[t]);
    assert!(text.contains("Bells &amp;"));
    assert!(writer::verify_document(&dest, 1, 2).is_ok());
}

#[test]
fn a_plan_writes_nothing_and_still_counts_the_work() {
    let dir = tempfile::tempdir().unwrap();
    let tracks = vec![on_disk(dir.path(), "a.mp3")];
    let dest = dir.path().join("out.xml");
    let request = testkit::request(ExportTarget::Rekordbox, false);

    let plan = run(&request, &tracks, &dest, Mode::Plan).unwrap();
    assert_eq!(plan.planned, 1);
    assert_eq!(plan.tracks_matched, 1);
    assert_eq!(plan.cues_to_write, 2);
    assert_eq!(plan.written, 0);
    assert!(!dest.exists(), "a dry run must not create the file");
}

#[test]
fn a_track_whose_file_moved_is_reported_not_written() {
    let dir = tempfile::tempdir().unwrap();
    let present = on_disk(dir.path(), "a.mp3");
    let gone = testkit::export_track("/keystone-no-such-dir/b.mp3");
    let dest = dir.path().join("out.xml");

    let report = run(
        &testkit::request(ExportTarget::Rekordbox, false),
        &[present, gone],
        &dest,
        Mode::Write,
    )
    .unwrap();
    assert_eq!(report.tracks_matched, 1);
    assert_eq!(report.tracks_missing, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.written, 1);
    assert_eq!(report.verified, 1);
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].reason.contains("no longer at this path"));
    // The missing track is absent from the document, not written as a dead
    // entry rekordbox would grey out with no explanation.
    let text = std::fs::read_to_string(&dest).unwrap();
    assert!(!text.contains("b.mp3"));
    assert!(text.contains("<COLLECTION Entries=\"1\">"));
}

#[test]
fn a_malformed_document_fails_verification_rather_than_being_reported_written() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("truncated.xml");
    std::fs::write(&dest, b"<DJ_PLAYLISTS><COLLECTION Entries=\"1\">").unwrap();
    let err = writer::verify_document(&dest, 1, 0).unwrap_err();
    assert!(err.to_string().contains("verification failed"), "{err}");
}
