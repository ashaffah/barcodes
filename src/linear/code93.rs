//! Code 93 barcode encoder.
//!
//! Code 93 is a variable-length, continuous symbology supporting digits 0–9,
//! uppercase letters A–Z, space, and the special characters `-`, `.`, `$`,
//! `/`, `+`, `%` (43 data characters in total).
//!
//! Each character is represented by 9 modules composed of 3 bars and 3 spaces.
//! The symbol is framed by the `*` start/stop character and terminated by a
//! single final bar module.  Two modulo-47 check characters (C and K) are
//! appended automatically after the data.
#![forbid(unsafe_code)]

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

/// Maximum number of data characters supported in a single symbol.
const MAX_DATA: usize = 256;

// ---- Encoding table --------------------------------------------------------

/// The 43 encodable data characters, indexed by their Code 93 value (0–42).
const CODE93_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";

/// Module patterns indexed by character value.
///
/// Indices 0–42 map to [`CODE93_CHARS`], indices 43–46 are the four control
/// characters (reachable as check-character values), and index 47 is the
/// `*` start/stop character.  Each pattern is a 9-bit module map where the
/// most-significant bit is the leftmost module: `1` = dark, `0` = light.
const CODE93_PATTERNS: [u16; 48] = [
    0b100010100, // 0
    0b101001000, // 1
    0b101000100, // 2
    0b101000010, // 3
    0b100101000, // 4
    0b100100100, // 5
    0b100100010, // 6
    0b101010000, // 7
    0b100010010, // 8
    0b100001010, // 9
    0b110101000, // A
    0b110100100, // B
    0b110100010, // C
    0b110010100, // D
    0b110010010, // E
    0b110001010, // F
    0b101101000, // G
    0b101100100, // H
    0b101100010, // I
    0b100110100, // J
    0b100011010, // K
    0b101011000, // L
    0b101001100, // M
    0b101000110, // N
    0b100101100, // O
    0b100010110, // P
    0b110110100, // Q
    0b110110010, // R
    0b110101100, // S
    0b110100110, // T
    0b110010110, // U
    0b110011010, // V
    0b101101100, // W
    0b101100110, // X
    0b100110110, // Y
    0b100111010, // Z
    0b100101110, // -
    0b111010100, // .
    0b111010010, // (space)
    0b111001010, // $
    0b101101110, // /
    0b101110110, // +
    0b110101110, // %
    0b100100110, // ($)  control
    0b111011010, // (%)  control
    0b111010110, // (/)  control
    0b100110010, // (+)  control
    0b101011110, // *    start/stop
];

/// Value of the `*` start/stop character.
const START_STOP: usize = 47;

// ---- Public encoder --------------------------------------------------------

/// Code 93 barcode encoder.
///
/// Encodes uppercase alphanumeric text and the special characters `-`, `.`,
/// `$`, `/`, `+`, `%`, and space.  The `*` start/stop delimiters and the two
/// modulo-47 check characters are added automatically.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::linear::code93::Code93;
///
/// let mut buf = [false; 256];
/// let Encoded::Linear { len, .. } = Code93::encode_into("CODE93", &mut buf).unwrap()
/// else { unreachable!() };
/// let bars = &buf[..len];
/// ```
pub struct Code93;

impl BarcodeEncoder for Code93 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput("Code 93 input must not be empty"));
        }

        // Map each character to its value in a fixed stack buffer (+2 for C, K).
        let mut values = [0usize; MAX_DATA + 2];
        let mut n = 0;
        for ch in input.chars() {
            let v = char_value(ch).ok_or(EncodeError::InvalidCharacter(ch))?;
            if n >= MAX_DATA {
                return Err(EncodeError::DataTooLong);
            }
            values[n] = v;
            n += 1;
        }

        // Append the two check characters (C then K).
        let c = check_value(&values[..n], 20);
        values[n] = c;
        n += 1;
        let k = check_value(&values[..n], 15);
        values[n] = k;
        n += 1;

        let len = encode_bars(&values[..n], buf)?;
        Ok(Encoded::Linear { len, height: 50 })
    }

    fn symbology_name() -> &'static str {
        "Code 93"
    }
}

// ---- Helpers ---------------------------------------------------------------

/// Return the Code 93 value (0–42) for a data character, if valid.
fn char_value(ch: char) -> Option<usize> {
    let byte = u8::try_from(ch as u32).ok()?;
    CODE93_CHARS.iter().position(|&c| c == byte)
}

/// Compute a modulo-47 weighted check value over `values`.
///
/// Weights run 1, 2, 3, … up to `max_weight` from the rightmost character,
/// then wrap back to 1.
fn check_value(values: &[usize], max_weight: u32) -> usize {
    let mut weight = 1u32;
    let mut sum = 0u32;
    for &v in values.iter().rev() {
        sum += weight * v as u32;
        weight = if weight == max_weight { 1 } else { weight + 1 };
    }
    (sum % 47) as usize
}

/// Append a character's 9-module pattern to the writer.
fn append_pattern(w: &mut SliceWriter, value: usize) -> Result<(), EncodeError> {
    let pattern = CODE93_PATTERNS[value];
    for i in (0..9).rev() {
        w.push((pattern >> i) & 1 == 1)?;
    }
    Ok(())
}

/// Write start + data/check chars + stop + termination bar; return module count.
fn encode_bars(values: &[usize], buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut w = SliceWriter::new(buf);

    append_pattern(&mut w, START_STOP)?;
    for &v in values {
        append_pattern(&mut w, v)?;
    }
    append_pattern(&mut w, START_STOP)?;

    // Final termination bar (single dark module).
    w.push(true)?;

    Ok(w.len())
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_len(input: &str) -> usize {
        let mut buf = [false; 4096];
        match Code93::encode_into(input, &mut buf).unwrap() {
            Encoded::Linear { len, .. } => len,
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_basic() {
        assert!(encode_len("CODE93") > 0);
    }

    #[test]
    fn test_encode_special_chars() {
        assert!(encode_len("HELLO WORLD") > 0);
    }

    #[test]
    fn test_invalid_lowercase() {
        let mut buf = [false; 4096];
        assert!(Code93::encode_into("hello", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_symbol() {
        let mut buf = [false; 4096];
        assert!(Code93::encode_into("ABC!DEF", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 4096];
        assert!(Code93::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 4];
        assert_eq!(
            Code93::encode_into("CODE93", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Code93::symbology_name(), "Code 93");
    }

    #[test]
    fn test_bar_count() {
        // "A": start + A + C + K + stop = 5 chars * 9 modules + 1 termination.
        assert_eq!(encode_len("A"), 5 * 9 + 1);
    }

    #[test]
    fn test_check_values_known() {
        // Worked example: "CODE93" -> C check char 'P' (25), K check char 'V' (31).
        let mut values = [0usize; 8];
        let mut n = 0;
        for ch in "CODE93".chars() {
            values[n] = char_value(ch).unwrap();
            n += 1;
        }
        let c = check_value(&values[..n], 20);
        assert_eq!(c, char_value('P').unwrap());
        values[n] = c;
        n += 1;
        let k = check_value(&values[..n], 15);
        assert_eq!(k, char_value('V').unwrap());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Code93::encode("TEST").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
