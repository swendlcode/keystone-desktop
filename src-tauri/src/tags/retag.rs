//! The whole Update Tags surface, composed.
//!
//! SPEC.md §3 is six "what to write" options times five "where to write it"
//! options, and three independent extras beside them: the custom Initial Key
//! tag, the energy in front of the Grouping, and the Tempo tag with its keep
//! existing values sub-option. [`retag_edit`] is where they meet, so the
//! commands stay thin and the composition is testable without a file.
//!
//! The hard part is not writing. It is writing **twice**: RE-TAG EXISTING FILES
//! over a library that has already been re-tagged must not leave
//! `8A - Energy 7 - 8A - Energy 7 - Midnight`. There is no column recording
//! what Keystone wrote last time, so the previous marker is recognised instead
//! — anchored at the position the setting writes to, and only accepting
//! segments that parse as the kind of marker that setting produces. A key, an
//! `Energy 7`, a bare energy when the setting writes bare energies, a tempo
//! when the setting writes tempos. Nothing else, and never more segments than
//! the setting composes.

use super::patterns::{compose, grouping_prefix, tempo_tag_value};
use super::{initial_key, verify, MARKER_SEPARATOR};
use crate::analysis::camelot::Camelot;
use crate::models::{
    Settings, TagEdit, TagField, TagSnapshot, TagWriteContent, TagWritePlacement, Track,
};

/// Energy is 1-10, so a bare marker segment outside that is somebody's title.
const ENERGY_RANGE: std::ops::RangeInclusive<u8> = 1..=10;
/// The tempo range a marker can plausibly carry. Narrow on purpose: it is what
/// keeps a track called `Midnight - 1999` intact.
const TEMPO_RANGE: std::ops::RangeInclusive<u32> = 50..=400;

/// Everything the Update Tags settings ask for, for one track.
///
/// Returns an empty edit when the master switch is off, or when the track has
/// nothing to say yet — never a half-written marker.
pub fn retag_edit(track: &Track, snapshot: &TagSnapshot, settings: &Settings) -> TagEdit {
    let mut edit = TagEdit::default();
    if !settings.update_tags_enabled {
        return edit;
    }

    if let Some(text) = compose(track, settings) {
        let previous = previous_marker(snapshot, settings);
        let placed = super::patterns::apply_placement(
            snapshot,
            &text,
            previous.as_deref(),
            settings.tag_write_placement,
        );
        edit.fields.extend(placed.fields);
    }

    if settings.update_initial_key_tag {
        if let Some((root, mode)) = track.key_root.zip(track.key_mode) {
            edit = edit.set(
                TagField::InitialKey,
                initial_key::tag_value(
                    root,
                    mode,
                    settings.key_notation,
                    settings.write_camelot_to_initial_key,
                    settings.pad_camelot_numbers,
                ),
            );
        }
    }

    if settings.write_energy_to_grouping {
        if let Some(energy) = track.energy {
            edit = edit.set(TagField::Grouping, grouping(snapshot, energy));
        }
    }

    if let Some(bpm) = tempo_tag_value(track, settings) {
        edit = edit.set(TagField::Bpm, bpm);
    }
    edit
}

/// Remove Key from Common Tags: takes the marker back out of wherever the
/// setting put it, and clears the dedicated key field.
///
/// The Grouping energy prefix goes too — it is part of the same stamp, and a
/// user removing the key does not want the 7 left behind.
pub fn remove_marker_edit(snapshot: &TagSnapshot, settings: &Settings) -> TagEdit {
    let mut edit = TagEdit::default().clear(TagField::InitialKey);

    let field = placement_field(settings.tag_write_placement);
    if let Some(previous) = previous_marker(snapshot, settings) {
        let current = verify::snapshot_field(snapshot, field).unwrap_or_default();
        let stripped = super::patterns::strip_exact(&current, &previous);
        edit = put(edit, field, stripped);
    } else if settings.tag_write_placement == TagWritePlacement::OverwriteComment {
        // Overwrite leaves nothing of the user's text to put back, so the
        // honest inverse is an empty comment.
        edit = edit.clear(field);
    }

    if let Some(grouping) = snapshot.grouping.as_deref() {
        let stripped = strip_leading_energy(grouping);
        if stripped != grouping {
            edit = put(edit, TagField::Grouping, stripped.to_string());
        }
    }
    edit
}

/// The marker already sitting at the configured position, if there is one.
///
/// `budget` leaves at least one segment behind, so a title that is *entirely*
/// marker-shaped is never emptied.
fn previous_marker(snapshot: &TagSnapshot, settings: &Settings) -> Option<String> {
    let placement = settings.tag_write_placement;
    if placement == TagWritePlacement::OverwriteComment {
        return None;
    }
    let current = verify::snapshot_field(snapshot, placement_field(placement))?;
    let segments: Vec<&str> = current.split(MARKER_SEPARATOR).collect();
    let content = settings.tag_write_content;
    let budget = segment_count(content).min(segments.len().saturating_sub(1));

    let taken: &[&str] = if placement == TagWritePlacement::EndOfTitle {
        let window = &segments[segments.len() - budget..];
        let mut start = window.len();
        while start > 0 && is_marker(window[start - 1], content) {
            start -= 1;
        }
        &window[start..]
    } else {
        let window = &segments[..budget];
        let mut end = 0;
        while end < window.len() && is_marker(window[end], content) {
            end += 1;
        }
        &window[..end]
    };

    (!taken.is_empty()).then(|| taken.join(MARKER_SEPARATOR))
}

/// An empty value means "take the field out", not "write an empty string": an
/// empty frame reads as present in some players and as absent in others.
fn put(edit: TagEdit, field: TagField, value: String) -> TagEdit {
    if value.trim().is_empty() {
        edit.clear(field)
    } else {
        edit.set(field, value)
    }
}

fn placement_field(placement: TagWritePlacement) -> TagField {
    match placement {
        TagWritePlacement::FrontOfArtist => TagField::Artist,
        TagWritePlacement::FrontOfTitle | TagWritePlacement::EndOfTitle => TagField::Title,
        TagWritePlacement::FrontOfComment | TagWritePlacement::OverwriteComment => {
            TagField::Comment
        }
    }
}

/// How many `" - "`-separated segments this content setting composes.
fn segment_count(content: TagWriteContent) -> usize {
    match content {
        TagWriteContent::KeyOnly
        | TagWriteContent::EnergyOnly
        | TagWriteContent::EnergyWithWord => 1,
        TagWriteContent::KeyAndEnergy
        | TagWriteContent::KeyAndEnergyWithWord
        | TagWriteContent::KeyAndTempo => 2,
    }
}

/// Setting-aware on purpose. A bare `7` is only treated as a marker when the
/// setting writes bare energies, and a bare `126` only when it writes tempos —
/// otherwise `Out - 8` and `Midnight - 126` would be eaten.
fn is_marker(segment: &str, content: TagWriteContent) -> bool {
    let s = segment.trim();
    if Camelot::parse(s).is_some() || is_key_name(s) {
        return true;
    }
    if let Some(n) = s.strip_prefix("Energy ") {
        return n.trim().parse::<u8>().is_ok_and(|e| ENERGY_RANGE.contains(&e));
    }
    match content {
        TagWriteContent::EnergyOnly | TagWriteContent::KeyAndEnergy => {
            s.parse::<u8>().is_ok_and(|e| ENERGY_RANGE.contains(&e))
        }
        TagWriteContent::KeyAndTempo => {
            s.parse::<u32>().is_ok_and(|t| TEMPO_RANGE.contains(&t))
        }
        _ => false,
    }
}

/// `Am`, `F#m`, `G♭m`, `C`. The notations the Key Notation setting can produce,
/// and nothing looser: a two-letter word is not a key.
fn is_key_name(s: &str) -> bool {
    let mut chars = s.chars();
    if !chars.next().is_some_and(|c| ('A'..='G').contains(&c)) {
        return false;
    }
    let rest: String = chars.collect();
    let rest = rest
        .strip_prefix('#')
        .or_else(|| rest.strip_prefix('b'))
        .or_else(|| rest.strip_prefix('♯'))
        .or_else(|| rest.strip_prefix('♭'))
        .unwrap_or(&rest);
    rest.is_empty() || rest == "m"
}

fn grouping(snapshot: &TagSnapshot, energy: u8) -> String {
    let prefix = grouping_prefix(energy);
    let base = snapshot
        .grouping
        .as_deref()
        .map(strip_leading_energy)
        .unwrap_or_default();
    if base.trim().is_empty() {
        prefix
    } else {
        format!("{prefix}{MARKER_SEPARATOR}{base}")
    }
}

fn strip_leading_energy(grouping: &str) -> &str {
    let (first, rest) = match grouping.split_once(MARKER_SEPARATOR) {
        Some((first, rest)) => (first, rest),
        None => (grouping, ""),
    };
    if first
        .trim()
        .parse::<u8>()
        .is_ok_and(|e| ENERGY_RANGE.contains(&e))
    {
        rest
    } else {
        grouping
    }
}

#[cfg(test)]
#[path = "retag_tests.rs"]
mod tests;
