use super::*;

#[test]
fn the_five_patterns_read_as_the_spec_prints_them() {
    let cases = [
        (RenamePattern::OriginalKey, "Midnight - 8A"),
        (RenamePattern::OriginalKeyTempo, "Midnight - 8A - 128"),
        (RenamePattern::KeyOriginal, "8A - Midnight"),
        (RenamePattern::KeyTempoOriginal, "8A - 128 - Midnight"),
        (RenamePattern::TempoKeyOriginal, "128 - 8A - Midnight"),
    ];
    for (pattern, expected) in cases {
        assert_eq!(compose_stem("Midnight", pattern, "8A", "128"), expected);
    }
}

#[test]
fn composing_twice_cannot_stack_a_suffix() {
    for pattern in [
        RenamePattern::OriginalKey,
        RenamePattern::OriginalKeyTempo,
        RenamePattern::KeyOriginal,
        RenamePattern::KeyTempoOriginal,
        RenamePattern::TempoKeyOriginal,
    ] {
        let once = compose_stem("Midnight", pattern, "8A", "128");
        let twice = compose_stem(&once, pattern, "8A", "128");
        assert_eq!(once, twice, "{pattern:?} stacked");
    }
}

#[test]
fn a_missing_tempo_leaves_no_dangling_separator() {
    assert_eq!(
        compose_stem("Midnight", RenamePattern::OriginalKeyTempo, "8A", ""),
        "Midnight - 8A"
    );
}

#[test]
fn the_strip_is_exact_when_the_original_name_is_known() {
    assert_eq!(
        strip_stem("Midnight - 8A - 128", Some("Midnight.mp3")),
        "Midnight"
    );
    // Even when the stored original itself contains the separator.
    assert_eq!(
        strip_stem("Lights - Out - 8A", Some("Lights - Out.aiff")),
        "Lights - Out"
    );
}

#[test]
fn the_fallback_strip_only_takes_segments_it_can_identify() {
    assert_eq!(strip_stem("Midnight - 8A - 128", None), "Midnight");
    assert_eq!(strip_stem("Midnight - 8A", None), "Midnight");
    assert_eq!(strip_stem("8A - Midnight", None), "Midnight");
    assert_eq!(strip_stem("8A - 128 - Midnight", None), "Midnight");
    assert_eq!(strip_stem("128 - 8A - Midnight", None), "Midnight");

    // A number with no key beside it is somebody's title, not our tempo.
    assert_eq!(strip_stem("Midnight - 128", None), "Midnight - 128");
    assert_eq!(strip_stem("1999 - Midnight", None), "1999 - Midnight");
    // And a track that is only a key keeps its name rather than becoming
    // the empty string.
    assert_eq!(strip_stem("8A", None), "8A");
}

#[test]
fn windows_illegal_characters_and_reserved_names_are_handled() {
    assert_eq!(sanitize_component("AC:DC / Back*Slash?.mp3"), "AC_DC _ Back_Slash_.mp3");
    assert_eq!(sanitize_component("CON.mp3"), "_CON.mp3");
    assert_eq!(sanitize_component("nul"), "_nul");
    assert_eq!(sanitize_component("COM1 - 8A.wav"), "COM1 - 8A.wav");
    // Trailing dots and spaces: Windows drops them, so Keystone drops them
    // first and records the name that will actually exist.
    assert_eq!(sanitize_component("Midnight. "), "Midnight");
    assert_eq!(sanitize_component("  "), "_");
    assert_eq!(sanitize_component("tab\there.mp3"), "tab_here.mp3");
}

#[test]
fn truncation_takes_bytes_off_the_original_and_never_off_the_suffix() {
    let long = "x".repeat(300);
    let out = truncate_component(&long, " - 8A - 128.mp3", MAX_COMPONENT_BYTES);
    assert!(out.len() <= MAX_COMPONENT_BYTES, "{} bytes", out.len());
    assert!(out.ends_with(" - 8A - 128.mp3"), "the suffix was eaten: {out}");

    // Short enough already: nothing is touched.
    assert_eq!(
        truncate_component("Midnight", " - 8A.mp3", MAX_COMPONENT_BYTES),
        "Midnight - 8A.mp3"
    );
}

#[test]
fn truncation_never_splits_a_multi_byte_character() {
    // Three bytes each, so a byte-wise cut lands mid-character.
    let original = "é".repeat(200);
    let out = truncate_component(&original, " - 8A.mp3", 100);
    assert!(out.len() <= 100);
    assert!(out.ends_with(" - 8A.mp3"));
    // The proof: it is still valid UTF-8 and every kept character is whole.
    assert!(out.trim_end_matches(" - 8A.mp3").chars().all(|c| c == 'é'));
}

#[test]
fn stems_and_extensions_split_the_way_the_rest_of_the_module_assumes() {
    assert_eq!(stem_of("Midnight - 8A.mp3"), "Midnight - 8A");
    assert_eq!(ext_of("Midnight - 8A.mp3"), ".mp3");
    assert_eq!(stem_of("no-extension"), "no-extension");
    assert_eq!(ext_of("no-extension"), "");
    assert_eq!(stem_of(".hidden"), ".hidden");
    assert_eq!(ext_of(".hidden"), "");
}
