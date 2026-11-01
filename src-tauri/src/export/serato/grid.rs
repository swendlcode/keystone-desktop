//! `Serato BeatGrid` and `Serato Autotags`.
//!
//! Both are small and both are big-endian. BeatGrid's last marker is a
//! different shape from the others — it carries a tempo rather than a beat
//! count — and there is always one, even when it is the only marker.

/// A downbeat, in seconds from the start of the file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMarker {
    pub position_s: f32,
    /// Beats until the next marker. Ignored on the terminal marker.
    pub beats_to_next: u32,
}

/// Builds the BeatGrid block.
///
/// An empty grid writes nothing rather than a zero-marker block: a grid with
/// no markers is not a claim Serato should be asked to interpret, and the
/// absence of the tag is the honest way to say the tempo is unknown.
pub fn build_beatgrid(markers: &[GridMarker], terminal_bpm: f32) -> Option<Vec<u8>> {
    if markers.is_empty() || !terminal_bpm.is_finite() || terminal_bpm <= 0.0 {
        return None;
    }

    let mut out = Vec::with_capacity(6 + markers.len() * 8 + 1);
    out.extend_from_slice(&[0x01, 0x00]);
    out.extend_from_slice(&(markers.len() as u32).to_be_bytes());

    let last = markers.len() - 1;
    for (i, m) in markers.iter().enumerate() {
        out.extend_from_slice(&m.position_s.to_be_bytes());
        if i == last {
            // The terminal marker carries the tempo from here to the end.
            out.extend_from_slice(&terminal_bpm.to_be_bytes());
        } else {
            out.extend_from_slice(&m.beats_to_next.to_be_bytes());
        }
    }

    // One trailing byte whose meaning is not documented. Zero rather than
    // the arbitrary value the sample happens to show: copying a byte from
    // somebody's example dump is pretending to know something.
    out.push(0x00);
    Some(out)
}

/// Builds the Autotags block: three NUL-terminated ASCII decimals.
///
/// Text, not floats, which is easy to get wrong because every other number
/// in these tags is binary. BPM carries two decimal places and the two gain
/// figures three, matching what Serato itself writes.
pub fn build_autotags(bpm: f64, auto_gain_db: f64, manual_gain_db: f64) -> Option<Vec<u8>> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return None;
    }
    let mut out = vec![0x01, 0x01];
    for text in [
        format!("{bpm:.2}"),
        format!("{auto_gain_db:.3}"),
        format!("{manual_gain_db:.3}"),
    ] {
        out.extend_from_slice(text.as_bytes());
        out.push(0x00);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented sample: one marker, 115 BPM. The terminal tempo must
    /// land as big-endian binary32.
    #[test]
    fn a_single_marker_grid_matches_the_documented_shape() {
        let out = build_beatgrid(
            &[GridMarker { position_s: 0.0, beats_to_next: 0 }],
            115.0,
        )
        .expect("a grid");

        assert_eq!(&out[0..2], &[0x01, 0x00]);
        assert_eq!(&out[2..6], &1u32.to_be_bytes());
        assert_eq!(&out[6..10], &0.0f32.to_be_bytes());
        // 115.0 as big-endian binary32 is 42 e6 00 00.
        assert_eq!(&out[10..14], &[0x42, 0xE6, 0x00, 0x00]);
        assert_eq!(out.len(), 15, "6 header + 8 marker + 1 footer");
    }

    #[test]
    fn only_the_last_marker_carries_the_tempo() {
        let out = build_beatgrid(
            &[
                GridMarker { position_s: 0.0, beats_to_next: 4 },
                GridMarker { position_s: 2.0, beats_to_next: 0 },
            ],
            128.0,
        )
        .expect("a grid");

        assert_eq!(&out[2..6], &2u32.to_be_bytes());
        // First marker: a beat count, not a tempo.
        assert_eq!(&out[10..14], &4u32.to_be_bytes());
        assert_eq!(&out[18..22], &128.0f32.to_be_bytes());
    }

    /// Writing a grid with no markers would be asserting a tempo of nothing.
    #[test]
    fn an_empty_grid_writes_no_tag_at_all() {
        assert!(build_beatgrid(&[], 128.0).is_none());
        assert!(build_beatgrid(
            &[GridMarker { position_s: 0.0, beats_to_next: 0 }],
            0.0
        )
        .is_none());
    }

    /// The numbers here are ASCII, unlike every other number in these tags.
    #[test]
    fn autotags_are_text_and_nul_separated() {
        let out = build_autotags(115.0, -3.257, 0.0).expect("autotags");
        assert_eq!(&out[0..2], &[0x01, 0x01]);
        // `\x00` rather than `\0`: a `\0` directly before a digit reads the
        // digits as part of the escape, which is how this literal first
        // claimed three leading zeros the code never wrote.
        assert_eq!(&out[2..], b"115.00\x00-3.257\x000.000\x00");
    }
}
