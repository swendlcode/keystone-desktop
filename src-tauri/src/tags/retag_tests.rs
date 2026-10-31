use super::*;
use crate::models::{KeyNotation, TagWriteContent, TagWritePlacement};
use crate::tags::test_support::analysed;

fn snapshot() -> TagSnapshot {
    TagSnapshot {
        artist: Some("Swendl".into()),
        title: Some("Midnight".into()),
        comment: Some("promo".into()),
        ..TagSnapshot::default()
    }
}

fn field(edit: &TagEdit, field: TagField) -> Option<Option<String>> {
    edit.fields
        .iter()
        .find(|(f, _)| *f == field)
        .map(|(_, v)| v.clone())
}

#[test]
fn every_extra_lands_in_its_own_field() {
    let settings = Settings {
        update_initial_key_tag: true,
        write_energy_to_grouping: true,
        update_tempo_tag: true,
        keep_existing_tempo: false,
        ..Settings::default()
    };
    let edit = retag_edit(&analysed(), &snapshot(), &settings);

    // The default content and placement: key and energy with the word, in front
    // of the song name.
    assert_eq!(
        field(&edit, TagField::Title),
        Some(Some("8A - Energy 7 - Midnight".into()))
    );
    // The dedicated key field defaults to a TKEY-legal value, not Camelot.
    assert_eq!(field(&edit, TagField::InitialKey), Some(Some("Am".into())));
    assert_eq!(field(&edit, TagField::Grouping), Some(Some("7".into())));
    assert_eq!(field(&edit, TagField::Bpm), Some(Some("126".into())));
}

#[test]
fn the_master_switch_writes_nothing_at_all() {
    let settings = Settings { update_tags_enabled: false, ..Settings::default() };
    assert!(retag_edit(&analysed(), &snapshot(), &settings).fields.is_empty());
}

#[test]
fn camelot_in_the_key_field_is_an_opt_in_and_the_notation_decides() {
    let mut settings = Settings {
        key_notation: KeyNotation::Camelot,
        write_camelot_to_initial_key: true,
        ..Settings::default()
    };
    let edit = retag_edit(&analysed(), &snapshot(), &settings);
    assert_eq!(field(&edit, TagField::InitialKey), Some(Some("8A".into())));

    // Opt-in off: back to a value TKEY actually allows. Camelot has no ASCII
    // note name, so the fallback is sharps.
    settings.write_camelot_to_initial_key = false;
    let edit = retag_edit(&analysed(), &snapshot(), &settings);
    assert_eq!(field(&edit, TagField::InitialKey), Some(Some("Am".into())));

    // Flats, on a root that has one, uses the ASCII `b` rather than the
    // typographic flat the UI shows: TKEY allows three characters from A-G, b,
    // # and m, and nothing else.
    let mut track = analysed();
    track.key_root = Some(6);
    settings.key_notation = KeyNotation::Flats;
    let edit = retag_edit(&track, &snapshot(), &settings);
    assert_eq!(field(&edit, TagField::InitialKey), Some(Some("Gbm".into())));
}

#[test]
fn re_tagging_an_already_tagged_library_does_not_stack_markers() {
    // The case that matters: RE-TAG EXISTING FILES, run twice, with no column
    // recording what was written the first time.
    let settings = Settings::default();
    let first = retag_edit(&analysed(), &snapshot(), &settings);
    let title = field(&first, TagField::Title).unwrap().unwrap();

    let mut after = snapshot();
    after.title = Some(title.clone());
    let second = retag_edit(&analysed(), &after, &settings);
    assert_eq!(field(&second, TagField::Title), Some(Some(title)));
}

#[test]
fn a_changed_key_replaces_the_old_marker_instead_of_adding_to_it() {
    let settings = Settings::default();
    let mut after = snapshot();
    after.title = Some("8A - Energy 7 - Midnight".into());

    let mut track = analysed();
    track.key_root = Some(0); // C minor -> 5A
    track.energy = Some(9);
    let edit = retag_edit(&track, &after, &settings);
    assert_eq!(
        field(&edit, TagField::Title),
        Some(Some("5A - Energy 9 - Midnight".into()))
    );
}

#[test]
fn a_marker_at_the_end_of_the_title_is_recognised_from_the_end() {
    let settings = Settings {
        tag_write_placement: TagWritePlacement::EndOfTitle,
        ..Settings::default()
    };
    let mut after = snapshot();
    after.title = Some("Midnight - 8A - Energy 7".into());
    let edit = retag_edit(&analysed(), &after, &settings);
    assert_eq!(
        field(&edit, TagField::Title),
        Some(Some("Midnight - 8A - Energy 7".into()))
    );
}

#[test]
fn a_title_that_only_looks_like_a_marker_survives() {
    let settings = Settings {
        tag_write_content: TagWriteContent::KeyAndTempo,
        ..Settings::default()
    };
    let mut after = snapshot();
    // Not a key and not a tempo: a number outside the plausible range, and a
    // word that starts with a note letter.
    after.title = Some("Energy - 1999 - Midnight".into());
    let edit = retag_edit(&analysed(), &after, &settings);
    assert_eq!(
        field(&edit, TagField::Title),
        Some(Some("8A - 126 - Energy - 1999 - Midnight".into()))
    );
}

#[test]
fn a_bare_energy_is_only_a_marker_when_the_setting_writes_bare_energies() {
    let mut after = snapshot();
    after.title = Some("7 - Midnight".into());

    // KeyOnly never writes a bare energy, so the 7 is part of the title.
    let settings = Settings {
        tag_write_content: TagWriteContent::KeyOnly,
        ..Settings::default()
    };
    let edit = retag_edit(&analysed(), &after, &settings);
    assert_eq!(
        field(&edit, TagField::Title),
        Some(Some("8A - 7 - Midnight".into()))
    );

    // EnergyOnly does, so it is replaced rather than pushed along.
    let settings = Settings {
        tag_write_content: TagWriteContent::EnergyOnly,
        ..Settings::default()
    };
    let edit = retag_edit(&analysed(), &after, &settings);
    assert_eq!(field(&edit, TagField::Title), Some(Some("7 - Midnight".into())));
}

#[test]
fn the_grouping_energy_prefix_replaces_itself_rather_than_stacking() {
    let settings = Settings { write_energy_to_grouping: true, ..Settings::default() };
    let mut after = snapshot();
    after.grouping = Some("4 - Peak Time".into());

    let edit = retag_edit(&analysed(), &after, &settings);
    assert_eq!(
        field(&edit, TagField::Grouping),
        Some(Some("7 - Peak Time".into()))
    );
}

#[test]
fn removing_the_key_takes_the_marker_and_the_grouping_prefix_with_it() {
    let settings = Settings { write_energy_to_grouping: true, ..Settings::default() };
    let mut after = snapshot();
    after.title = Some("8A - Energy 7 - Midnight".into());
    after.grouping = Some("7 - Peak Time".into());

    let edit = remove_marker_edit(&after, &settings);
    assert_eq!(field(&edit, TagField::Title), Some(Some("Midnight".into())));
    assert_eq!(field(&edit, TagField::InitialKey), Some(None));
    assert_eq!(field(&edit, TagField::Grouping), Some(Some("Peak Time".into())));
}

#[test]
fn removing_a_marker_that_was_the_whole_field_clears_it() {
    let settings = Settings {
        tag_write_placement: TagWritePlacement::OverwriteComment,
        ..Settings::default()
    };
    let edit = remove_marker_edit(&snapshot(), &settings);
    assert_eq!(field(&edit, TagField::Comment), Some(None));
}
