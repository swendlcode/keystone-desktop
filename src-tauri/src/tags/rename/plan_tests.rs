use std::path::{Path, PathBuf};

use super::*;
use crate::models::{KeyNotation, RenamePattern};
use crate::tags::test_support::analysed;

fn track_at(dir: &Path, name: &str) -> Track {
    let mut t = analysed();
    let path = dir.join(name);
    t.id = name.to_string();
    t.path = path.to_string_lossy().into_owned();
    t.filename = name.to_string();
    t
}

fn settings(pattern: RenamePattern) -> Settings {
    Settings { rename_pattern: pattern, ..Settings::default() }
}

fn only(built: &RenamePlan) -> &RenamePlanItem {
    assert_eq!(built.items.len(), 1);
    &built.items[0]
}

fn touch(path: &Path) {
    std::fs::write(path, b"audio").unwrap();
}

#[test]
fn the_recommended_pattern_produces_the_name_the_spec_shows() {
    let dir = tempfile::tempdir().unwrap();
    let t = track_at(dir.path(), "Midnight.mp3");
    touch(Path::new(&t.path));

    let built = plan(&[t], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    let item = only(&built);
    assert_eq!(item.skipped, None);
    assert_eq!(
        PathBuf::from(&item.to_path).file_name().unwrap(),
        "Midnight - 8A - 126.mp3"
    );
}

#[test]
fn a_key_suffix_can_never_be_applied_twice() {
    // Idempotence, risk 12. The second pass has to see the name as already
    // correct, or `Midnight - 8A - 126 - 8A - 126.mp3` is one re-scan away.
    let dir = tempfile::tempdir().unwrap();
    let mut t = track_at(dir.path(), "Midnight - 8A - 126.mp3");
    t.original_filename = Some("Midnight.mp3".to_string());
    touch(Path::new(&t.path));

    let built = plan(&[t.clone()], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    assert_eq!(only(&built).skipped.as_deref(), Some("Already named correctly."));

    // And with no `original_filename` recorded — a row from another tool —
    // the fallback strip has to reach the same answer.
    t.original_filename = None;
    let built = plan(&[t], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    assert_eq!(only(&built).skipped.as_deref(), Some("Already named correctly."));
}

#[test]
fn nothing_is_renamed_before_the_analysis_has_something_to_say() {
    let dir = tempfile::tempdir().unwrap();
    let mut t = track_at(dir.path(), "Midnight.mp3");
    touch(Path::new(&t.path));

    t.key_root = None;
    let built = plan(std::slice::from_ref(&t), &settings(RenamePattern::OriginalKey)).unwrap();
    assert_eq!(only(&built).skipped.as_deref(), Some("No key detected yet."));

    // A pattern that asks for a tempo waits for one rather than writing a name
    // that a later pass would have to correct.
    let mut t = track_at(dir.path(), "Midnight.mp3");
    t.tempo_display = None;
    t.tempo_raw = None;
    let built = plan(&[t], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    assert_eq!(only(&built).skipped.as_deref(), Some("No tempo detected yet."));
}

#[test]
fn a_collision_with_another_file_or_another_item_is_skipped_with_a_reason() {
    let dir = tempfile::tempdir().unwrap();
    let t = track_at(dir.path(), "Midnight.mp3");
    touch(Path::new(&t.path));
    touch(&dir.path().join("Midnight - 8A - 126.mp3"));

    let built = plan(&[t], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    assert_eq!(
        only(&built).skipped.as_deref(),
        Some("A different file already has that name.")
    );

    // Two different originals that compose to one name: the second one is
    // reported, not silently dropped and not allowed to overwrite.
    let dir = tempfile::tempdir().unwrap();
    let mut a = track_at(dir.path(), "a.mp3");
    a.original_filename = Some("Midnight.mp3".to_string());
    let mut b = track_at(dir.path(), "b.mp3");
    b.original_filename = Some("Midnight.mp3".to_string());
    touch(Path::new(&a.path));
    touch(Path::new(&b.path));

    let built = plan(&[a, b], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    assert_eq!(built.items[0].skipped, None);
    assert_eq!(
        built.items[1].skipped.as_deref(),
        Some("Another track in this batch would take the same name.")
    );
}

#[test]
fn illegal_characters_and_the_component_limit_are_handled_in_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    let mut t = track_at(dir.path(), "x.mp3");
    t.original_filename = Some(format!("{}:AC/DC.mp3", "n".repeat(300)));
    touch(Path::new(&t.path));

    let built = plan(&[t], &settings(RenamePattern::OriginalKeyTempo)).unwrap();
    let name = PathBuf::from(&only(&built).to_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    assert!(name.len() <= MAX_COMPONENT_BYTES, "{} bytes", name.len());
    // The key and tempo suffix is what the rename is *for*, so it survives the
    // truncation and the extension comes with it.
    assert!(name.ends_with(" - 8A - 126.mp3"), "{name}");
    assert!(!name.contains(':') && !name.contains('/'), "{name}");
}

#[test]
fn the_key_notation_setting_decides_the_suffix_rather_than_this_module() {
    let dir = tempfile::tempdir().unwrap();
    let t = track_at(dir.path(), "Midnight.mp3");
    touch(Path::new(&t.path));

    let mut s = settings(RenamePattern::KeyOriginal);
    s.key_notation = KeyNotation::Sharps;
    let built = plan(std::slice::from_ref(&t), &s).unwrap();
    assert_eq!(
        PathBuf::from(&only(&built).to_path).file_name().unwrap(),
        "Am - Midnight.mp3"
    );

    s.key_notation = KeyNotation::Camelot;
    s.pad_camelot_numbers = true;
    let built = plan(&[t], &s).unwrap();
    assert_eq!(
        PathBuf::from(&only(&built).to_path).file_name().unwrap(),
        "08A - Midnight.mp3"
    );
}
