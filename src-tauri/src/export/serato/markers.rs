//! The two cue blocks: modern `Markers2` and legacy `Markers_`.
//!
//! Both are written, always, because Serato prefers the legacy block where
//! the two disagree. Writing only the modern one leaves the first five cues
//! reading from whatever was there before.
//!
//! Implemented from the published field documentation; nothing is copied
//! from it. Where that documentation is ambiguous — it marks several bytes
//! "unknown" — the choice here is to reproduce the sample values it shows
//! rather than to guess a meaning, and to preserve whatever the file already
//! had in preference to inventing a value.

use crate::export::serato::serato32;

/// Base64 in these tags wraps at this width.
pub const LINE_WIDTH: usize = 72;
/// Markers2 is null-padded to at least this length.
pub const MARKERS2_MIN_BYTES: usize = 470;
/// `Markers_` is a FIXED 14 slots: five cues then nine loops.
pub const LEGACY_SLOTS: usize = 14;
pub const LEGACY_CUES: usize = 5;
const LEGACY_ENTRY_BYTES: usize = 22;

/// One cue, reduced to what the tags carry.
#[derive(Debug, Clone, PartialEq)]
pub struct SeratoCue {
    /// 0-based hot cue index.
    pub index: u8,
    pub position_ms: u32,
    pub rgb: [u8; 3],
    pub label: String,
}

// ── Markers2 ────────────────────────────────────────────────────────────

/// An entry read out of an existing Markers2 block.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEntry {
    pub name: String,
    pub body: Vec<u8>,
}

/// Builds the decoded Markers2 body: `01 01`, the entries, then a `00`.
///
/// `keep` is every entry from the file that this writer does not own —
/// loops, flips, track colour, anything a later Serato version introduced.
/// They are copied through byte for byte. The alternative is deleting a
/// user's loops because this build did not recognise them, and the format
/// carries an explicit length per entry precisely so that a reader that does
/// not understand one can still step over it.
pub fn build_markers2_body(cues: &[SeratoCue], keep: &[RawEntry]) -> Vec<u8> {
    let mut out = vec![0x01, 0x01];

    for entry in keep {
        push_entry(&mut out, &entry.name, &entry.body);
    }
    for cue in cues {
        push_entry(&mut out, "CUE", &cue_entry_body(cue));
    }

    out.push(0x00);
    out
}

fn push_entry(out: &mut Vec<u8>, name: &str, body: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(0x00);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
}

/// One CUE entry's body.
///
/// The two gap bytes and the pair after the colour are undocumented; the
/// samples show them zero and that is what goes in. The name is UTF-8 and
/// NUL-terminated.
fn cue_entry_body(cue: &SeratoCue) -> Vec<u8> {
    let mut body = Vec::with_capacity(13 + cue.label.len());
    body.push(cue.index);
    body.push(0x00);
    body.extend_from_slice(&cue.position_ms.to_be_bytes());
    body.push(0x00);
    body.extend_from_slice(&cue.rgb);
    body.extend_from_slice(&[0x00, 0x00]);
    body.extend_from_slice(cue.label.as_bytes());
    body.push(0x00);
    body
}

/// Reads the entries out of a decoded Markers2 body.
///
/// Returns every entry it finds, including ones it does not understand —
/// that is the whole point, so they can be written back untouched.
pub fn parse_markers2_body(body: &[u8]) -> Vec<RawEntry> {
    let mut entries = Vec::new();
    // Skip the two-byte header; stop at the terminating NUL.
    let mut i = 2usize;

    while i < body.len() {
        let Some(end) = body[i..].iter().position(|&b| b == 0) else {
            break;
        };
        if end == 0 {
            break; // the terminator
        }
        let Ok(name) = std::str::from_utf8(&body[i..i + end]) else {
            break;
        };
        let name = name.to_string();
        i += end + 1;

        if i + 4 > body.len() {
            break;
        }
        let len = u32::from_be_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]) as usize;
        i += 4;
        if i + len > body.len() {
            break;
        }
        entries.push(RawEntry {
            name,
            body: body[i..i + len].to_vec(),
        });
        i += len;
    }

    entries
}

/// Wraps a decoded body into the stored payload: `01 01`, base64 at 72
/// columns, then NUL padding to the minimum length.
pub fn wrap_markers2(body: &[u8]) -> Vec<u8> {
    let encoded = base64_wrapped(body, LINE_WIDTH);
    let mut out = Vec::with_capacity(MARKERS2_MIN_BYTES.max(encoded.len() + 3));
    out.extend_from_slice(&[0x01, 0x01]);
    out.extend_from_slice(encoded.as_bytes());
    out.push(0x00);
    while out.len() < MARKERS2_MIN_BYTES {
        out.push(0x00);
    }
    out
}

/// The inverse: strips the header and padding and decodes the base64.
pub fn unwrap_markers2(payload: &[u8]) -> Option<Vec<u8>> {
    let text = payload.get(2..)?;
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    let cleaned: String = std::str::from_utf8(&text[..end])
        .ok()?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64_decode(&cleaned)
}

// ── Markers_ (legacy) ───────────────────────────────────────────────────

/// Builds the fixed 14-slot legacy block.
///
/// **Fixed is the dangerous word.** Slots five through thirteen are the
/// user's loops and this writer never produces any, so `keep_loops` carries
/// the existing bytes for those slots straight back out. Emitting a shorter
/// block, or zeroing the slots this build has nothing to say about, deletes
/// every loop in the track and reports success — the block has no field that
/// distinguishes "no loop here" from "this writer did not look".
pub fn build_markers_legacy(
    cues: &[SeratoCue],
    keep_loops: &[[u8; LEGACY_ENTRY_BYTES]],
    track_rgb: [u8; 3],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + LEGACY_SLOTS * LEGACY_ENTRY_BYTES + 4);
    out.extend_from_slice(&[0x02, 0x05]);
    out.extend_from_slice(&(LEGACY_SLOTS as u32).to_be_bytes());

    for slot in 0..LEGACY_CUES {
        match cues.iter().find(|c| usize::from(c.index) == slot) {
            Some(cue) => out.extend_from_slice(&legacy_cue_entry(cue)),
            None => out.extend_from_slice(&empty_legacy_entry(0x01)),
        }
    }

    for slot in 0..(LEGACY_SLOTS - LEGACY_CUES) {
        match keep_loops.get(slot) {
            Some(existing) => out.extend_from_slice(existing),
            None => out.extend_from_slice(&empty_legacy_entry(0x03)),
        }
    }

    out.extend_from_slice(&serato32::encode_rgb(track_rgb));
    out
}

fn legacy_cue_entry(cue: &SeratoCue) -> [u8; LEGACY_ENTRY_BYTES] {
    let mut e = [0u8; LEGACY_ENTRY_BYTES];
    e[0] = 0x00; // start position is set
    e[1..5].copy_from_slice(&serato32::encode_u24(cue.position_ms));
    e[5] = 0x7F; // a cue has no end position
    e[6..10].copy_from_slice(&serato32::UNSET);
    // Six undocumented bytes; the samples show this pattern.
    e[10..16].copy_from_slice(&[0x00, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F]);
    e[16..20].copy_from_slice(&serato32::encode_rgb(cue.rgb));
    e[20] = 0x01; // cue
    e[21] = 0x00; // unlocked
    e
}

fn empty_legacy_entry(kind: u8) -> [u8; LEGACY_ENTRY_BYTES] {
    let mut e = [0u8; LEGACY_ENTRY_BYTES];
    e[0] = 0x7F;
    e[1..5].copy_from_slice(&serato32::UNSET);
    e[5] = 0x7F;
    e[6..10].copy_from_slice(&serato32::UNSET);
    e[10..16].copy_from_slice(&[0x00, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F]);
    e[16..20].copy_from_slice(&serato32::UNSET);
    e[20] = kind;
    e[21] = 0x00;
    e
}

/// Pulls the nine loop slots out of an existing legacy block, so they can be
/// written back untouched. Returns nothing if the block is not the shape this
/// code understands — in which case the caller must refuse to write rather
/// than replace it with a block of empties.
pub fn existing_loop_slots(block: &[u8]) -> Option<Vec<[u8; LEGACY_ENTRY_BYTES]>> {
    let expected = 6 + LEGACY_SLOTS * LEGACY_ENTRY_BYTES;
    if block.len() < expected {
        return None;
    }
    let count = u32::from_be_bytes([block[2], block[3], block[4], block[5]]) as usize;
    if count != LEGACY_SLOTS {
        return None;
    }
    let mut loops = Vec::with_capacity(LEGACY_SLOTS - LEGACY_CUES);
    for slot in LEGACY_CUES..LEGACY_SLOTS {
        let at = 6 + slot * LEGACY_ENTRY_BYTES;
        let mut e = [0u8; LEGACY_ENTRY_BYTES];
        e.copy_from_slice(&block[at..at + LEGACY_ENTRY_BYTES]);
        loops.push(e);
    }
    Some(loops)
}

// ── base64, wrapped the way these tags want it ──────────────────────────

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, a line feed every `width` characters.
pub fn base64_wrapped(data: &[u8], width: usize) -> String {
    let mut out = String::new();
    let mut column = 0usize;
    let push = |c: char, out: &mut String, column: &mut usize| {
        if width > 0 && *column == width {
            out.push('\n');
            *column = 0;
        }
        out.push(c);
        *column += 1;
    };

    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = [
            B64[(n >> 18) as usize & 63] as char,
            B64[(n >> 12) as usize & 63] as char,
            B64[(n >> 6) as usize & 63] as char,
            B64[n as usize & 63] as char,
        ];
        let keep = chunk.len() + 1;
        for (i, c) in chars.iter().enumerate() {
            push(if i < keep { *c } else { '=' }, &mut out, &mut column);
        }
    }
    out
}

pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in text.chars() {
        if c == '=' || c.is_whitespace() {
            continue;
        }
        let v = B64.iter().position(|&b| b as char == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(index: u8, ms: u32, label: &str) -> SeratoCue {
        SeratoCue {
            index,
            position_ms: ms,
            rgb: [0xCC, 0x00, 0x00],
            label: label.to_string(),
        }
    }

    #[test]
    fn a_markers2_body_round_trips_through_its_own_parser() {
        let body = build_markers2_body(&[cue(0, 1000, "Intro"), cue(1, 2500, "Drop")], &[]);
        let parsed = parse_markers2_body(&body);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|e| e.name == "CUE"));
        assert_eq!(parsed[0].body[0], 0);
        assert_eq!(&parsed[0].body[2..6], &1000u32.to_be_bytes());
        assert!(parsed[0].body.ends_with(b"Intro\0"));
    }

    /// The format stores a length per entry so a reader can step over what it
    /// does not understand. Rewriting must use that rather than discard it:
    /// a FLIP or a loop from a newer Serato has to come back out unchanged.
    #[test]
    fn entries_this_writer_does_not_understand_survive_a_rewrite() {
        let foreign = RawEntry {
            name: "FLIP".to_string(),
            body: vec![9, 8, 7, 6, 5],
        };
        let colour = RawEntry {
            name: "COLOR".to_string(),
            body: vec![0x00, 0x99, 0xFF, 0x99],
        };

        let body = build_markers2_body(&[cue(0, 500, "A")], &[foreign.clone(), colour.clone()]);
        let parsed = parse_markers2_body(&body);

        assert!(parsed.contains(&foreign), "a FLIP entry was dropped");
        assert!(parsed.contains(&colour), "the track colour was dropped");
    }

    #[test]
    fn the_wrapped_payload_decodes_back_to_the_body() {
        let body = build_markers2_body(&[cue(3, 61_234, "Break")], &[]);
        let payload = wrap_markers2(&body);

        assert_eq!(&payload[0..2], &[0x01, 0x01]);
        assert!(payload.len() >= MARKERS2_MIN_BYTES, "short of the minimum");
        assert_eq!(unwrap_markers2(&payload).expect("decodes"), body);
    }

    /// Serato's own writer wraps at 72; a reader that splits on line length
    /// rather than on base64 validity would mis-read an unwrapped block.
    #[test]
    fn long_payloads_wrap_at_seventy_two_columns() {
        let cues: Vec<SeratoCue> = (0..8)
            .map(|i| cue(i, u32::from(i) * 10_000, "a reasonably long cue label"))
            .collect();
        let encoded = base64_wrapped(&build_markers2_body(&cues, &[]), LINE_WIDTH);

        assert!(encoded.contains('\n'), "expected wrapping");
        assert!(
            encoded.split('\n').all(|line| line.len() <= LINE_WIDTH),
            "a line ran past the wrap width",
        );
    }

    // ── the legacy block ────────────────────────────────────────────────

    #[test]
    fn the_legacy_block_is_always_the_documented_length() {
        let block = build_markers_legacy(&[cue(0, 1000, "A")], &[], [0xFF, 0xFF, 0xFF]);
        assert_eq!(block.len(), 6 + LEGACY_SLOTS * 22 + 4);
        assert_eq!(&block[0..2], &[0x02, 0x05]);
        assert_eq!(&block[2..6], &(LEGACY_SLOTS as u32).to_be_bytes());
        assert_eq!(&block[block.len() - 4..], &serato32::encode_rgb([0xFF, 0xFF, 0xFF]));
    }

    /// The reason this file is careful. Slots 5..14 are the user's loops and
    /// nothing here creates one, so they must be carried through verbatim.
    /// A block that zeroes them is a silent deletion: the format has no way
    /// to say "this writer had nothing to add here".
    #[test]
    fn existing_loops_are_carried_through_untouched() {
        let mut loops = Vec::new();
        for slot in 0..9u8 {
            let mut e = [0u8; 22];
            e[0] = 0x00;
            e[20] = 0x03;
            e[21] = slot; // a marker this test can recognise again
            loops.push(e);
        }

        let block = build_markers_legacy(&[cue(0, 1000, "A")], &loops, [0, 0, 0]);
        let recovered = existing_loop_slots(&block).expect("the block parses");

        assert_eq!(recovered, loops, "a loop slot was altered");
    }

    #[test]
    fn a_block_that_is_not_the_expected_shape_is_refused() {
        assert!(existing_loop_slots(&[0x02, 0x05, 0, 0, 0, 14]).is_none());
        let mut wrong = vec![0x02, 0x05];
        wrong.extend_from_slice(&7u32.to_be_bytes());
        wrong.resize(6 + 14 * 22 + 4, 0);
        assert!(
            existing_loop_slots(&wrong).is_none(),
            "a block claiming a different slot count must not be trusted",
        );
    }

    #[test]
    fn a_cue_slot_with_no_cue_is_marked_unset_rather_than_zero() {
        let block = build_markers_legacy(&[], &[], [0, 0, 0]);
        // First entry, first byte: 0x7f means "no start position".
        assert_eq!(block[6], 0x7F);
        assert_eq!(&block[7..11], &serato32::UNSET);
    }
}
