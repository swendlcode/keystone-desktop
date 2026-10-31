//! The per-container `Initial Key` field map, and the TKEY legality rule.

use crate::error::Result;
use crate::models::{Container, KeyMode, KeyNotation};

/// Which field(s) carry the key, per container. Several containers need **both**
/// written because there is no single convention:
///
/// - MP3, AIFF, WAV: `TKEY` (read by Serato's Key column, Traktor's Key Text,
///   and rekordbox).
/// - MP4, M4A: `----:com.apple.iTunes:initialkey`, **plus a Comment fallback** —
///   a Serato report has AAC showing the key only in Comments.
/// - FLAC, Ogg: `INITIALKEY` **and** `KEY`.
pub fn fields_for(container: Container) -> &'static [&'static str] {
    match container {
        Container::Id3v2 | Container::Riff | Container::Aiff => &["TKEY"],
        Container::Ilst => &["----:com.apple.iTunes:initialkey", "©cmt"],
        Container::VorbisComments => &["INITIALKEY", "KEY"],
        Container::Ape => &["Key"],
    }
}

/// ID3v2 restricts `TKEY` to at most three characters drawn from `A`-`G`, `b`,
/// `#`, `m`, plus `o` for off-key. So `Am`, `F#m` and `Bb` are legal and `11A`
/// is not.
///
/// Mixed In Key and others write Camelot there anyway and the DJ apps accept it,
/// but third-party taggers may reject or mangle it — which is why writing
/// Camelot is an explicit opt-in tied to the existing Key Notation setting
/// rather than a second control, and the default is sharps or flats.
pub fn is_tkey_legal(value: &str) -> bool {
    if value.is_empty() || value.len() > 3 {
        return false;
    }
    if value == "o" {
        return true;
    }
    value
        .chars()
        .all(|c| matches!(c, 'A'..='G' | 'b' | '#' | 'm'))
}

/// The value to write, honouring the opt-in. Returns the Camelot label only
/// when the user asked for it.
pub fn tag_value(
    root: u8,
    mode: KeyMode,
    notation: KeyNotation,
    allow_camelot: bool,
    pad_camelot: bool,
) -> String {
    use crate::analysis::camelot::Camelot;
    if allow_camelot && notation == KeyNotation::Camelot {
        return Camelot::from_key(root, mode).label(pad_camelot);
    }
    crate::analysis::camelot::key_tag_string(root, mode, notation)
}

/// Clears the key field(s) for this container. Backs the context menu's Remove
/// Key from Common Tags.
///
/// `container` does not decide *which* field gets cleared — the write path
/// resolves that from the file itself, and every entry [`fields_for`] lists for
/// a container is cleared together, because FLAC carries both `INITIALKEY` and
/// `KEY` and M4A carries the freeform atom plus a comment. The argument is
/// checked against the file instead, so a caller holding a stale container gets
/// an error rather than a no-op that looks like success.
pub fn clear(path: &std::path::Path, container: Container) -> Result<()> {
    let actual = crate::tags::container_for(path)?;
    if actual != container {
        return Err(crate::tags::tag_err(
            path,
            format!("holds a {} tag, not {}", actual.as_str(), container.as_str()),
        ));
    }
    let edit = crate::models::TagEdit::default().clear(crate::models::TagField::InitialKey);
    crate::tags::write::write(path, &edit, &crate::tags::write::WriteOptions::default())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tkey_accepts_what_the_spec_allows_and_nothing_else() {
        for ok in ["A", "Am", "F#m", "Bb", "Bbm", "o", "C"] {
            assert!(is_tkey_legal(ok), "{ok} should be legal");
        }
        // Camelot is the case that matters: it is not legal, which is the whole
        // reason writing it is an opt-in.
        for bad in ["11A", "8A", "G♭m", "", "Amin", "Cmaj", "H"] {
            assert!(!is_tkey_legal(bad), "{bad} should be rejected");
        }
    }

    #[test]
    fn the_default_tag_value_is_always_tkey_legal() {
        for root in 0u8..12 {
            for mode in [KeyMode::Major, KeyMode::Minor] {
                // Camelot notation with the opt-in OFF must still fall back to a
                // legal value rather than writing a wheel position.
                for notation in [KeyNotation::Camelot, KeyNotation::Sharps, KeyNotation::Flats] {
                    let v = tag_value(root, mode, notation, false, false);
                    assert!(is_tkey_legal(&v), "{v} is not legal in TKEY");
                }
            }
        }
    }

    #[test]
    fn the_opt_in_writes_camelot_and_only_then() {
        assert_eq!(tag_value(9, KeyMode::Minor, KeyNotation::Camelot, true, false), "8A");
        assert_eq!(tag_value(9, KeyMode::Minor, KeyNotation::Camelot, true, true), "08A");
        // Sharps notation ignores the Camelot opt-in: the user picked sharps.
        assert_eq!(tag_value(9, KeyMode::Minor, KeyNotation::Sharps, true, false), "Am");
    }

    #[test]
    fn every_container_has_at_least_one_key_field() {
        for c in [
            Container::Id3v2,
            Container::Ilst,
            Container::VorbisComments,
            Container::Ape,
            Container::Riff,
            Container::Aiff,
        ] {
            assert!(!fields_for(c).is_empty(), "{c:?} has no key field");
        }
    }
}
