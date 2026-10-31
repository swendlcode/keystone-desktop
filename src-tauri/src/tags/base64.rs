//! Standard base64, encode and decode.
//!
//! Hand-rolled rather than pulled in as a crate: embedded cover art is the only
//! thing in Keystone that needs base64, it crosses the IPC boundary as a data
//! URL, and `ENGINE.md` §4 keeps the dependency table to the crates that were
//! actually argued for. Forty lines of table lookup is cheaper than a
//! dependency nobody reviewed.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

pub fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3F] as char
        } else {
            PAD as char
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3F] as char
        } else {
            PAD as char
        });
    }
    out
}

/// Rejects anything that is not well-formed base64 rather than guessing, since
/// the input is a cover image the user is about to have written into their
/// file. Whitespace is skipped: `Serato Markers2` and the FLAC base64 blobs
/// both wrap at 72 characters, so wrapped input is normal, not malformed.
pub fn decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(input.len() / 4 * 3);

    for c in input.bytes() {
        if c.is_ascii_whitespace() || c == PAD {
            continue;
        }
        let v = sextet(c).ok_or("not base64")?;
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
        // Drop what has been emitted. Without this the accumulator keeps
        // shifting left and overflows `u32` on any input past a few bytes,
        // which is a debug-build panic rather than a wrong answer.
        acc &= (1 << bits) - 1;
    }

    // The leftover bits must be zero padding, never data: anything else means
    // the string was truncated mid-byte.
    if acc != 0 {
        return Err("truncated base64");
    }
    Ok(out)
}

fn sextet(c: u8) -> Option<u8> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_4648_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), encoded, "encoding {plain:?}");
            assert_eq!(decode(encoded).unwrap(), plain.as_bytes(), "decoding {encoded:?}");
        }
    }

    #[test]
    fn round_trips_every_byte_value() {
        let all: Vec<u8> = (0..=255u8).collect();
        assert_eq!(decode(&encode(&all)).unwrap(), all);
    }

    #[test]
    fn line_wrapped_input_decodes() {
        // 72-character wrapping is what the FLAC and Serato blobs look like.
        assert_eq!(decode("Zm9v\nYmFy\r\n").unwrap(), b"foobar");
    }

    #[test]
    fn garbage_is_rejected_rather_than_silently_truncated() {
        assert!(decode("Zm9v!").is_err());
        assert!(decode("Zh").is_err());
    }
}
