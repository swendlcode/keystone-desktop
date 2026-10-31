//! Building the new filename, and taking it back off again.
//!
//! Everything here is pure string work on a single path component, so it is
//! unit-testable on every platform — which matters, because the rules it
//! encodes are Windows' and the machine this was written on is not.

use crate::analysis::camelot::Camelot;
use crate::models::RenamePattern;
use crate::tags::MARKER_SEPARATOR;

/// Maximum bytes in one path component on the filesystems we target.
pub const MAX_COMPONENT_BYTES: usize = 255;

/// Characters Windows rejects in a filename. `/` is included for unix too: a
/// slash in a stem would silently become a directory level.
const ILLEGAL: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Reserved device names. Windows resolves these *before* looking at the
/// filesystem, so `CON.mp3` is not a file — it is the console — and the open
/// succeeds while writing nothing anyone can find again.
const RESERVED: [&str; 26] = [
    "CON", "PRN", "AUX", "NUL", "CLOCK$", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
    "COM7", "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
    "LPT8", "LPT9", "CONIN$",
];

/// A plausible tempo, used only by the fallback strip. Narrow on purpose: it is
/// what keeps a track called `Midnight - 1999` intact.
const TEMPO_RANGE: std::ops::RangeInclusive<u32> = 50..=400;

/// Composes the new stem from the pattern. `key` and `tempo` are already
/// formatted by the Key Notation and Tempo settings; this function does not
/// re-decide notation.
///
/// Idempotent, and that is the point: `Track - 8A - 128 - 8A - 128` must be
/// impossible. The guard is an exact comparison of the segments against the
/// `key` and `tempo` it was handed, not a pattern match, so a track genuinely
/// called `8A` is still renamed correctly.
pub fn compose_stem(original: &str, pattern: RenamePattern, key: &str, tempo: &str) -> String {
    let at_end = matches!(
        pattern,
        RenamePattern::OriginalKey | RenamePattern::OriginalKeyTempo
    );
    let markers: Vec<&str> = match pattern {
        RenamePattern::OriginalKey | RenamePattern::KeyOriginal => vec![key],
        RenamePattern::OriginalKeyTempo | RenamePattern::KeyTempoOriginal => vec![key, tempo],
        RenamePattern::TempoKeyOriginal => vec![tempo, key],
    }
    .into_iter()
    .filter(|m| !m.is_empty())
    .collect();

    if markers.is_empty() {
        return original.to_string();
    }

    // Already applied? `original` should be the pre-rename stem, but a row with
    // no `original_filename` falls back to the current name, so this is the
    // backstop that makes a second pass a no-op.
    let segments: Vec<&str> = original.split(MARKER_SEPARATOR).collect();
    if already_applied(&segments, &markers, at_end) {
        return original.to_string();
    }

    let joined = markers.join(MARKER_SEPARATOR);
    if at_end {
        format!("{original}{MARKER_SEPARATOR}{joined}")
    } else {
        format!("{joined}{MARKER_SEPARATOR}{original}")
    }
}

fn already_applied(segments: &[&str], markers: &[&str], at_end: bool) -> bool {
    // Strictly greater: a track whose whole name *is* the key still gets the
    // marker, and the pass after that one stops.
    if segments.len() <= markers.len() {
        return false;
    }
    let window = if at_end {
        &segments[segments.len() - markers.len()..]
    } else {
        &segments[..markers.len()]
    };
    window == markers
}

/// The exact inverse, using `original_filename` rather than a pattern match.
/// Backs the context menu's Remove Key from File Name.
///
/// With `original_filename` this is exact. Without it — a library renamed by
/// some other tool, or a row from before the column was filled — it falls back
/// to stripping segments that *parse* as a Camelot code, plus a tempo segment
/// only when one sits next to a stripped key. That pairing rule is what keeps
/// `Midnight - 128` whole while `Midnight - 8A - 128` comes apart correctly.
pub fn strip_stem(current: &str, original_filename: Option<&str>) -> String {
    if let Some(name) = original_filename {
        return stem_of(name).to_string();
    }

    let mut segments: Vec<&str> = current.split(MARKER_SEPARATOR).collect();
    strip_end(&mut segments);
    strip_start(&mut segments);
    if segments.is_empty() {
        return current.to_string();
    }
    segments.join(MARKER_SEPARATOR)
}

/// `Original - Key[ - Tempo]`.
fn strip_end(segments: &mut Vec<&str>) {
    let tempo_last = segments
        .last()
        .is_some_and(|s| Camelot::parse(s).is_none() && is_tempo(s));
    let key_at = if tempo_last { 2 } else { 1 };
    if segments.len() > key_at
        && Camelot::parse(segments[segments.len() - key_at]).is_some()
    {
        segments.truncate(segments.len() - key_at);
    }
}

/// `Key[ - Tempo] - Original` and `Tempo - Key - Original`.
fn strip_start(segments: &mut Vec<&str>) {
    let mut drop = 0;
    if segments.len() > 1 && Camelot::parse(segments[0]).is_some() {
        drop = 1;
        if segments.len() > 2 && is_tempo(segments[1]) {
            drop = 2;
        }
    } else if segments.len() > 2 && is_tempo(segments[0]) && Camelot::parse(segments[1]).is_some() {
        drop = 2;
    }
    segments.drain(..drop);
}

fn is_tempo(s: &str) -> bool {
    s.parse::<u32>().is_ok_and(|n| TEMPO_RANGE.contains(&n))
}

/// Replaces characters the target filesystem rejects and escapes reserved
/// device names.
///
/// Trailing dots and spaces go too: Windows silently drops them, so the name on
/// disk would not be the name Keystone recorded, and the row would point at a
/// file that does not exist under that spelling.
pub fn sanitize_component(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if ILLEGAL.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();

    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    let trimmed = out.trim_start().to_string();
    out = trimmed;

    if out.is_empty() {
        return "_".to_string();
    }
    if is_reserved(stem_of(&out)) {
        out.insert(0, '_');
    }
    out
}

fn is_reserved(stem: &str) -> bool {
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
}

/// Truncates to fit `max_bytes`, taking the bytes off the `original` segment
/// and never off `suffix`.
///
/// `suffix` is everything that must survive — the key, the tempo and the
/// extension. Truncating that instead would defeat the whole point of the
/// rename, and dropping the extension would make the file unplayable.
pub fn truncate_component(original: &str, suffix: &str, max_bytes: usize) -> String {
    if original.len() + suffix.len() <= max_bytes {
        return format!("{original}{suffix}");
    }
    let room = max_bytes.saturating_sub(suffix.len());
    let mut end = room.min(original.len());
    // UTF-8: step back to a character boundary rather than splitting a
    // multi-byte character and producing a name no filesystem will accept.
    while end > 0 && !original.is_char_boundary(end) {
        end -= 1;
    }
    // A cut that lands mid-separator would leave `Midni -  - 8A.mp3`.
    let kept = original[..end].trim_end_matches([' ', '-']);
    format!("{kept}{suffix}")
}

/// The file name without its final extension. Not `Path::file_stem`, because
/// this runs on a bare component and must behave identically on every platform.
pub fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        // A leading dot is a hidden file, not an extension.
        Some(0) | None => name,
        Some(i) => &name[..i],
    }
}

/// The extension including the dot, or empty.
pub fn ext_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(0) | None => "",
        Some(i) => &name[i..],
    }
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
