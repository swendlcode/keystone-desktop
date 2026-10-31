//! Which field in which container, and how to set it without going near
//! lofty's generic `Tag`.
//!
//! Five fields — artist, title, album, genre, comment — go through lofty's
//! `Accessor`, which knows the right frame, atom or comment key per container.
//! The four that matter most to a DJ do not: grouping, BPM, the key field and
//! the year are addressed by their container-specific names here, because
//! `Accessor` has no method for them and guessing is how a key ends up written
//! somewhere Serato does not look.

use lofty::ape::ApeItem;
use lofty::id3::v2::{Frame, FrameId, Id3v2Tag, TextInformationFrame};
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
use lofty::ogg::tag::VorbisComments;
use lofty::ape::ApeTag;
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor, ItemValue};
use lofty::TextEncoding;

use super::concrete::{Primary, Side};
use crate::error::{KeystoneError, Result};
use crate::models::TagField;

/// The same field, named per container.
struct FieldSpec {
    id3v2: &'static str,
    /// `None` for fields with no four-character code, which then use
    /// [`FieldSpec::ilst_freeform`].
    ilst_fourcc: Option<[u8; 4]>,
    ilst_freeform: Option<(&'static str, &'static str)>,
    /// Several keys means write **all** of them. FLAC and Ogg have no single
    /// convention for the key field, so `INITIALKEY` and `KEY` both go in.
    vorbis: &'static [&'static str],
    ape: &'static str,
}

/// `TIT1` is ID3v2's own "content group description". Apple writes `GRP1`
/// instead, so [`set`] keeps that one in step when the file already has it
/// rather than leaving iTunes and Serato reading different groupings.
const GROUPING: FieldSpec = FieldSpec {
    id3v2: "TIT1",
    ilst_fourcc: Some(*b"\xa9grp"),
    ilst_freeform: None,
    vorbis: &["GROUPING"],
    ape: "Grouping",
};

/// `ItemKey::Bpm` (decimal) has no ID3v2 mapping — only the integer `TBPM` and
/// the MP4 `tmpo` atom exist — so the spec's decimal tempo display always comes
/// from Keystone's own database and never from a tag round trip.
const BPM: FieldSpec = FieldSpec {
    id3v2: "TBPM",
    ilst_fourcc: Some(*b"tmpo"),
    ilst_freeform: None,
    vorbis: &["BPM"],
    ape: "BPM",
};

const INITIAL_KEY: FieldSpec = FieldSpec {
    id3v2: "TKEY",
    ilst_fourcc: None,
    ilst_freeform: Some(("com.apple.iTunes", "initialkey")),
    vorbis: &["INITIALKEY", "KEY"],
    ape: "Key",
};

const YEAR: FieldSpec = FieldSpec {
    id3v2: "TDRC",
    ilst_fourcc: Some(*b"\xa9day"),
    ilst_freeform: None,
    vorbis: &["DATE"],
    ape: "Year",
};

/// The `GRP1` frame iTunes uses for Grouping, kept in step with `TIT1`.
const APPLE_GROUPING: &str = "GRP1";

/// A Serato report has AAC files showing the key only in Comments, so M4A gets
/// both the freeform atom and a comment. There is no single field that works.
const ILST_COMMENT: [u8; 4] = *b"\xa9cmt";

pub fn is_common(field: TagField) -> bool {
    matches!(
        field,
        TagField::Artist
            | TagField::Title
            | TagField::Album
            | TagField::Genre
            | TagField::Comment
    )
}

pub fn set(primary: &mut Primary, field: TagField, value: Option<&str>) -> Result<()> {
    if is_common(field) {
        match primary {
            Primary::Id3v2(t) => accessor_set(&mut **t, field, value),
            Primary::Ilst(t) => accessor_set(&mut **t, field, value),
            Primary::Vorbis(t) => accessor_set(&mut **t, field, value),
            Primary::Ape(t) => accessor_set(&mut **t, field, value),
        }
        return Ok(());
    }

    match field {
        TagField::Grouping => {
            set_spec(primary, &GROUPING, value)?;
            // Only if it is already there: Keystone does not add Apple's
            // duplicate frame to files that do not have one.
            if let Primary::Id3v2(t) = primary {
                if t.get_text(&frame_id(APPLE_GROUPING)?).is_some() {
                    id3v2_set(t, APPLE_GROUPING, value)?;
                }
            }
            Ok(())
        }
        TagField::Bpm => set_spec(primary, &BPM, value),
        TagField::Year => set_spec(primary, &YEAR, value),
        TagField::InitialKey => {
            set_spec(primary, &INITIAL_KEY, value)?;
            if let Primary::Ilst(t) = primary {
                ilst_set(t, AtomIdent::Fourcc(ILST_COMMENT), value);
            }
            Ok(())
        }
        _ => unreachable!("every common field returned above"),
    }
}

/// Keeps a secondary block's copy of a common field from disagreeing with the
/// primary. Only the five fields every container can hold: an ID3v1 tag has
/// nowhere to put a key, and the AIFF text chunks have no album at all.
pub fn mirror_common(side: &mut Side, field: TagField, value: Option<&str>) {
    if !is_common(field) {
        return;
    }
    match side {
        Side::Id3v1(t) => accessor_set(&mut **t, field, value),
        Side::Riff(t) => accessor_set(&mut **t, field, value),
        Side::AiffText(t) => accessor_set(&mut **t, field, value),
    }
}

fn accessor_set<T: Accessor>(tag: &mut T, field: TagField, value: Option<&str>) {
    match (field, value) {
        (TagField::Artist, Some(v)) => tag.set_artist(v.to_string()),
        (TagField::Artist, None) => tag.remove_artist(),
        (TagField::Title, Some(v)) => tag.set_title(v.to_string()),
        (TagField::Title, None) => tag.remove_title(),
        (TagField::Album, Some(v)) => tag.set_album(v.to_string()),
        (TagField::Album, None) => tag.remove_album(),
        (TagField::Genre, Some(v)) => tag.set_genre(v.to_string()),
        (TagField::Genre, None) => tag.remove_genre(),
        (TagField::Comment, Some(v)) => tag.set_comment(v.to_string()),
        (TagField::Comment, None) => tag.remove_comment(),
        _ => {}
    }
}

fn set_spec(primary: &mut Primary, spec: &FieldSpec, value: Option<&str>) -> Result<()> {
    match primary {
        Primary::Id3v2(t) => id3v2_set_spec(t, spec, value),
        Primary::Ilst(t) => {
            ilst_set_spec(t, spec, value);
            Ok(())
        }
        Primary::Vorbis(t) => {
            vorbis_set(t, spec.vorbis, value);
            Ok(())
        }
        Primary::Ape(t) => ape_set(t, spec.ape, value),
    }
}

/// The year goes through `Accessor::set_date` rather than a raw `TDRC` frame:
/// lofty parses `TDRC` as a timestamp, and handing it a plain text frame with
/// the same id would read back as something else.
fn id3v2_set_spec(tag: &mut Id3v2Tag, spec: &FieldSpec, value: Option<&str>) -> Result<()> {
    if spec.id3v2 == YEAR.id3v2 {
        match value.and_then(|v| v.trim().parse::<u16>().ok()) {
            Some(year) => tag.set_date(Timestamp {
                year,
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
            }),
            None => tag.remove_date(),
        }
        return Ok(());
    }
    id3v2_set(tag, spec.id3v2, value)
}

fn id3v2_set(tag: &mut Id3v2Tag, id: &str, value: Option<&str>) -> Result<()> {
    let id = frame_id(id)?;
    match value {
        Some(v) => {
            tag.insert(Frame::Text(TextInformationFrame::new(
                id,
                // lofty downgrades this to a v2.3-legal encoding itself when
                // the tag is written back as v2.3.
                TextEncoding::UTF8,
                v.to_string(),
            )));
        }
        None => tag.remove(&id).for_each(drop),
    }
    Ok(())
}

fn frame_id(id: &str) -> Result<FrameId<'static>> {
    FrameId::new(id.to_string())
        .map_err(|e| KeystoneError::Tag(format!("{id} is not a legal ID3v2 frame id: {e}")))
}

fn ilst_set_spec(tag: &mut Ilst, spec: &FieldSpec, value: Option<&str>) {
    let ident = match (spec.ilst_fourcc, spec.ilst_freeform) {
        (Some(code), _) => AtomIdent::Fourcc(code),
        (None, Some((mean, name))) => AtomIdent::Freeform {
            mean: mean.into(),
            name: name.into(),
        },
        (None, None) => return,
    };

    // `tmpo` is an integer atom, not text. Writing the digits as a string there
    // leaves iTunes and Serato showing no BPM at all.
    if spec.ilst_fourcc == BPM.ilst_fourcc {
        let parsed = value.and_then(|v| v.trim().parse::<u32>().ok());
        match parsed {
            Some(n) => tag.replace_atom(Atom::new(ident, AtomData::UnsignedInteger(n))),
            None => tag.remove(&ident).for_each(drop),
        }
        return;
    }
    ilst_set(tag, ident, value);
}

fn ilst_set(tag: &mut Ilst, ident: AtomIdent<'static>, value: Option<&str>) {
    match value {
        Some(v) => tag.replace_atom(Atom::new(ident, AtomData::UTF8(v.to_string()))),
        None => tag.remove(&ident).for_each(drop),
    }
}

fn vorbis_set(tag: &mut VorbisComments, keys: &[&str], value: Option<&str>) {
    for key in keys {
        match value {
            // `insert` replaces every existing value for the key, which is what
            // a field edit means; `push` would leave the old one behind.
            Some(v) => tag.insert((*key).to_string(), v.to_string()),
            None => tag.remove(key).for_each(drop),
        }
    }
}

fn ape_set(tag: &mut ApeTag, key: &str, value: Option<&str>) -> Result<()> {
    match value {
        Some(v) => {
            let item = ApeItem::new(key.to_string(), ItemValue::Text(v.to_string()))
                .map_err(|e| KeystoneError::Tag(format!("{key}: {e}")))?;
            tag.insert(item);
        }
        None => tag.remove(key),
    }
    Ok(())
}
