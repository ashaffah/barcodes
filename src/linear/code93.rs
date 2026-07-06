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

extern crate alloc;
use alloc::vec::Vec;

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

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
/// use barcodes::linear::code93::Code93;
///
/// let out = Code93::encode("CODE93").unwrap();
/// ```
pub struct Code93;

impl BarcodeEncoder for Code93 {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Code 93 input must not be empty".into(),
            ));
        }

        // Map each character to its value, rejecting anything unsupported.
        let mut values: Vec<usize> = Vec::with_capacity(input.len());
        for ch in input.chars() {
            match char_value(ch) {
                Some(v) => values.push(v),
                None => {
                    return Err(EncodeError::InvalidInput(alloc::format!(
                        "character '{ch}' is not valid in Code 93"
                    )));
                }
            }
        }

        // Append the two check characters (C then K).
        let c = check_value(&values, 20);
        values.push(c);
        let k = check_value(&values, 15);
        values.push(k);

        let bars = encode_bars(&values);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(input.into()),
        }))
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

/// Append a character's 9-module pattern to `bars`.
fn append_pattern(bars: &mut Vec<bool>, value: usize) {
    let pattern = CODE93_PATTERNS[value];
    for i in (0..9).rev() {
        bars.push((pattern >> i) & 1 == 1);
    }
}

fn encode_bars(values: &[usize]) -> Vec<bool> {
    // start + data/check chars + stop, 9 modules each, plus a termination bar.
    let mut bars: Vec<bool> = Vec::with_capacity((values.len() + 2) * 9 + 1);

    append_pattern(&mut bars, START_STOP);
    for &v in values {
        append_pattern(&mut bars, v);
    }
    append_pattern(&mut bars, START_STOP);

    // Final termination bar (single dark module).
    bars.push(true);

    bars
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_basic() {
        let out = Code93::encode("CODE93").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_special_chars() {
        let out = Code93::encode("HELLO WORLD").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_invalid_lowercase() {
        assert!(Code93::encode("hello").is_err());
    }

    #[test]
    fn test_invalid_symbol() {
        assert!(Code93::encode("ABC!DEF").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Code93::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Code93::symbology_name(), "Code 93");
    }

    #[test]
    fn test_bar_count() {
        // "A": start + A + C + K + stop = 5 chars * 9 modules + 1 termination.
        let out = Code93::encode("A").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => assert_eq!(lb.bars.len(), 5 * 9 + 1),
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_check_values_known() {
        // Worked example: "CODE93" -> C check char 'P' (25), K check char 'V' (31).
        let values: Vec<usize> = "CODE93".chars().map(|c| char_value(c).unwrap()).collect();
        let c = check_value(&values, 20);
        assert_eq!(c, char_value('P').unwrap());
        let mut with_c = values.clone();
        with_c.push(c);
        let k = check_value(&with_c, 15);
        assert_eq!(k, char_value('V').unwrap());
    }

    #[test]
    fn test_svg_output() {
        let svg = Code93::encode("TEST").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
