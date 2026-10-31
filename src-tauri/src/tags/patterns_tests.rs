use super::*;
use crate::tags::test_support::{analysed, track};

#[test]
fn the_six_forms_read_exactly_as_the_spec_prints_them() {
    let t = analysed();
    let mut s = Settings::default();
    for (content, expected) in [
        (TagWriteContent::KeyOnly, "8A"),
        (TagWriteContent::EnergyWithWord, "Energy 7"),
        (TagWriteContent::KeyAndEnergyWithWord, "8A - Energy 7"),
        (TagWriteContent::EnergyOnly, "7"),
        (TagWriteContent::KeyAndEnergy, "8A - 7"),
        (TagWriteContent::KeyAndTempo, "8A - 126"),
    ] {
        s.tag_write_content = content;
        assert_eq!(compose(&t, &s).as_deref(), Some(expected), "{content:?}");
    }
}

#[test]
fn an_unanalysed_track_writes_nothing_rather_than_an_empty_marker() {
    let s = Settings::default();
    assert_eq!(compose(&track(), &s), None);
}

#[test]
fn a_missing_half_writes_the_other_half_without_a_dangling_separator() {
    let mut t = analysed();
    t.energy = None;
    let s = Settings {
        tag_write_content: TagWriteContent::KeyAndEnergyWithWord,
        ..Settings::default()
    };
    assert_eq!(compose(&t, &s).as_deref(), Some("8A"));
}

#[test]
fn re_tagging_twice_does_not_stack_markers() {
    let snapshot = TagSnapshot {
        title: Some("8A - Energy 7 - Midnight".into()),
        ..TagSnapshot::default()
    };
    let edit = apply_placement(
        &snapshot,
        "8A - Energy 8",
        Some("8A - Energy 7"),
        TagWritePlacement::FrontOfTitle,
    );
    assert_eq!(
        edit.fields,
        vec![(TagField::Title, Some("8A - Energy 8 - Midnight".into()))]
    );
}

#[test]
fn the_strip_is_exact_and_leaves_real_dashes_and_real_words_alone() {
    assert_eq!(strip_exact("8A - Energy 7 - Midnight", "8A - Energy 7"), "Midnight");
    assert_eq!(strip_exact("Midnight - 8A", "8A"), "Midnight");
    assert_eq!(strip_exact("8A", "8A"), "");
    // A title that is genuinely about energy, and one that genuinely has a
    // dash in it, both survive.
    assert_eq!(strip_exact("Energy - The Remix", "8A"), "Energy - The Remix");
    assert_eq!(
        strip_exact("8A - Energy 7 - Lights - Out", "8A - Energy 7"),
        "Lights - Out"
    );
}

#[test]
fn placements_land_where_the_spec_says() {
    let snapshot = TagSnapshot {
        artist: Some("Swendl".into()),
        title: Some("Midnight".into()),
        comment: Some("promo".into()),
        ..TagSnapshot::default()
    };
    let cases = [
        (TagWritePlacement::FrontOfArtist, TagField::Artist, "8A - Swendl"),
        (TagWritePlacement::FrontOfTitle, TagField::Title, "8A - Midnight"),
        (TagWritePlacement::EndOfTitle, TagField::Title, "Midnight - 8A"),
        (TagWritePlacement::FrontOfComment, TagField::Comment, "8A - promo"),
        (TagWritePlacement::OverwriteComment, TagField::Comment, "8A"),
    ];
    for (placement, field, expected) in cases {
        let edit = apply_placement(&snapshot, "8A", None, placement);
        assert_eq!(edit.fields, vec![(field, Some(expected.into()))], "{placement:?}");
    }
}

#[test]
fn an_empty_field_takes_the_marker_without_a_separator() {
    let edit = apply_placement(
        &TagSnapshot::default(),
        "8A",
        None,
        TagWritePlacement::FrontOfTitle,
    );
    assert_eq!(edit.fields, vec![(TagField::Title, Some("8A".into()))]);
}

#[test]
fn extra_spaces_go_and_clean_fields_are_not_rewritten() {
    let snapshot = TagSnapshot {
        artist: Some("  Swendl   feat.  Nobody ".into()),
        title: Some("Midnight".into()),
        ..TagSnapshot::default()
    };
    let edit = remove_extra_spaces(&snapshot);
    assert_eq!(
        edit.fields,
        vec![(TagField::Artist, Some("Swendl feat. Nobody".into()))]
    );
}

#[test]
fn global_replace_is_a_plain_case_sensitive_substring() {
    let snapshot = TagSnapshot {
        artist: Some("A feat. B".into()),
        ..TagSnapshot::default()
    };
    let edit = global_replace(&snapshot, &[TagField::Artist], "feat.", "ft.")
        .unwrap()
        .expect("it matched");
    assert_eq!(edit.fields, vec![(TagField::Artist, Some("A ft. B".into()))]);

    // Case-sensitive, and a miss is None rather than a pointless file write.
    assert!(global_replace(&snapshot, &[TagField::Artist], "FEAT.", "ft.")
        .unwrap()
        .is_none());
    assert!(global_replace(&snapshot, &[TagField::Album], "x", "y")
        .unwrap()
        .is_none());
    assert!(global_replace(&snapshot, &[TagField::Artist], "", "x").is_err());
}

#[test]
fn every_ticked_column_lands_in_one_edit() {
    // SPEC.md section 2 ticks columns, plural. One edit per file rather than one
    // per column: the write path opens, rewrites and verifies once per edit.
    let snapshot = TagSnapshot {
        artist: Some("A feat. B".into()),
        title: Some("Track feat. C".into()),
        album: Some("No needle here".into()),
        ..TagSnapshot::default()
    };
    let edit = global_replace(
        &snapshot,
        &[TagField::Artist, TagField::Title, TagField::Album],
        "feat.",
        "ft.",
    )
    .unwrap()
    .expect("two of the three matched");
    assert_eq!(
        edit.fields,
        vec![
            (TagField::Artist, Some("A ft. B".into())),
            (TagField::Title, Some("Track ft. C".into())),
        ]
    );

    // No column carries the needle, so nothing is written at all.
    assert!(global_replace(
        &snapshot,
        &[TagField::Album, TagField::Genre],
        "feat.",
        "ft."
    )
    .unwrap()
    .is_none());
}

#[test]
fn keep_existing_tempo_values_means_keep_them() {
    let mut t = analysed();
    let mut s = Settings {
        update_tempo_tag: true,
        keep_existing_tempo: true,
        ..Settings::default()
    };

    assert_eq!(tempo_tag_value(&t, &s).as_deref(), Some("126"));
    t.tag_bpm = Some(125.0);
    assert_eq!(tempo_tag_value(&t, &s), None);
    s.keep_existing_tempo = false;
    assert_eq!(tempo_tag_value(&t, &s).as_deref(), Some("126"));
    s.update_tempo_tag = false;
    assert_eq!(tempo_tag_value(&t, &s), None);
}
