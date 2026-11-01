//! Serato's 7-bit packing for positions and colours in the legacy
//! `Markers_` tag.
//!
//! Three plaintext bytes are spread across four stored bytes, seven bits
//! each, most significant first. The top bit of every stored byte stays
//! clear — which is the point: the block sits inside an ID3 frame that
//! predates binary-safe handling, and keeping every byte under 0x80 means
//! nothing downstream mistakes a position for a terminator.
//!
//! Implemented from the published field documentation and verified against
//! the two sample values it gives. The decode direction is documented; the
//! encode direction here is its inverse, and the tests pin both so an
//! inverted shift cannot ship — a wrong position in this tag is a cue point
//! silently landing somewhere else in the user's track.

/// Four stored bytes back to the three they carry.
pub fn decode(packed: [u8; 4]) -> [u8; 3] {
    let [w, x, y, z] = packed;
    let c = (z & 0x7F) | ((y & 0x01) << 7);
    let b = ((y & 0x7F) >> 1) | ((x & 0x03) << 6);
    let a = ((x & 0x7F) >> 2) | ((w & 0x07) << 5);
    [a, b, c]
}

/// Three plaintext bytes into the four they are stored as.
pub fn encode(plain: [u8; 3]) -> [u8; 4] {
    let [a, b, c] = plain;
    [
        (a >> 5) & 0x07,
        ((a & 0x1F) << 2) | ((b >> 6) & 0x03),
        ((b & 0x3F) << 1) | ((c >> 7) & 0x01),
        c & 0x7F,
    ]
}

/// A millisecond position. Three bytes hold up to 16,777,215 ms — four and a
/// half hours, so saturating rather than wrapping is safe for any real track
/// and wrapping would put a cue at the start of the file.
pub fn encode_u24(value: u32) -> [u8; 4] {
    let v = value.min(0x00FF_FFFF);
    encode([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn decode_u24(packed: [u8; 4]) -> u32 {
    let [a, b, c] = decode(packed);
    (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c)
}

/// `#RRGGBB` into the stored form.
pub fn encode_rgb(rgb: [u8; 3]) -> [u8; 4] {
    encode(rgb)
}

/// The value meaning "this slot holds nothing".
pub const UNSET: [u8; 4] = [0x7F, 0x7F, 0x7F, 0x7F];

#[cfg(test)]
mod tests {
    use super::*;

    /// Both sample values from the published documentation, in both
    /// directions. These are the only ground truth available without a
    /// Serato install, so they are pinned rather than paraphrased.
    #[test]
    fn the_documented_colour_sample_round_trips() {
        let packed = [0x06, 0x30, 0x00, 0x00];
        assert_eq!(decode(packed), [0xCC, 0x00, 0x00]);
        assert_eq!(encode([0xCC, 0x00, 0x00]), packed);
    }

    #[test]
    fn the_documented_position_sample_round_trips() {
        let packed = [0x00, 0x0D, 0x2A, 0x58];
        assert_eq!(decode_u24(packed), 218_456);
        assert_eq!(encode_u24(218_456), packed);
    }

    #[test]
    fn the_documented_white_footer_round_trips() {
        let packed = [0x07, 0x7F, 0x7F, 0x7F];
        assert_eq!(decode(packed), [0xFF, 0xFF, 0xFF]);
        assert_eq!(encode([0xFF, 0xFF, 0xFF]), packed);
    }

    /// Every stored byte must have its top bit clear, or the block is not
    /// 7-bit clean and Serato reads garbage from the first value that sets it.
    #[test]
    fn no_encoded_byte_ever_sets_its_top_bit() {
        for value in (0u32..0x00FF_FFFF).step_by(1021) {
            for byte in encode_u24(value) {
                assert_eq!(byte & 0x80, 0, "value {value} produced a high bit");
            }
        }
    }

    #[test]
    fn every_value_survives_the_round_trip() {
        for value in (0u32..0x00FF_FFFF).step_by(1021) {
            assert_eq!(decode_u24(encode_u24(value)), value);
        }
    }

    /// Four and a half hours in, the three bytes are full. Saturating keeps a
    /// cue at the end of the file; wrapping would move it to the beginning.
    #[test]
    fn an_impossible_position_saturates_rather_than_wrapping() {
        assert_eq!(decode_u24(encode_u24(0x0100_0000)), 0x00FF_FFFF);
    }
}
