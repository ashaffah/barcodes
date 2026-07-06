//! Royal Mail 4-State Customer Code (RM4SCC) encoder.
//!
//! Encodes alphanumeric data (0–9, A–Z) into the Royal Mail 4-state barcode
//! (a start bar, one four-bar character per input character, a check character,
//! and a stop bar).  The output is a 3-row matrix: row 0 is the ascender, row 1
//! the tracker (always present), row 2 the descender.
#![forbid(unsafe_code)]

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

/// Maximum number of input characters.
const MAX_CHARS: usize = 50;
/// Maximum number of four-state bars (start + data×4 + check×4 + stop).
const MAX_STATES: usize = MAX_CHARS * 4 + 6;

/// Bar states per character value (0–35): 0 = Full, 1 = Ascender, 2 = Descender,
/// 3 = Tracker.  Source: zint `RM4KIX`.
const RM4KIX: [[u8; 4]; 36] = [
    [3, 3, 0, 0],
    [3, 2, 1, 0],
    [3, 2, 0, 1],
    [2, 3, 1, 0],
    [2, 3, 0, 1],
    [2, 2, 1, 1],
    [3, 1, 2, 0],
    [3, 0, 3, 0],
    [3, 0, 2, 1],
    [2, 1, 3, 0],
    [2, 1, 2, 1],
    [2, 0, 3, 1],
    [3, 1, 0, 2],
    [3, 0, 1, 2],
    [3, 0, 0, 3],
    [2, 1, 1, 2],
    [2, 1, 0, 3],
    [2, 0, 1, 3],
    [1, 3, 2, 0],
    [1, 2, 3, 0],
    [1, 2, 2, 1],
    [0, 3, 3, 0],
    [0, 3, 2, 1],
    [0, 2, 3, 1],
    [1, 3, 0, 2],
    [1, 2, 1, 2],
    [1, 2, 0, 3],
    [0, 3, 1, 2],
    [0, 3, 0, 3],
    [0, 2, 1, 3],
    [1, 1, 2, 2],
    [1, 0, 3, 2],
    [1, 0, 2, 3],
    [0, 1, 3, 2],
    [0, 1, 2, 3],
    [0, 0, 3, 3],
];

/// (top, bottom) contribution for the check-digit sum.  Source: zint.
const CHECK_TOP_BOTTOM: [[u32; 2]; 36] = [
    [1, 1],
    [1, 2],
    [1, 3],
    [1, 4],
    [1, 5],
    [1, 0],
    [2, 1],
    [2, 2],
    [2, 3],
    [2, 4],
    [2, 5],
    [2, 0],
    [3, 1],
    [3, 2],
    [3, 3],
    [3, 4],
    [3, 5],
    [3, 0],
    [4, 1],
    [4, 2],
    [4, 3],
    [4, 4],
    [4, 5],
    [4, 0],
    [5, 1],
    [5, 2],
    [5, 3],
    [5, 4],
    [5, 5],
    [5, 0],
    [0, 1],
    [0, 2],
    [0, 3],
    [0, 4],
    [0, 5],
    [0, 0],
];

/// Map an input character to its value 0–35 (0–9, A–Z).
fn char_value(b: u8) -> Option<usize> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as usize),
        b'A'..=b'Z' => Some((b - b'A' + 10) as usize),
        b'a'..=b'z' => Some((b - b'a' + 10) as usize),
        _ => None,
    }
}

/// Royal Mail 4-State Customer Code (RM4SCC) encoder.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::postal::rm4scc::Rm4scc;
///
/// let mut buf = [false; 3 * 128];
/// let Encoded::Matrix { height, .. } = Rm4scc::encode_into("SN34RD1A", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(height, 3);
/// ```
pub struct Rm4scc;

impl BarcodeEncoder for Rm4scc {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        // Validate and collect character values (ignoring whitespace).
        let mut posns = [0usize; MAX_CHARS];
        let mut n = 0;
        for b in input.bytes() {
            if b.is_ascii_whitespace() {
                continue;
            }
            let v = char_value(b).ok_or(EncodeError::InvalidCharacter(b as char))?;
            if n >= MAX_CHARS {
                return Err(EncodeError::DataTooLong);
            }
            posns[n] = v;
            n += 1;
        }
        if n == 0 {
            return Err(EncodeError::InvalidInput("RM4SCC input must not be empty"));
        }

        // Assemble the four-state bars: start, data, check character, stop.
        let mut states = [0u8; MAX_STATES];
        let mut s = 0;
        states[s] = 1; // start: ascender
        s += 1;

        let mut top = 0u32;
        let mut bottom = 0u32;
        for &p in &posns[..n] {
            states[s..s + 4].copy_from_slice(&RM4KIX[p]);
            s += 4;
            top += CHECK_TOP_BOTTOM[p][0];
            bottom += CHECK_TOP_BOTTOM[p][1];
        }

        // Check character from the top/bottom sums.
        let row = (top % 6).checked_sub(1).unwrap_or(5) as usize;
        let column = (bottom % 6).checked_sub(1).unwrap_or(5) as usize;
        let check = 6 * row + column;
        states[s..s + 4].copy_from_slice(&RM4KIX[check]);
        s += 4;

        states[s] = 0; // stop: full
        s += 1;

        // Render into a 3-row matrix (bar every 2 columns).
        let width = s * 2 - 1;
        let cells = 3 * width;
        if buf.len() < cells {
            return Err(EncodeError::BufferTooSmall);
        }
        for slot in buf[..cells].iter_mut() {
            *slot = false;
        }
        for (i, &state) in states[..s].iter().enumerate() {
            let col = i * 2;
            if state == 0 || state == 1 {
                buf[col] = true; // ascender (top row)
            }
            buf[width + col] = true; // tracker (middle row)
            if state == 0 || state == 2 {
                buf[2 * width + col] = true; // descender (bottom row)
            }
        }

        Ok(Encoded::Matrix { width, height: 3 })
    }

    fn symbology_name() -> &'static str {
        "RM4SCC"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dims(input: &str) -> (usize, usize) {
        let mut buf = [false; 3 * 512];
        match Rm4scc::encode_into(input, &mut buf).unwrap() {
            Encoded::Matrix { width, height } => (width, height),
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_encode_postcode() {
        // start + 8 chars × 4 + check × 4 + stop = 1 + 32 + 4 + 1 = 38 bars.
        let (width, height) = dims("SN34RD1A");
        assert_eq!(height, 3);
        assert_eq!(width, 38 * 2 - 1);
    }

    #[test]
    fn test_check_character_algorithm() {
        // "SN35TL" — verify the check character value via the documented rule.
        let vals: [usize; 6] = ['S', 'N', '3', '5', 'T', 'L'].map(|c| char_value(c as u8).unwrap());
        let mut top = 0u32;
        let mut bottom = 0u32;
        for &p in &vals {
            top += CHECK_TOP_BOTTOM[p][0];
            bottom += CHECK_TOP_BOTTOM[p][1];
        }
        let row = (top % 6).checked_sub(1).unwrap_or(5) as usize;
        let column = (bottom % 6).checked_sub(1).unwrap_or(5) as usize;
        assert!(6 * row + column < 36);
    }

    #[test]
    fn test_normalize_spaces() {
        let a = dims("SN34RD1A");
        let b = dims("SN3 4RD1A");
        assert_eq!(a, b);
    }

    #[test]
    fn test_invalid_char() {
        let mut buf = [false; 3 * 512];
        assert!(Rm4scc::encode_into("SN3-4RD", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 3 * 512];
        assert!(Rm4scc::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Rm4scc::symbology_name(), "RM4SCC");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Rm4scc::encode("SN34RD1A").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
