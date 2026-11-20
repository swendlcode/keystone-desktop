//! Reading `Artist - Title` out of a filename, and dropping what a video site
//! put there.
//!
//! A library downloaded from YouTube arrives with the uploader channel in the
//! artist tag — `Defected Records`, `Spinnin' Records`, `Becky Hill
//! Official` — and `People & Blogs` or `Music` in the genre, while the
//! filename is the only place the real credit survives:
//!
//! ```text
//! Tom & Collins & OMRI. - Dancing Shoes (Official Full Stream) [3YIsRfh7npE]
//! ```
//!
//! So the filename is the source and the noise is what has to come off it.
//! The whole difficulty is **which** bracketed group is noise: `(Official
//! Video)` is, and `(James Hype Remix)`, `(pull me out of this)`,
//! `(Overdrive)` and `[Make The Girls Dance Records]` are not. A blocklist of
//! words appearing anywhere would eat all four, so a group is dropped only
//! when **everything** in it is noise. Anything with a word this module does
//! not recognise is left alone, which is the safe direction: a title that
//! keeps a redundant `(Audio)` is a cosmetic flaw, and one that loses
//! `(James Hype Remix)` is a different record.

/// Words that only ever describe a video or a publication, never the music.
///
/// `remaster`, `live`, `preview` and `snippet` are deliberately absent: each
/// says something true about which recording this is.
const NOISE: &[&str] = &[
    "official", "video", "videoclip", "audio", "lyric", "lyrics", "lyricvideo", "visualiser",
    "visualizer", "visual", "visualizers", "music", "mv", "full", "stream", "fullstream", "hd",
    "hq", "uhd", "4k", "8k", "1080p", "720p", "out", "now", "outnow", "free", "download",
    "downloads", "dl", "freedownload", "freedl", "premiere", "exclusive", "teaser", "trailer",
    "clip", "new", "copyright", "ncs", "release", "pseudo", "topic", "remastered2021",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    /// `None` when the name carries no `Artist - Title` separator at all.
    pub artist: Option<String>,
    pub title: String,
}

/// Parses a filename stem. The extension must already be off.
pub fn parse(stem: &str) -> ParsedName {
    let cleaned = clean(stem);
    split_artist_title(&cleaned)
}

/// Everything a video site appended, removed.
///
/// Also the eleven-character video id in square brackets, which is a precise
/// enough shape to match on: `[3YIsRfh7npE]` goes and `[Make The Girls Dance
/// Records]` stays.
pub fn clean(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut rest = normalize_dashes(stem);
    rest = strip_leading_index(&rest);

    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let Some(close) = closing(&chars, i) else {
            out.push(chars[i]);
            i += 1;
            continue;
        };
        let inner: String = chars[i + 1..close].iter().collect();
        if is_noise(&inner) || is_video_id(&inner) {
            // The group and nothing else: the space that preceded it is
            // tidied by `collapse`, so `A (Official Video) B` does not become
            // `A  B`.
            i = close + 1;
            continue;
        }
        out.extend(&chars[i..=close]);
        i = close + 1;
    }
    collapse(&out)
}

/// The index of the matching bracket, if this character opens one.
///
/// Nesting is not supported and does not need to be: it does not occur in
/// these names, and the first close is the right answer for `(a (b)` either
/// way.
fn closing(chars: &[char], at: usize) -> Option<usize> {
    let want = match chars[at] {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    chars[at + 1..]
        .iter()
        .position(|c| *c == want)
        .map(|p| at + 1 + p)
}

/// Is every word in this group noise?
fn is_noise(inner: &str) -> bool {
    let words: Vec<String> = inner
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();
    !words.is_empty() && words.iter().all(|w| NOISE.contains(&w.as_str()))
}

/// A YouTube id: exactly eleven characters of the URL-safe alphabet, plus one
/// test that separates it from an eleven-letter word.
///
/// The length alone is not enough — `[Remastered]` is eleven letters — and
/// "contains a digit" is not enough either, because plenty of real ids have
/// none: `naCvEJABUyc` and `MX7h4FvFPLg` both came out of this library. What
/// an id has and a word does not is case that changes mid-string: a word in
/// brackets is lower case, Title Case or SHOUTED, and none of those put an
/// upper-case letter straight after a lower-case one. So a digit, a `-`, a
/// `_`, or that transition.
fn is_video_id(inner: &str) -> bool {
    if inner.len() != 11
        || !inner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return false;
    }
    let has_symbol_or_digit = inner
        .chars()
        .any(|c| c.is_ascii_digit() || c == '-' || c == '_');
    let camel = inner
        .chars()
        .zip(inner.chars().skip(1))
        .any(|(a, b)| a.is_ascii_lowercase() && b.is_ascii_uppercase());
    has_symbol_or_digit || camel
}

/// En and em dashes become the plain separator, so one split rule covers
/// `Artist – Title` as well as `Artist - Title`.
fn normalize_dashes(stem: &str) -> String {
    stem.replace(['\u{2013}', '\u{2014}', '\u{2015}'], "-")
}

/// `01. `, `01 - `, `03_` at the front of a name. A number on its own is
/// never an artist.
fn strip_leading_index(stem: &str) -> String {
    let trimmed = stem.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 3 {
        return stem.to_string();
    }
    let after = trimmed[digits.len()..].trim_start();
    let after = after
        .strip_prefix('.')
        .or_else(|| after.strip_prefix('-'))
        .or_else(|| after.strip_prefix('_'))
        .map(str::trim_start);
    match after {
        // `128 - 4A - Track` is a tempo, not an index, but that shape is the
        // renamer's and has already been stripped by `strip_stem`. What is
        // guarded here is the other direction: a name that is only digits.
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => stem.to_string(),
    }
}

/// Runs of whitespace to one space, and separators left dangling by a removed
/// group taken off the ends.
fn collapse(value: &str) -> String {
    let single: String = {
        let mut out = String::with_capacity(value.len());
        let mut last_space = false;
        for c in value.chars() {
            let space = c.is_whitespace();
            if !space || !last_space {
                out.push(if space { ' ' } else { c });
            }
            last_space = space;
        }
        out
    };
    single.trim().trim_end_matches(['-', '|', ',', ';']).trim().to_string()
}

/// Splits on the **first** separator, because everything after it belongs to
/// the title: `Odd Mob, OMNOM, HYPERBEAM - Coming Up (It's Dare)` has commas
/// in the artist and `Fred again.. - Delilah (pull me out of this)` has dots.
fn split_artist_title(cleaned: &str) -> ParsedName {
    if let Some((left, right)) = cleaned.split_once(" - ") {
        let artist = left.trim();
        let title = right.trim();
        if !artist.is_empty() && !title.is_empty() {
            return ParsedName {
                artist: Some(artist.to_string()),
                title: title.to_string(),
            };
        }
    }
    ParsedName {
        artist: None,
        title: cleaned.trim().to_string(),
    }
}

#[cfg(test)]
#[path = "filename_tests.rs"]
mod tests;
