//! One `ENTRY`: matching it to a track, and splicing the children we model.
//!
//! The entry is buffered before anything is decided, because `LOCATION` — the
//! only thing that says which track this is — arrives after the `ENTRY` start
//! tag. Buffering means an unmatched entry is emitted as the exact bytes it
//! arrived as, which is the whole safety property of this target.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::XmlVersion;

use super::{attr, cue_type, unresolved};
use super::{EntryEdit, LocationKey, RewriteStats, TraktorCue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Start,
    Empty,
    End,
    Other,
}

pub(super) struct Item {
    pub raw: String,
    pub name: Option<String>,
    pub kind: Kind,
}

pub(super) struct EntryBuf {
    pub items: Vec<Item>,
    pub key: Option<LocationKey>,
}

/// Tracks whose own volume could not be determined. They may only match on
/// `(DIR, FILE)`, and only when that pair is unique across the whole run: a
/// guess here is a cue on the wrong song.
pub(super) fn volumeless_index(edits: &HashMap<LocationKey, EntryEdit>) -> HashMap<(String, String), usize> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for key in edits.keys() {
        *counts.entry(key.dir_file()).or_insert(0) += 1;
    }
    counts
}

fn lookup<'a>(
    key: &LocationKey,
    edits: &'a HashMap<LocationKey, EntryEdit>,
    volumeless: &HashMap<(String, String), usize>,
) -> Option<(&'a LocationKey, &'a EntryEdit)> {
    if let Some((k, e)) = edits.get_key_value(key) {
        return Some((k, e));
    }
    // The entry's own volume may be a name we cannot reproduce (an external
    // disk that is not mounted, a renamed startup disk). Fall back only when
    // exactly one of our tracks has this directory and filename, and only when
    // that track had no volume of its own to compare.
    let dir_file = key.dir_file();
    if volumeless.get(&dir_file).copied().unwrap_or(0) != 1 {
        return None;
    }
    edits
        .iter()
        .find(|(k, _)| k.dir_file() == dir_file && !k.has_volume())
}

/// Emits one `ENTRY`, either spliced or byte-for-byte as it arrived.
pub(super) fn finish_entry(
    buf: EntryBuf,
    end_tag: &str,
    edits: &HashMap<LocationKey, EntryEdit>,
    volumeless: &HashMap<(String, String), usize>,
    overwrite: bool,
    stats: &mut RewriteStats,
) -> String {
    let matched = buf
        .key
        .as_ref()
        .and_then(|k| lookup(k, edits, volumeless));
    let Some((key, edit)) = matched else {
        return verbatim(&buf, end_tag);
    };
    stats.entries_matched += 1;
    stats.matched_keys.push(key.clone());

    let existing_pads = buf
        .items
        .iter()
        .filter(|i| i.name.as_deref() == Some("CUE_V2") && i.kind != Kind::End)
        .filter(|i| !is_grid_marker(&i.raw))
        .count();
    stats.cues_existing += existing_pads;

    // With overwrite off, a track that already carries hot cues is left exactly
    // as it is. Half-applying — our grid over their cues — would be worse than
    // either outcome.
    if existing_pads > 0 && !overwrite {
        stats.entries_skipped += 1;
        return verbatim(&buf, end_tag);
    }

    let writes_grid = edit.cues.iter().any(|c| c.cue_type == cue_type::GRID);
    let mut out = String::with_capacity(end_tag.len() + 512);
    let mut wrote_key = false;
    for item in &buf.items {
        out.push_str(&rewrite_child(item, edit, writes_grid, &mut wrote_key, stats));
    }
    let indent = trailing_indent(&buf);
    out.push_str(&splice(edit, &indent, &mut wrote_key, stats));
    out.push_str(end_tag);
    stats.entries_written += 1;
    out
}

/// One child of a matched entry. Dropped children return an empty string, which
/// is how an existing `CUE_V2` disappears without its surrounding bytes moving.
fn rewrite_child(
    item: &Item,
    edit: &EntryEdit,
    writes_grid: bool,
    wrote_key: &mut bool,
    stats: &mut RewriteStats,
) -> String {
    match item.name.as_deref() {
        Some("CUE_V2") => {
            let grid = is_grid_marker(&item.raw);
            if grid && !writes_grid {
                return item.raw.clone();
            }
            if item.kind != Kind::End {
                stats.cues_replaced += 1;
            }
            String::new()
        }
        Some("MUSICAL_KEY") if item.kind != Kind::End => match edit.musical_key {
            Some(value) => {
                *wrote_key = true;
                attr::set(&item.raw, "VALUE", &value.to_string())
            }
            None => item.raw.clone(),
        },
        Some("INFO") if item.kind != Kind::End => match edit.key_text.as_deref() {
            Some(key) => attr::set(&item.raw, "KEY", key),
            None => item.raw.clone(),
        },
        Some("TEMPO") if item.kind != Kind::End => match edit.bpm {
            Some(bpm) => attr::set(&item.raw, "BPM", &format!("{bpm:.6}")),
            None => item.raw.clone(),
        },
        _ => item.raw.clone(),
    }
}

/// The elements we add: the key, when the entry had no `MUSICAL_KEY` to edit,
/// then the grid marker and the hot cues.
fn splice(
    edit: &EntryEdit,
    indent: &str,
    wrote_key: &mut bool,
    stats: &mut RewriteStats,
) -> String {
    let mut out = String::new();
    if let Some(value) = edit.musical_key {
        if !*wrote_key {
            out.push_str(indent);
            out.push_str(&format!("<MUSICAL_KEY VALUE=\"{value}\"></MUSICAL_KEY>"));
            *wrote_key = true;
        }
    }
    for cue in &edit.cues {
        out.push_str(indent);
        out.push_str(&cue_element(cue));
        stats.cues_written += 1;
    }
    out
}

/// A loop is a cue with `LEN > 0`. The grid is `TYPE="4"`, named `AutoGrid`,
/// and the BPM it belongs to lives in the separate `TEMPO` element.
pub(super) fn cue_element(cue: &TraktorCue) -> String {
    let hotcue = match cue.slot {
        Some(slot) => unresolved::hotcue_attr(slot),
        None => unresolved::AUTOGRID_HOTCUE,
    };
    format!(
        "<CUE_V2 NAME=\"{name}\" DISPL_ORDER=\"0\" TYPE=\"{kind}\" START=\"{start:.6}\" \
         LEN=\"{len:.6}\" REPEATS=\"-1\" HOTCUE=\"{hotcue}\"></CUE_V2>",
        name = attr::escape(&cue.name),
        kind = cue.cue_type,
        start = unresolved::start_attr(cue.position_ms),
        len = unresolved::start_attr(cue.len_ms),
    )
}

fn is_grid_marker(raw: &str) -> bool {
    attr::get(raw, "TYPE").as_deref() == Some(&cue_type::GRID.to_string())
        || attr::get(raw, "NAME").as_deref() == Some(super::AUTOGRID_NAME)
}

fn verbatim(buf: &EntryBuf, end_tag: &str) -> String {
    let mut out = String::new();
    for item in &buf.items {
        out.push_str(&item.raw);
    }
    out.push_str(end_tag);
    out
}

/// The whitespace in front of `</ENTRY>`, reused so inserted children line up
/// with the ones already there. Cosmetic, but a collection a user can still
/// read is a collection they can still repair by hand.
fn trailing_indent(buf: &EntryBuf) -> String {
    buf.items
        .last()
        .filter(|i| i.kind == Kind::Other)
        .map(|i| i.raw.as_str())
        .filter(|raw| raw.starts_with('\n') && raw.trim().is_empty())
        .map(|raw| format!("{raw}  "))
        .unwrap_or_default()
}

pub(super) fn location_of(e: &BytesStart<'_>) -> LocationKey {
    let (mut dir, mut file, mut volume) = (String::new(), String::new(), String::new());
    for a in e.attributes().flatten() {
        // Normalised, not raw: the path we compare against came from the
        // filesystem, so `&amp;` in the file has to become `&` before it can
        // match a track whose name contains an ampersand.
        let value = a
            .normalized_value(XmlVersion::Explicit1_0)
            .map(|v| v.into_owned())
            .unwrap_or_else(|_| a.value.to_string());
        match a.key.as_ref() {
            "DIR" => dir = value,
            "FILE" => file = value,
            "VOLUME" => volume = value,
            _ => {}
        }
    }
    LocationKey::from_attrs(&dir, &file, &volume)
}

pub(super) fn classify(event: &Event<'_>) -> (Option<String>, Kind) {
    match event {
        Event::Start(e) => (Some(e.name().as_ref().to_string()), Kind::Start),
        Event::Empty(e) => (Some(e.name().as_ref().to_string()), Kind::Empty),
        Event::End(e) => (Some(e.name().as_ref().to_string()), Kind::End),
        _ => (None, Kind::Other),
    }
}

