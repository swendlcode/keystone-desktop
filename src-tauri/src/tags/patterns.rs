//! SPEC.md §3's "what to write / where to write it" composition, and its exact
//! inverse.
//!
//! The inverse has to be exact rather than a regex guess, because the context
//! menu offers Remove Key from Common Tags and a greedy pattern would eat a
//! track genuinely called `Energy`. What makes it exact is composing from the
//! stored values and removing that specific string.

use super::MARKER_SEPARATOR;
use crate::analysis::camelot::key_string;
use crate::error::{KeystoneError, Result};
use crate::models::{
    Settings, TagEdit, TagField, TagSnapshot, TagWriteContent, TagWritePlacement, Track,
};

/// The text the settings say to write, e.g. `10A - Energy 7`.
///
/// Returns `None` when the track has nothing to say — no key and no energy —
/// rather than writing an empty marker into the user's title. When one half of
/// a two-part form is missing the other half is written alone: a dangling
/// `10A - ` in someone's title is worse than a partial answer.
pub fn compose(track: &Track, settings: &Settings) -> Option<String> {
    let key = key_marker(track, settings);
    let energy = track.energy.map(|e| e.to_string());
    let tempo = tempo_marker(track);

    let joined = |a: Option<String>, b: Option<String>| match (a, b) {
        (Some(a), Some(b)) => Some(format!("{a}{MARKER_SEPARATOR}{b}")),
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    };

    match settings.tag_write_content {
        TagWriteContent::KeyOnly => key,
        TagWriteContent::EnergyOnly => energy,
        TagWriteContent::EnergyWithWord => energy.map(|e| format!("Energy {e}")),
        TagWriteContent::KeyAndEnergy => joined(key, energy),
        TagWriteContent::KeyAndEnergyWithWord => {
            joined(key, energy.map(|e| format!("Energy {e}")))
        }
        TagWriteContent::KeyAndTempo => joined(key, tempo),
    }
}

/// The key as the Key Notation setting asks for it, Camelot included — this is
/// the marker that goes into the artist, title or comment, which has no legal
/// value set. The dedicated key field is different and goes through
/// [`super::initial_key`], where `TKEY`'s three-character restriction applies.
fn key_marker(track: &Track, settings: &Settings) -> Option<String> {
    let root = track.key_root?;
    let mode = track.key_mode?;
    Some(key_string(
        root,
        mode,
        settings.key_notation,
        settings.pad_camelot_numbers,
    ))
}

/// The marker tempo is always an integer. The decimal display is a read-time
/// format over `tempo_display`, and `10A - 126.14` in a filename or a title is
/// noise.
fn tempo_marker(track: &Track) -> Option<String> {
    track
        .tempo_display
        .or(track.tempo_raw)
        .filter(|b| b.is_finite() && *b > 0.0)
        .map(|b| format!("{}", b.round() as i64))
}

/// Applies `text` at the configured position, producing the field writes.
///
/// `OverwriteComment` replaces; every other placement inserts. An insert must
/// first strip whatever Keystone wrote last time, or re-tagging a library twice
/// produces `10A - 10A - Title`.
pub fn apply_placement(
    snapshot: &TagSnapshot,
    text: &str,
    previous: Option<&str>,
    placement: TagWritePlacement,
) -> TagEdit {
    let field = match placement {
        TagWritePlacement::FrontOfArtist => TagField::Artist,
        TagWritePlacement::FrontOfTitle | TagWritePlacement::EndOfTitle => TagField::Title,
        TagWritePlacement::FrontOfComment | TagWritePlacement::OverwriteComment => {
            TagField::Comment
        }
    };

    let current = super::verify::snapshot_field(snapshot, field).unwrap_or_default();
    let base = previous.map_or_else(|| current.clone(), |p| strip_exact(&current, p));

    let value = match placement {
        TagWritePlacement::OverwriteComment => text.to_string(),
        TagWritePlacement::EndOfTitle => append(&base, text),
        _ => prepend(text, &base),
    };
    TagEdit::default().set(field, value)
}

fn prepend(marker: &str, rest: &str) -> String {
    if rest.trim().is_empty() {
        marker.to_string()
    } else {
        format!("{marker}{MARKER_SEPARATOR}{rest}")
    }
}

fn append(rest: &str, marker: &str) -> String {
    if rest.trim().is_empty() {
        marker.to_string()
    } else {
        format!("{rest}{MARKER_SEPARATOR}{marker}")
    }
}

/// Removes exactly `previous` from the field it was written into. Backs the
/// context menu's Remove Key from Common Tags.
///
/// Exactly one occurrence, with the separator Keystone put beside it, and
/// nothing else. A track called `Energy` survives, and a title that genuinely
/// contains ` - ` keeps every other one it has.
pub fn strip_exact(value: &str, previous: &str) -> String {
    if previous.is_empty() {
        return value.to_string();
    }
    let Some(at) = value.find(previous) else {
        return value.to_string();
    };

    let head = &value[..at];
    let tail = &value[at + previous.len()..];
    let joined = if let Some(rest) = tail.strip_prefix(MARKER_SEPARATOR) {
        format!("{head}{rest}")
    } else if let Some(start) = head.strip_suffix(MARKER_SEPARATOR) {
        format!("{start}{tail}")
    } else {
        format!("{head}{tail}")
    };
    joined.trim().to_string()
}

/// The `Remove Extra Spaces` tab: collapses runs of whitespace and trims, over
/// the editable text columns.
///
/// Only the fields that actually change are emitted, so the tab reports a count
/// the user can believe and files that are already clean are not rewritten.
pub fn remove_extra_spaces(snapshot: &TagSnapshot) -> TagEdit {
    const TEXT_FIELDS: [TagField; 6] = [
        TagField::Artist,
        TagField::Title,
        TagField::Album,
        TagField::Genre,
        TagField::Grouping,
        TagField::Comment,
    ];

    let mut edit = TagEdit::default();
    for field in TEXT_FIELDS {
        let Some(current) = super::verify::snapshot_field(snapshot, field) else {
            continue;
        };
        let tidied = collapse(&current);
        if tidied != current {
            edit.fields.push((field, Some(tidied).filter(|v| !v.is_empty())));
        }
    }
    edit
}

fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Global Replace across one of Artist, Title, Key, Genre, Album, Grouping,
/// Comment. Case-sensitive, plain substring: a DJ cleaning `feat.` to `ft.`
/// does not want a regex.
/// One edit covering every field the user ticked, or `None` when the needle is
/// in none of them.
///
/// All the fields go into a single `TagEdit` rather than one edit per field,
/// because the write path opens, rewrites and verifies the file once per edit:
/// seven ticked columns would otherwise be seven rewrites of the same file,
/// seven backups, and seven chances for one of them to fail halfway.
pub fn global_replace(
    snapshot: &TagSnapshot,
    fields: &[TagField],
    find: &str,
    replace: &str,
) -> Result<Option<TagEdit>> {
    if find.is_empty() {
        // An empty needle matches between every character, so "replace all"
        // would interleave the replacement through the whole value.
        return Err(KeystoneError::Other(
            "Enter the text to replace.".to_string(),
        ));
    }
    let mut edit = TagEdit::default();
    let mut touched = false;
    for field in fields {
        let Some(current) = super::verify::snapshot_field(snapshot, *field) else {
            continue;
        };
        if !current.contains(find) {
            continue;
        }
        edit = edit.set(*field, current.replace(find, replace));
        touched = true;
    }
    Ok(touched.then_some(edit))
}

/// The energy value written in front of Grouping, when that option is on.
///
/// The bare number, not `Energy 7`: Grouping is a column DJs sort and filter
/// on in Serato and rekordbox, and `7` sorts.
pub fn grouping_prefix(energy: u8) -> String {
    energy.to_string()
}

/// What goes in the Tempo tag. Honours `keep_existing_tempo`, and note that
/// ID3v2 has no decimal BPM frame — only an integer maps to `TBPM` and the MP4
/// `tmpo` atom, so the spec's decimal display always comes from our own
/// database and never from a tag round trip.
pub fn tempo_tag_value(track: &Track, settings: &Settings) -> Option<String> {
    if !settings.update_tempo_tag {
        return None;
    }
    if settings.keep_existing_tempo && track.tag_bpm.is_some_and(|b| b > 0.0) {
        return None;
    }
    tempo_marker(track)
}

#[cfg(test)]
#[path = "patterns_tests.rs"]
mod tests;
