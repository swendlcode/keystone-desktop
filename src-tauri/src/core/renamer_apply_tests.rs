//! ENGINE.md risk 12, end to end through [`plan`] and [`apply`].
//!
//! `tags/rename` unit-tests the naming rules and the move in isolation. These
//! drive the wired pair against a real database and a real directory instead,
//! because that is where the two meet: the name is composed from a row, the
//! file moves, and the row has to end up agreeing with the directory entry.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::db::test_support::Harness;
use crate::models::{RenamePlan, RenamePlanItem, Settings};

/// One analysed track on disk: A minor at 126, which the default pattern
/// composes as ` - 8A - 126`.
struct Lib {
    dir: TempDir,
    h: Harness,
    id: String,
}

fn library(name: &str) -> Lib {
    let dir = TempDir::new().expect("temp dir");
    let h = Harness::new();
    fs::write(dir.path().join(name), b"audio").expect("write file");
    let id = h.add(&dir.path().join(name).to_string_lossy());
    h.set_analysis_columns(&id, "key_root = 9, key_mode = 'minor', tempo_display = 126.4");
    Lib { dir, h, id }
}

impl Lib {
    fn track(&self) -> crate::models::Track {
        self.h.tracks.get_by_id(&self.id).expect("get").expect("track")
    }

    fn set(&self, assignments: &str) {
        self.h.set_analysis_columns(&self.id, assignments);
    }

    /// The directory as the filesystem spells it. Never `exists()`: on APFS that
    /// answers yes to a spelling that is not there.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.dir.path())
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn plan(&self) -> RenamePlan {
        plan(&[self.track()], &Settings::default()).expect("plan")
    }

    fn apply(&self, built: &RenamePlan) -> RenameReport {
        apply(built, &self.h.tracks).expect("apply")
    }
}

fn skip_reason(built: &RenamePlan) -> Option<String> {
    assert_eq!(built.items.len(), 1);
    built.items[0].skipped.clone()
}

/// A case-only rename on a case-insensitive filesystem. The direct call is
/// either a no-op or an error and both look like success, so it goes via a
/// staging name — and the proof is the directory entry, not `exists()`.
#[test]
fn a_case_only_rename_lands_and_the_row_follows_it() {
    let lib = library("track.mp3");
    let to = lib.dir.path().join("Track.mp3").to_string_lossy().into_owned();
    let built = RenamePlan {
        items: vec![RenamePlanItem {
            track_id: lib.id.clone(),
            from_path: lib.track().path,
            to_path: to.clone(),
            skipped: None,
        }],
    };

    let report = lib.apply(&built);

    assert_eq!((report.renamed, report.failed), (1, 0));
    assert_eq!(lib.names(), vec!["Track.mp3".to_string()]);
    assert_eq!(lib.track().path, to);
    assert_eq!(lib.track().filename, "Track.mp3");
    assert_eq!(lib.h.tracks.open_file_ops().unwrap().len(), 0);
}

/// A crash between the two halves of a case-only rename leaves the file under
/// the staging name, which neither side of the journal row mentions. Without
/// the staged hop the track is reported missing while its audio sits next to
/// it, which is the one way journal-first can still lose a file.
#[test]
fn recovers_a_case_only_rename_interrupted_between_its_two_steps() {
    let dir = TempDir::new().expect("temp dir");
    let h = Harness::new();
    let from = dir.path().join("track.mp3").to_string_lossy().into_owned();
    let to = dir.path().join("Track.mp3").to_string_lossy().into_owned();
    // What `paths::move_file` parks the file under between its two renames.
    fs::write(dir.path().join(".keystone-4242-99-Track.mp3"), b"audio").expect("write");
    let track_id = h.add(&from);
    h.tracks.record_file_op(&track_id, &from, &to).expect("record");

    let n = recover_journal(&h.tracks).expect("recover");

    assert_eq!(n, 1);
    let names: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["Track.mp3".to_string()], "the staged file was settled");
    assert_eq!(h.tracks.get_by_id(&track_id).unwrap().unwrap().path, to);
    assert_eq!(h.tracks.open_file_ops().unwrap().len(), 0);
}

/// NFD vs NFC. macOS hands back decomposed names, so the composed name out of
/// the database and the decomposed one off the disk are the same file — and a
/// plain string compare would rename it again, which on APFS is a no-op at best.
#[test]
fn an_nfd_spelling_of_the_composed_name_is_not_a_rename() {
    let lib = library("Caf\u{e9} - 8A - 126.mp3");
    // The pre-rename name as macOS would have handed it over: e + combining
    // acute, where the row's own path carries the composed form.
    lib.set("original_filename = 'Cafe\u{301}.mp3'");
    let composed = lib.track().path;
    assert!(
        !composed.contains('\u{301}'),
        "the test is pointless unless the two spellings differ as strings"
    );

    let built = lib.plan();

    assert_eq!(skip_reason(&built).as_deref(), Some("Already named correctly."));
    let report = lib.apply(&built);
    assert_eq!((report.renamed, report.skipped), (0, 1));
    assert_eq!(lib.names(), vec!["Caf\u{e9} - 8A - 126.mp3".to_string()]);
}

/// Idempotence: the second pass has to see its own work. `Midnight - 8A - 126 -
/// 8A - 126.mp3` is one re-scan away otherwise, and renaming is the only
/// irreversible thing the app does.
#[test]
fn a_key_and_tempo_suffix_can_never_be_applied_twice() {
    let lib = library("Midnight.mp3");

    assert_eq!(lib.apply(&lib.plan()).renamed, 1);
    assert_eq!(lib.names(), vec!["Midnight - 8A - 126.mp3".to_string()]);
    assert_eq!(
        lib.track().original_filename.as_deref(),
        Some("Midnight.mp3"),
        "the pre-rename name is what makes the second pass exact"
    );

    // Second pass, from the row the first one wrote.
    let again = lib.plan();
    assert_eq!(skip_reason(&again).as_deref(), Some("Already named correctly."));
    let report = lib.apply(&again);
    assert_eq!((report.renamed, report.skipped), (0, 1));
    assert_eq!(lib.names(), vec!["Midnight - 8A - 126.mp3".to_string()]);
}

/// Windows-illegal characters never reach the disk, and a slash in particular
/// must not quietly become a directory level.
#[test]
fn illegal_characters_never_reach_the_disk() {
    let lib = library("x.mp3");
    lib.set("original_filename = 'AC/DC: Back*Slash?.mp3'");

    assert_eq!(lib.apply(&lib.plan()).renamed, 1);

    let names = lib.names();
    assert_eq!(names.len(), 1, "a slash would have made a subdirectory: {names:?}");
    let name = &names[0];
    assert!(name.ends_with(" - 8A - 126.mp3"), "{name}");
    assert!(
        !name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*']),
        "{name}"
    );

    // The reserved device names are asserted directly: with a pattern applied
    // the stem can never be a bare `CON`, so the plan has no way to exercise
    // the rule that matters on Windows.
    assert_eq!(sanitize_component("CON.mp3"), "_CON.mp3");
    assert_eq!(sanitize_component("nul"), "_nul");
    assert_eq!(sanitize_component("COM1 - 8A.wav"), "COM1 - 8A.wav");
}

/// The 255-byte component limit, against a real filesystem: a name over it is
/// `ENAMETOOLONG`, not a silent truncation. The bytes come off the original
/// segment, never off the key and tempo the rename exists to add.
#[test]
fn the_component_limit_truncates_the_original_and_keeps_the_suffix() {
    let lib = library("x.mp3");
    // Multi-byte on purpose: a byte-wise cut would land mid-character and the
    // name would not be valid UTF-8 at all.
    lib.set(&format!("original_filename = '{}.mp3'", "é".repeat(200)));

    assert_eq!(lib.apply(&lib.plan()).renamed, 1);

    let names = lib.names();
    assert_eq!(names.len(), 1);
    let name = &names[0];
    assert!(name.len() <= MAX_COMPONENT_BYTES, "{} bytes", name.len());
    assert!(name.ends_with(" - 8A - 126.mp3"), "{name}");
    assert!(
        name.trim_end_matches(" - 8A - 126.mp3").chars().all(|c| c == 'é'),
        "every kept character is whole: {name}"
    );
    assert_eq!(lib.track().filename, *name);
}
