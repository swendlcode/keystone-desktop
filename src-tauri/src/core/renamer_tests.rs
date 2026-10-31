//! Journal recovery, against a real database and real files on disk.
//!
//! Each test stages the crash it is about: a row in `file_ops`, a `tracks` row
//! pointing at one of the two paths, and only one of the two paths actually
//! present. What recovery is allowed to do is decided by the filesystem, so
//! nothing here fakes the filesystem.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::db::test_support::Harness;

/// A library of one track, with `from` on disk unless `on_disk` says otherwise.
struct Staged {
    _dir: TempDir,
    h: Harness,
    track_id: String,
    from: String,
    to: String,
}

fn stage(db_path: &str, on_disk: &[&str]) -> Staged {
    let dir = TempDir::new().expect("temp dir");
    let h = Harness::new();
    let at = |n: &str| dir.path().join(n).to_string_lossy().into_owned();
    let (from, to) = (at("A.mp3"), at("B - 8A - 128.mp3"));
    for name in on_disk {
        fs::write(dir.path().join(name), b"audio").expect("write file");
    }
    let track_id = h.add(&at(db_path));
    Staged { _dir: dir, h, track_id, from, to }
}

/// Records the staged rename. `renamed` stages the crash *after* `fs::rename`.
fn journal(s: &Staged, renamed: bool) -> String {
    let id = s.h.tracks.record_file_op(&s.track_id, &s.from, &s.to).expect("record");
    if renamed {
        s.h.tracks.mark_file_op_renamed(&id).expect("mark");
    }
    id
}

fn track_of(s: &Staged) -> crate::models::Track {
    s.h.tracks.get_by_id(&s.track_id).expect("get").expect("track")
}

fn path_of(s: &Staged) -> String {
    track_of(s).path
}

fn open_rows(s: &Staged) -> usize {
    s.h.tracks.open_file_ops().expect("open ops").len()
}

/// Crash after `fs::rename`, before the UPDATE. The file is at the new path and
/// the row still points at the old one: this is the case the journal exists for.
#[test]
fn finishes_a_rename_the_database_never_saw() {
    let s = stage("A.mp3", &["B - 8A - 128.mp3"]);
    journal(&s, true);

    let n = recover_journal(&s.h.tracks).expect("recover");

    assert_eq!(n, 1, "the two sides disagreed, so one row was repaired");
    assert_eq!(path_of(&s), s.to);
    let t = track_of(&s);
    assert_eq!(
        t.original_filename.as_deref(),
        Some("A.mp3"),
        "the pre-rename name is recorded, so the inverse strip stays exact"
    );
    assert_eq!(t.filename, "B - 8A - 128.mp3");
    assert_eq!(open_rows(&s), 0, "the row is cleared once settled");
}

/// Crash before `fs::rename`. The file never moved, so the database is already
/// right and recovery must not touch it.
#[test]
fn leaves_a_rename_that_never_happened_alone() {
    let s = stage("A.mp3", &["A.mp3"]);
    journal(&s, false);

    let n = recover_journal(&s.h.tracks).expect("recover");

    assert_eq!(n, 0);
    assert_eq!(path_of(&s), s.from);
    assert_eq!(open_rows(&s), 0);
}

/// The one direction a naive replay gets wrong: the row says the file moved, the
/// disk says it did not. The filesystem wins and the path goes back.
#[test]
fn puts_the_path_back_when_the_file_did_not_move() {
    let s = stage("B - 8A - 128.mp3", &["A.mp3"]);
    journal(&s, true);

    let n = recover_journal(&s.h.tracks).expect("recover");

    assert_eq!(n, 1);
    assert_eq!(path_of(&s), s.from);
}

/// Both sides already agree and only the DELETE was missed. Nothing to repair.
#[test]
fn clears_a_row_both_sides_already_agree_on() {
    let s = stage("B - 8A - 128.mp3", &["B - 8A - 128.mp3"]);
    journal(&s, true);

    assert_eq!(recover_journal(&s.h.tracks).expect("recover"), 0);
    assert_eq!(open_rows(&s), 0);
}

/// Neither path is on disk. Recovery cannot know which side won, so it must not
/// write a path at all; the reconciler is what marks the track missing.
#[test]
fn refuses_to_guess_when_the_file_is_gone() {
    let s = stage("A.mp3", &[]);
    journal(&s, false);

    let n = recover_journal(&s.h.tracks).expect("recover");

    assert_eq!(n, 0);
    assert_eq!(path_of(&s), s.from, "the stored path is left as it was");
    assert_eq!(open_rows(&s), 0);
}

/// The target name is taken by a different file, so the rename cannot have
/// happened: the source is still where the database says it is.
#[test]
fn treats_an_occupied_target_as_a_rename_that_did_not_run() {
    let s = stage("A.mp3", &["A.mp3", "B - 8A - 128.mp3"]);
    journal(&s, false);

    assert_eq!(recover_journal(&s.h.tracks).expect("recover"), 0);
    assert_eq!(path_of(&s), s.from);
}

/// A case-only rename. On a case-insensitive filesystem both paths "exist" and
/// are the same file, so only the directory entry's spelling says which side
/// won; on a case-sensitive one the plain existence check already does.
#[test]
fn reads_a_case_only_rename_from_the_directory_entry() {
    let dir = TempDir::new().expect("temp dir");
    let h = Harness::new();
    let from = dir.path().join("track.mp3").to_string_lossy().into_owned();
    let to = dir.path().join("Track.mp3").to_string_lossy().into_owned();
    fs::write(dir.path().join("Track.mp3"), b"audio").expect("write");
    let track_id = h.add(&from);
    let id = h.tracks.record_file_op(&track_id, &from, &to).expect("record");
    h.tracks.mark_file_op_renamed(&id).expect("mark");

    let n = recover_journal(&h.tracks).expect("recover");

    assert_eq!(n, 1);
    assert_eq!(
        h.tracks.get_by_id(&track_id).unwrap().unwrap().path,
        to,
        "the on-disk spelling is the one that won"
    );
}

/// Recovery runs on every launch, so running it twice must be the same as
/// running it once.
#[test]
fn is_idempotent() {
    let s = stage("A.mp3", &["B - 8A - 128.mp3"]);
    journal(&s, true);

    assert_eq!(recover_journal(&s.h.tracks).expect("first"), 1);
    assert_eq!(recover_journal(&s.h.tracks).expect("second"), 0);
    assert_eq!(path_of(&s), s.to);
}

/// `done` rows carry no decision. They are deleted, not replayed.
#[test]
fn purges_finished_rows() {
    let s = stage("A.mp3", &["A.mp3"]);
    let id = journal(&s, false);
    s.h.db
        .get()
        .unwrap()
        .execute("UPDATE file_ops SET state = 'done' WHERE id = ?1", [&id])
        .expect("mark done");

    assert_eq!(recover_journal(&s.h.tracks).expect("recover"), 0);
    let left: i64 = s
        .h
        .db
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM file_ops", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

/// An op this build does not understand is reported and kept, not guessed at
/// and not quietly dropped.
#[test]
fn keeps_an_unknown_op_instead_of_guessing() {
    let s = stage("A.mp3", &["A.mp3"]);
    s.h.db
        .get()
        .unwrap()
        .execute(
            "INSERT INTO file_ops (id, track_id, op, from_path, to_path, state, created_at) \
             VALUES ('x', ?1, 'teleport', ?2, ?3, 'intended', 1)",
            rusqlite::params![&s.track_id, &s.from, &s.to],
        )
        .expect("insert");

    assert_eq!(recover_journal(&s.h.tracks).expect("recover"), 0);
    assert_eq!(open_rows(&s), 1, "left for a build that knows what it means");
}

/// A row whose track was removed from the library afterwards is stale, not a
/// disagreement.
#[test]
fn clears_a_row_whose_track_is_gone() {
    let s = stage("A.mp3", &["B - 8A - 128.mp3"]);
    journal(&s, false);
    s.h.tracks.delete(&s.track_id).expect("delete track");

    assert_eq!(recover_journal(&s.h.tracks).expect("recover"), 0);
    assert_eq!(open_rows(&s), 0);
}

#[test]
fn identity_not_string_equality() {
    let dir = TempDir::new().expect("temp dir");
    let a = dir.path().join("one.mp3");
    let b = dir.path().join("two.mp3");
    fs::write(&a, b"audio").expect("write");
    fs::write(&b, b"audio").expect("write");

    assert!(is_same_file(&a, &a));
    assert!(!is_same_file(&a, &b), "same bytes is not the same file");

    let missing = dir.path().join("gone.mp3");
    assert!(
        is_same_file(&missing, &missing),
        "an unopenable path falls back to the normalised compare"
    );
    assert!(!is_same_file(&missing, &a));
}

#[test]
fn nfd_and_nfc_spellings_are_one_path() {
    // "Café.mp3" composed, then decomposed.
    assert!(same_path("/m/Caf\u{e9}.mp3", "/m/Cafe\u{301}.mp3"));
    assert!(!same_path("/m/Cafe.mp3", "/m/Caf\u{e9}.mp3"));
}

/// Two open rows for the same file: a chain of renames caught mid-flight. The
/// disk is in exactly one of those states and recovery must settle on it, in
/// journal order, with the first row's `original_filename` surviving the second.
#[test]
fn settles_a_chain_of_rows_on_the_state_the_disk_is_in() {
    let dir = TempDir::new().expect("temp dir");
    let h = Harness::new();
    let a = dir.path().join("A.mp3").to_string_lossy().into_owned();
    let b = dir.path().join("B.mp3").to_string_lossy().into_owned();
    let c = dir.path().join("C.mp3").to_string_lossy().into_owned();
    // A -> B ran; B -> C did not.
    fs::write(dir.path().join("B.mp3"), b"audio").expect("write");
    let track_id = h.add(&a);
    let first = h.tracks.record_file_op(&track_id, &a, &b).expect("record");
    h.tracks.mark_file_op_renamed(&first).expect("mark");
    let second = h.tracks.record_file_op(&track_id, &b, &c).expect("record");
    // Both rows are written inside the same second, so the order has to survive
    // a tie on `created_at`; these make the intended order explicit.
    let conn = h.db.get().unwrap();
    conn.execute("UPDATE file_ops SET created_at = 10 WHERE id = ?1", [&first])
        .unwrap();
    conn.execute("UPDATE file_ops SET created_at = 20 WHERE id = ?1", [&second])
        .unwrap();
    drop(conn);

    recover_journal(&h.tracks).expect("recover");

    let t = h.tracks.get_by_id(&track_id).unwrap().unwrap();
    assert_eq!(t.path, b, "the disk is at B, so the track is");
    assert_eq!(
        t.original_filename.as_deref(),
        Some("A.mp3"),
        "the first replay records the original and the second must not overwrite it"
    );
    assert_eq!(h.tracks.open_file_ops().unwrap().len(), 0);
}
