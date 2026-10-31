//! GiantSteps+ EDM Key, Zenodo record 1095691. CC BY-SA 4.0.
//!
//! Two facts to state **every time a number from this is reported**:
//!
//! - This is the revised **600-track** set with confidence levels and key-change
//!   annotations. It is **not byte-identical to the 604** the published numbers
//!   use, so our re-scores will differ slightly from the literature. Differences
//!   of a point or two mean nothing.
//! - The original Beatport preview URLs the 604-track set used are **404 dead** —
//!   every one tested. The audio comes from the Zenodo archive.
//!
//! It is also **84.8% minor**, which is why the harness reports mode accuracy
//! separately: a minor-biased model posts a good weighted score while being
//! useless.
//!
//! **The file layout below is inferred, not verified.** The archive is not on
//! this machine, so [`parse_key`] is written to accept every spelling the set is
//! documented or reported to use — `F minor`, `F# minor`, `Db major`, tab or
//! space separated, an optional trailing confidence integer, and more than one
//! key in a file for the key-change annotations. Verify it against the real
//! `keys.zip` before quoting any number that came through it; a loader that
//! silently drops two thirds of a corpus still produces a plausible-looking
//! score.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::KeyMode;

pub const TRACK_COUNT: usize = 600;
pub const MINOR_FRACTION: f32 = 0.848;

/// Extensions the audio half of the archive may use.
const AUDIO_EXTENSIONS: [&str; 4] = ["mp3", "wav", "flac", "m4a"];

pub struct Annotation {
    pub audio_path: PathBuf,
    pub root: u8,
    pub mode: KeyMode,
    /// The revised set's confidence level. Low-confidence tracks are reported
    /// separately rather than dropped — dropping the tracks every app failed is
    /// exactly what makes a marketed accuracy figure meaningless.
    pub confidence: Option<u8>,
    /// True when the annotation records a key change, which the 24-key taxonomy
    /// cannot represent.
    pub has_key_change: bool,
}

/// Pairs every annotation file under `corpus_dir` with its audio file.
///
/// Looks for `.key` files anywhere below the directory, and for the audio
/// beside them, under `audio/`, or under a sibling `audio/` of the annotation's
/// own parent — the three layouts the two zips produce when unpacked together,
/// separately, or into one tree.
///
/// An annotation whose audio is missing is **skipped silently by nothing**: it
/// is still returned, with the path it expected, so the caller counts it as a
/// failure to find rather than as a corpus that is simply smaller. A harness
/// that quietly shrinks its own corpus is the single easiest way to publish a
/// wrong number.
pub fn load(corpus_dir: &Path) -> Result<Vec<Annotation>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(corpus_dir).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("key") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        let Some(parsed) = parse_annotation(&text) else { continue };
        out.push(Annotation {
            audio_path: audio_for(path, corpus_dir),
            root: parsed.root,
            mode: parsed.mode,
            confidence: parsed.confidence,
            has_key_change: parsed.has_key_change,
        });
    }
    out.sort_by(|a, b| a.audio_path.cmp(&b.audio_path));
    Ok(out)
}

/// The audio file an annotation refers to.
///
/// `1234.LOFI.mp3.key` names its audio outright; `1234.key` does not, so every
/// supported extension is tried in each candidate directory before falling back
/// to the stem as written.
fn audio_for(key_path: &Path, corpus_dir: &Path) -> PathBuf {
    let stem = key_path.file_stem().map(PathBuf::from).unwrap_or_default();
    let parent = key_path.parent().unwrap_or(corpus_dir);
    let candidates = [
        parent.to_path_buf(),
        parent.join("audio"),
        parent.parent().unwrap_or(corpus_dir).join("audio"),
        corpus_dir.join("audio"),
    ];
    for dir in &candidates {
        let direct = dir.join(&stem);
        if direct.extension().is_some() && direct.is_file() {
            return direct;
        }
        for ext in AUDIO_EXTENSIONS {
            let with_ext = dir.join(&stem).with_extension(ext);
            if with_ext.is_file() {
                return with_ext;
            }
        }
    }
    parent.join(stem)
}

struct Parsed {
    root: u8,
    mode: KeyMode,
    confidence: Option<u8>,
    has_key_change: bool,
}

/// Reads a whole annotation file: one key per non-empty line or per `|`-
/// separated field, with the first taken as the reference and any second
/// distinct key recorded as a key change.
fn parse_annotation(text: &str) -> Option<Parsed> {
    let mut keys: Vec<(u8, KeyMode)> = Vec::new();
    let mut confidence = None;
    for field in text.split(['\n', '\r', '|']) {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        if let Some(key) = parse_key(field) {
            keys.push(key);
            if confidence.is_none() {
                confidence = trailing_confidence(field);
            }
        }
    }
    let first = *keys.first()?;
    let has_key_change = keys.iter().any(|k| *k != first);
    Some(Parsed { root: first.0, mode: first.1, confidence, has_key_change })
}

fn trailing_confidence(field: &str) -> Option<u8> {
    field.split_whitespace().last()?.parse::<u8>().ok()
}

/// Parses an annotation string such as `F minor` into (root, mode).
///
/// Accepts tabs or spaces, enharmonic spellings with `#`, `b` or the
/// typographic flat, and an optional trailing confidence integer. Returns
/// `None` for anything it does not recognise, including the set's own
/// no-key markers, rather than guessing a key: a mis-parsed reference scores
/// every estimate against the wrong answer and looks like a detector bug.
pub fn parse_key(s: &str) -> Option<(u8, KeyMode)> {
    let cleaned = s.replace('\t', " ").replace('♭', "b").replace('♯', "#");
    let mut parts = cleaned.split_whitespace().filter(|p| p.parse::<u8>().is_err());
    let root_token = parts.next()?;
    let mode_token = parts.next();

    // `Cm` / `F#m` with no separator.
    let (root_token, inline_minor) = match root_token.strip_suffix('m') {
        Some(stripped) if mode_token.is_none() && !stripped.is_empty() => (stripped, true),
        _ => (root_token, false),
    };

    let root = pitch_class(root_token)?;
    let mode = match mode_token.map(|m| m.to_ascii_lowercase()) {
        Some(m) if m.starts_with("maj") => KeyMode::Major,
        Some(m) if m.starts_with("min") || m == "m" => KeyMode::Minor,
        Some(_) => return None,
        None if inline_minor => KeyMode::Minor,
        // A bare pitch name is major, which is how the set spells major keys in
        // the shortest of the layouts.
        None => KeyMode::Major,
    };
    Some((root, mode))
}

fn pitch_class(token: &str) -> Option<u8> {
    let mut chars = token.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut semitones: i32 = base;
    for c in chars {
        match c {
            '#' => semitones += 1,
            'b' | 'B' => semitones -= 1,
            _ => return None,
        }
    }
    Some(semitones.rem_euclid(12) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KeyMode::{Major, Minor};

    #[test]
    fn the_spellings_the_set_is_reported_to_use_all_parse() {
        for (text, expected) in [
            ("F minor", (5, Minor)),
            ("F# minor", (6, Minor)),
            ("Db major", (1, Major)),
            ("C\tmajor", (0, Major)),
            ("Bb\tmin", (10, Minor)),
            ("A", (9, Major)),
            ("Cm", (0, Minor)),
            ("G major 2", (7, Major)),
            ("g MAJOR", (7, Major)),
        ] {
            assert_eq!(parse_key(text), Some(expected), "{text}");
        }
        for text in ["", "silence", "none", "-", "H minor", "C lydian", "9"] {
            assert_eq!(parse_key(text), None, "{text} should not parse as a key");
        }
    }

    #[test]
    fn a_file_with_two_keys_is_flagged_rather_than_averaged() {
        let p = parse_annotation("C minor\nEb major\n").expect("parsed");
        assert_eq!((p.root, p.mode), (0, Minor), "the first key is the reference");
        assert!(p.has_key_change);

        let steady = parse_annotation("C minor\nC minor\n").expect("parsed");
        assert!(!steady.has_key_change, "the same key twice is not a key change");

        let with_confidence = parse_annotation("A minor 2").expect("parsed");
        assert_eq!(with_confidence.confidence, Some(2));
        assert_eq!(with_confidence.root, 9);
    }

    #[test]
    fn a_missing_corpus_loads_as_empty_rather_than_failing() {
        // The harness has to be able to say "no corpus" rather than panic on a
        // machine that does not have the 855 MB archive, which is every machine
        // except the one it was downloaded to.
        let empty = load(Path::new("/nonexistent/giantsteps")).expect("load");
        assert!(empty.is_empty());
    }
}
