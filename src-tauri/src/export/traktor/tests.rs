use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::export::testkit;
use crate::export::ExportTarget;
use crate::models::{CueKind, KeyMode, Settings};

const FIXTURE: &str = include_str!("fixtures/collection.nml");

/// The fixture's first entry, addressed the way the exporter addresses it.
fn one() -> LocationKey {
    location_key(Path::new("/Volumes/Music/DJ/one.mp3"))
}

fn two() -> LocationKey {
    location_key(Path::new("/Volumes/Music/DJ/two.mp3"))
}

fn track_for(path: &str) -> ExportTrack {
    let mut t = testkit::export_track(path);
    t.track.path = path.to_string();
    t.track.tempo_display = Some(128.0);
    t
}

fn edits_for(path: &str, settings: &Settings) -> HashMap<LocationKey, EntryEdit> {
    let track = track_for(path);
    let mut map = HashMap::new();
    map.insert(location_key(Path::new(path)), entry_edit(&track, settings));
    map
}

fn fixture_on_disk(dir: &Path) -> PathBuf {
    let dest = dir.join("collection.nml");
    std::fs::write(&dest, FIXTURE).unwrap();
    dest
}

#[test]
fn a_run_with_no_matches_returns_the_document_byte_for_byte() {
    let (out, stats) = rewrite_document(FIXTURE, &HashMap::new(), false).unwrap();
    assert_eq!(out, FIXTURE, "an untouched collection must not be reformatted");
    assert_eq!(stats.entries_seen, 2);
    assert_eq!(stats.entries_matched, 0);
    assert_eq!(stats.entries_written, 0);
}

#[test]
fn every_element_keystone_does_not_model_survives_the_splice() {
    let edits = edits_for("/Volumes/Music/DJ/one.mp3", &Settings::default());
    let (out, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_matched, 1);

    crate::export::safety::assert_preserved(FIXTURE.as_bytes(), out.as_bytes(), rewrite::MODELLED)
        .expect("an unmodelled element was dropped");
    // Named explicitly as well, because these are the ones that cost a user
    // their analysis if a reserialising pass ever creeps back in.
    for carried in [
        "AUDIO_ID=\"AQIBAQEBAQEBAQ\"",
        "<MODIFICATION_INFO AUTHOR_TYPE=\"user\"></MODIFICATION_INFO>",
        "<LOUDNESS PEAK_DB=\"-0.300000\"",
        "COVERARTID=\"010/ABC\"",
        "<STEMS></STEMS>",
        "<FLAGS VALUE=\"0\"></FLAGS>",
        "<SUBNODES COUNT=\"0\"></SUBNODES>",
    ] {
        assert!(out.contains(carried), "{carried} was lost");
    }
}

#[test]
fn the_match_is_the_location_triplet_and_a_different_volume_is_a_different_track() {
    let settings = Settings::default();
    // Same directory and filename, different disk.
    let edits = edits_for("/Volumes/Backup/DJ/one.mp3", &settings);
    let (out, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_matched, 0, "cues must not land on the wrong song");
    assert_eq!(out, FIXTURE);

    let edits = edits_for("/Volumes/Music/DJ/one.mp3", &settings);
    let (_, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_matched, 1);
}

#[test]
fn cues_the_grid_and_the_key_text_are_spliced_into_the_matched_entry() {
    let edits = edits_for("/Volumes/Music/DJ/one.mp3", &Settings::default());
    let (out, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();

    // Two cues plus the grid marker.
    assert_eq!(stats.cues_written, 3);
    assert!(out.contains("NAME=\"AutoGrid\" DISPL_ORDER=\"0\" TYPE=\"4\""));
    assert!(out.contains("NAME=\"Intro\""));
    assert!(out.contains("NAME=\"Drop\""));
    assert!(out.contains("REPEATS=\"-1\""));
    // Key Text is the default target, and the BPM is made to agree with ours or
    // every imported cue drifts.
    assert!(out.contains("KEY=\"8A\""));
    assert!(out.contains("BPM=\"128.000000\""));
    assert!(!out.contains("BPM=\"126.000000\""));
    // The second entry was not matched, so its own tempo is untouched.
    assert!(out.contains("BPM=\"140.000000\""));
}

#[test]
fn musical_key_is_left_alone_unless_the_advanced_option_is_on() {
    let off = edits_for("/Volumes/Music/DJ/one.mp3", &Settings::default());
    let (out, _) = rewrite_document(FIXTURE, &off, true).unwrap();
    assert!(
        out.contains("<MUSICAL_KEY VALUE=\"21\"></MUSICAL_KEY>"),
        "Traktor's own key field must not move by default"
    );

    let mut settings = Settings::default();
    settings.write_traktor_musical_key = true;
    let on = edits_for("/Volumes/Music/DJ/one.mp3", &settings);
    let (out, _) = rewrite_document(FIXTURE, &on, true).unwrap();
    // A minor is root 9, minor: chromatic from C, minors after the majors.
    let expected = musical_key_value(9, KeyMode::Minor);
    assert!(out.contains(&format!("<MUSICAL_KEY VALUE=\"{expected}\">")));
}

#[test]
fn musical_key_is_inserted_when_the_entry_has_none() {
    let mut settings = Settings::default();
    settings.write_traktor_musical_key = true;
    let edits = edits_for("/Volumes/Music/DJ/two.mp3", &settings);
    let (out, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_written, 1);
    assert_eq!(out.matches("<MUSICAL_KEY").count(), 2);
}

#[test]
fn a_track_that_already_has_cues_is_untouched_unless_overwrite_is_on() {
    let settings = Settings::default();
    let edits = edits_for("/Volumes/Music/DJ/two.mp3", &settings);

    let (out, stats) = rewrite_document(FIXTURE, &edits, false).unwrap();
    assert_eq!(stats.entries_matched, 1);
    assert_eq!(stats.entries_skipped, 1);
    assert_eq!(stats.entries_written, 0);
    assert_eq!(stats.cues_existing, 1, "the hand-set cue, not the grid marker");
    assert_eq!(stats.cues_replaced, 0);
    assert_eq!(out, FIXTURE, "a skipped entry is not rewritten at all");

    let (out, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_skipped, 0);
    assert_eq!(stats.entries_written, 1);
    assert!(!out.contains("NAME=\"Hand set\""));
    assert!(out.contains("NAME=\"Intro\""));
    // Our own grid replaced theirs rather than being added beside it.
    assert_eq!(out.matches("NAME=\"AutoGrid\"").count(), 1);
}

#[test]
fn the_entries_attribute_stays_in_sync_with_the_children() {
    let edits = edits_for("/Volumes/Music/DJ/one.mp3", &Settings::default());
    let (out, _) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert!(out.contains("<COLLECTION ENTRIES=\"2\">"));

    // A collection that arrived with the wrong count is corrected, because
    // Traktor trusts the attribute over the children.
    let wrong = FIXTURE.replace("ENTRIES=\"2\"", "ENTRIES=\"7\"");
    let (out, _) = rewrite_document(&wrong, &edits, true).unwrap();
    assert!(out.contains("<COLLECTION ENTRIES=\"2\">"));
}

#[test]
fn the_start_attribute_follows_the_one_unit_constant() {
    let edits = edits_for("/Volumes/Music/DJ/one.mp3", &Settings::default());
    let (out, _) = rewrite_document(FIXTURE, &edits, true).unwrap();
    let expected = format!("START=\"{:.6}\"", start_attr(15_234.0));
    assert!(out.contains(&expected), "{expected} missing");
    // The assumption under test: milliseconds. If a real install says seconds,
    // one constant changes and this assertion changes with it.
    assert_eq!(unresolved::START_UNITS_PER_SECOND, 1000.0);
    assert!(out.contains("START=\"15234.000000\""));
}

#[test]
fn a_loop_is_a_cue_with_a_length() {
    let mut track = track_for("/Volumes/Music/DJ/one.mp3");
    track.cues = vec![{
        let mut c = testkit::cue(3, 30_000.0, CueKind::Loop);
        c.end_ms = Some(45_000.0);
        c
    }];
    let mut edits = HashMap::new();
    edits.insert(one(), entry_edit(&track, &Settings::default()));
    let (out, _) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert!(out.contains(&format!("LEN=\"{:.6}\"", start_attr(15_000.0))));
    assert!(out.contains(&format!("HOTCUE=\"{}\"", hotcue_attr(3))));
}



#[test]
fn a_plan_leaves_the_file_exactly_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let dest = fixture_on_disk(dir.path());
    let before = std::fs::read(&dest).unwrap();

    let tracks = vec![track_for("/Volumes/Music/DJ/one.mp3")];
    let report = run(
        &testkit::request(ExportTarget::Traktor, true),
        &tracks,
        &dest,
        &Settings::default(),
        Mode::Plan,
    )
    .unwrap();

    assert_eq!(report.tracks_matched, 1);
    assert_eq!(report.cues_to_write, 3);
    assert_eq!(report.written, 0);
    assert_eq!(std::fs::read(&dest).unwrap(), before);
}

#[test]
fn a_write_replaces_the_file_and_verifies_it() {
    let dir = tempfile::tempdir().unwrap();
    let dest = fixture_on_disk(dir.path());
    let tracks = vec![
        track_for("/Volumes/Music/DJ/one.mp3"),
        track_for("/Volumes/Music/DJ/missing.mp3"),
    ];
    let report = run(
        &testkit::request(ExportTarget::Traktor, true),
        &tracks,
        &dest,
        &Settings::default(),
        Mode::Write,
    )
    .unwrap();

    assert_eq!(report.tracks_matched, 1);
    assert_eq!(report.tracks_missing, 1);
    assert_eq!(report.written, 1);
    assert_eq!(report.verified, 1);
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].reason.contains("not in the Traktor collection"));

    let after = std::fs::read_to_string(&dest).unwrap();
    assert!(after.contains("NAME=\"Intro\""));
    assert!(after.contains("<COLLECTION ENTRIES=\"2\">"));
    // Still a collection Keystone can read back, which is what verification
    // means here.
    rewrite_document(&after, &HashMap::new(), false).unwrap();
}

#[test]
fn a_volumeless_track_matches_only_when_the_name_is_unambiguous() {
    // A path we cannot assign a volume to: Traktor still has it under a volume
    // name of its own, and dir plus filename is all we can compare.
    let mut key = two();
    key.volume = String::new();
    let mut edits = HashMap::new();
    edits.insert(key, entry_edit(&track_for("/Volumes/Music/DJ/two.mp3"), &Settings::default()));
    let (_, stats) = rewrite_document(FIXTURE, &edits, true).unwrap();
    assert_eq!(stats.entries_matched, 1);

    // Ambiguous: another of our tracks claims the same directory and filename
    // on a named volume. The fallback is disabled rather than picking one, so
    // the cues go nowhere instead of onto the wrong song.
    let mut ambiguous = HashMap::new();
    let mut blind = two();
    blind.volume = String::new();
    let mut other_disk = two();
    other_disk.volume = "Backup".into();
    for key in [blind, other_disk] {
        ambiguous.insert(key, EntryEdit::default());
    }
    assert_eq!(ambiguous.len(), 2);
    let (out, stats) = rewrite_document(FIXTURE, &ambiguous, true).unwrap();
    assert_eq!(stats.entries_matched, 0);
    assert_eq!(out, FIXTURE);
}
