//! The event-stream rewrite.
//!
//! Parsing NML is trivial; **preserving** it is not. Entries carry `AUDIO_ID`,
//! `LOUDNESS`, `COVERARTID`, `STEMS`, `FLAGS` and `MODIFICATION_INFO` that
//! Keystone has no model for, and a parse-into-structs-and-reserialise pass
//! silently destroys them.
//!
//! So nothing here is reserialised. Every event is copied as the exact source
//! bytes it came from — `buffer_position` gives the span and the spans tile the
//! document — and only the elements this module models are replaced. Even
//! `INFO` and `TEMPO`, which we do change, are edited one attribute at a time
//! inside their original tag text, because reserialising them would reorder and
//! requote attributes we never read.

use std::collections::HashMap;
use std::ops::Range;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::entry::{self, EntryBuf, Item, Kind};
use super::{attr, EntryEdit, LocationKey, RewriteStats};
use crate::error::{KeystoneError, Result};

/// The elements this module may change, so the only ones whose bytes are
/// allowed to differ afterwards. Everything else is byte-copied, and
/// `safety::assert_preserved` is given this list to prove it.
///
/// `INFO` and `TEMPO` are here because one attribute inside each is rewritten
/// (`@KEY` and `@BPM`) even though the rest of the tag is left alone, and
/// `COLLECTION` because `@ENTRIES` is kept in sync. Keeping the list honest
/// matters more than keeping it short: an element missing from it would make
/// the assertion pass over a real loss.
pub const MODELLED: &[&str] = &["CUE_V2", "MUSICAL_KEY", "INFO", "TEMPO", "COLLECTION"];

/// Rewrites the document in memory. Nothing is written here: the caller decides
/// whether to persist, which is what makes the plan and the write one code path.
pub fn rewrite_document(
    text: &str,
    edits: &HashMap<LocationKey, EntryEdit>,
    overwrite: bool,
) -> Result<(String, RewriteStats)> {
    let mut scan = Scan {
        out: String::with_capacity(text.len() + 4_096),
        stats: RewriteStats::default(),
        entry: None,
        collection: None,
        saw_nml: false,
        volumeless: entry::volumeless_index(edits),
    };
    let mut reader = Reader::from_str(text);
    let mut pos = 0usize;
    loop {
        let event = reader.read_event().map_err(|e| malformed(e, pos))?;
        let end = reader.buffer_position() as usize;
        let raw = text[pos..end].to_string();
        pos = end;
        if matches!(event, Event::Eof) {
            break;
        }
        scan.event(&event, raw, edits, overwrite);
    }
    scan.finish()
}

struct Scan {
    out: String,
    stats: RewriteStats,
    entry: Option<EntryBuf>,
    collection: Option<Range<usize>>,
    saw_nml: bool,
    volumeless: HashMap<(String, String), usize>,
}

impl Scan {
    fn event(
        &mut self,
        event: &Event<'_>,
        raw: String,
        edits: &HashMap<LocationKey, EntryEdit>,
        overwrite: bool,
    ) {
        let (name, kind) = entry::classify(event);
        if self.entry.is_some() {
            self.inside_entry(event, raw, name, kind, edits, overwrite);
            return;
        }
        match (name.as_deref(), kind) {
            (Some("NML"), _) => self.saw_nml = true,
            (Some("ENTRY"), Kind::Start) => {
                self.stats.entries_seen += 1;
                self.entry = Some(EntryBuf {
                    items: vec![Item { raw, name, kind }],
                    key: None,
                });
                return;
            }
            // An ENTRY with no children cannot be matched to a track, but it
            // still counts towards COLLECTION/@ENTRIES.
            (Some("ENTRY"), Kind::Empty) => self.stats.entries_seen += 1,
            (Some("COLLECTION"), Kind::Start) => {
                self.collection = Some(self.out.len()..self.out.len() + raw.len());
            }
            _ => {}
        }
        self.stats.bytes_passed_through += raw.len();
        self.out.push_str(&raw);
    }

    fn inside_entry(
        &mut self,
        event: &Event<'_>,
        raw: String,
        name: Option<String>,
        kind: Kind,
        edits: &HashMap<LocationKey, EntryEdit>,
        overwrite: bool,
    ) {
        let buf = self.entry.as_mut().expect("checked by the caller");
        if name.as_deref() == Some("LOCATION") && kind != Kind::End {
            if let Event::Start(e) | Event::Empty(e) = event {
                buf.key = Some(entry::location_of(e));
            }
        }
        if name.as_deref() == Some("ENTRY") && kind == Kind::End {
            let buf = self.entry.take().expect("checked by the caller");
            let spliced = entry::finish_entry(
                buf,
                &raw,
                edits,
                &self.volumeless,
                overwrite,
                &mut self.stats,
            );
            self.out.push_str(&spliced);
            return;
        }
        buf.items.push(Item { raw, name, kind });
    }

    /// The shape checks happen here rather than up front, because "is this a
    /// Traktor collection" is only answerable once the document has been read.
    fn finish(mut self) -> Result<(String, RewriteStats)> {
        if self.entry.is_some() {
            return Err(KeystoneError::Export(
                "the collection file ends inside an ENTRY: it is truncated, and \
                 Keystone will not write over a file it could not read."
                    .into(),
            ));
        }
        if !self.saw_nml {
            return Err(KeystoneError::Export(
                "this is not a Traktor collection: the file has no NML root element.".into(),
            ));
        }
        let collection = self.collection.clone().ok_or_else(|| {
            KeystoneError::Export(
                "this Traktor collection has no COLLECTION element, so there is \
                 nothing to write into."
                    .into(),
            )
        })?;
        sync_entries_attribute(&mut self.out, collection, self.stats.entries_seen);
        Ok((self.out, self.stats))
    }
}

/// `COLLECTION/@ENTRIES` must agree with the child count. We never add or remove
/// an entry, so this normally writes the same number back — but a collection
/// that arrived inconsistent is corrected rather than carried forward, because
/// Traktor trusts the attribute over the children.
fn sync_entries_attribute(out: &mut String, tag: Range<usize>, entries: usize) {
    let original = out[tag.clone()].to_string();
    let updated = attr::set(&original, "ENTRIES", &entries.to_string());
    if updated != original {
        out.replace_range(tag, &updated);
    }
}

/// Reported, never guessed at. The byte offset is included because a user whose
/// collection will not parse needs something to give support.
fn malformed(e: quick_xml::Error, pos: usize) -> KeystoneError {
    KeystoneError::Export(format!(
        "the Traktor collection is not valid XML (byte {pos}): {e}. Nothing was written."
    ))
}
