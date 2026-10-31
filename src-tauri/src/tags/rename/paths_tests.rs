use super::*;

#[test]
fn a_case_only_change_is_recognised_and_a_real_change_is_not() {
    assert!(is_case_only_change(
        Path::new("/m/track.mp3"),
        Path::new("/m/Track.mp3")
    ));
    assert!(!is_case_only_change(
        Path::new("/m/track.mp3"),
        Path::new("/m/track.mp3")
    ));
    assert!(!is_case_only_change(
        Path::new("/m/track.mp3"),
        Path::new("/m/track - 8A.mp3")
    ));
}

#[test]
fn nfd_and_nfc_spellings_of_one_name_compare_equal() {
    // "Café": composed é, then e + combining acute. macOS writes the second
    // and a plain string compare calls them different files.
    let composed = Path::new("/m/Caf\u{e9}.mp3");
    let decomposed = Path::new("/m/Cafe\u{301}.mp3");
    assert_ne!(
        composed.to_string_lossy(),
        decomposed.to_string_lossy(),
        "the test is pointless if these are already equal"
    );
    assert!(same_path_text(composed, decomposed));
    // And it is not a case-only change, which would send it the long way
    // round for nothing.
    assert!(!is_case_only_change(composed, decomposed));
}

#[test]
fn identity_falls_back_to_text_when_neither_file_exists() {
    let a = Path::new("/nowhere/Caf\u{e9}.mp3");
    let b = Path::new("/nowhere/Cafe\u{301}.mp3");
    assert!(is_same_file(a, b));
    assert!(!is_same_file(a, Path::new("/nowhere/other.mp3")));
}

#[test]
fn the_staging_name_is_hidden_and_in_the_same_directory() {
    let staged = staging_path(Path::new("/m/Track.mp3"));
    assert_eq!(staged.parent(), Path::new("/m/Track.mp3").parent());
    let name = staged.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with(".keystone-"), "{name}");
    assert!(name.ends_with("Track.mp3"), "{name}");
}

#[test]
fn a_case_only_rename_goes_through_a_staging_name_and_lands() {
    // The hazard risk 12 names first: APFS is case-insensitive by default, so
    // the direct rename is either a no-op or an error, and both look like
    // success to a caller that only checks for an error.
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("track.mp3");
    let to = dir.path().join("Track.mp3");
    std::fs::write(&from, b"audio").unwrap();

    move_file(&from, &to).expect("the case-only rename");

    // Read the directory rather than asking `exists()`, which on a
    // case-insensitive filesystem answers yes to both spellings.
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["Track.mp3".to_string()]);
    assert_eq!(std::fs::read(&to).unwrap(), b"audio");
}

#[test]
fn a_rename_onto_a_different_existing_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("a.mp3");
    let to = dir.path().join("b.mp3");
    std::fs::write(&from, b"mine").unwrap();
    std::fs::write(&to, b"theirs").unwrap();

    assert!(move_file(&from, &to).is_err());
    // Neither file was touched: this is the only irreversible operation in the
    // app and it does not overwrite.
    assert_eq!(std::fs::read(&from).unwrap(), b"mine");
    assert_eq!(std::fs::read(&to).unwrap(), b"theirs");
}
