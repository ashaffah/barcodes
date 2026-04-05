//! ITF (Interleaved 2 of 5) barcode encoder.
//!
//! ITF is a numeric-only barcode that encodes pairs of digits: the first digit
//! of each pair is encoded in the bars, and the second digit is encoded in the
//! spaces that separate those bars.
//!
//! # Structure
//!
//! ```text
//! start(NNNN) + digit-pairs + stop(WNN)
//! ```
//!
//! Wide = 3 modules, narrow = 1 module.
//! Input must have an even number of digits.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{string::String, vec, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Encoding table --------------------------------------------------------

/// ITF encoding patterns indexed by digit 0-9.
/// Each pattern is 5 booleans: false=narrow, true=wide.
/// Pattern: NNWWN, WNNNW, NWNNW, WWNNN, NNWNW, WNWNN, NWWNN, NNNWW, WNNWN, NWNWN
const ITF_TABLE: [[bool; 5]; 10] = [
    [false, false, true, true, false], // 0: NNWWN
    [true, false, false, false, true], // 1: WNNNW
    [false, true, false, false, true], // 2: NWNNW
    [true, true, false, false, false], // 3: WWNNN
    [false, false, true, false, true], // 4: NNWNW
    [true, false, true, false, false], // 5: WNWNN
    [false, true, true, false, false], // 6: NWWNN
    [false, false, false, true, true], // 7: NNNWW
    [true, false, false, true, false], // 8: WNNWN
    [false, true, false, true, false], // 9: NWNWN
];

// ---- Public encoder --------------------------------------------------------

/// ITF (Interleaved 2 of 5) barcode encoder.
///
/// Encodes even-length numeric strings.  If the input has an odd number of
/// digits, a leading zero is prepended automatically.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::linear::itf::Itf;
///
/// let out = Itf::encode("12345678").unwrap();
/// ```
pub struct Itf;

impl BarcodeEncoder for Itf {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(EncodeError::InvalidInput(
                "ITF input must not be empty".into(),
            ));
        }
        if !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Err(EncodeError::InvalidInput(
                "ITF input must contain digits only".into(),
            ));
        }

        // Pad to even length with leading zero if necessary
        let padded: String = if !trimmed.len().is_multiple_of(2) {
            let mut s = String::with_capacity(trimmed.len() + 1);
            s.push('0');
            s.push_str(trimmed);
            s
        } else {
            trimmed.into()
        };

        let digits: Vec<u8> = padded.bytes().map(|b| b - b'0').collect();
        let bars = encode_bars(&digits);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(trimmed.into()),
        }))
    }

    fn symbology_name() -> &'static str {
        "ITF"
    }
}

// ---- Helpers ---------------------------------------------------------------

/// Push a single module (narrow or wide) of given polarity.
#[inline]
fn push_module(bars: &mut Vec<bool>, dark: bool, wide: bool) {
    let width = if wide { 3 } else { 1 };
    for _ in 0..width {
        bars.push(dark);
    }
}

fn encode_bars(digits: &[u8]) -> Vec<bool> {
    // Start pattern: 4 narrow bars/spaces = NNNN = dark, light, dark, light
    let mut bars: Vec<bool> = vec![true, false, true, false];

    // Encode pairs
    let mut i = 0;
    while i + 1 < digits.len() {
        let d1 = digits[i] as usize; // encoded in bars
        let d2 = digits[i + 1] as usize; // encoded in spaces

        let p1 = &ITF_TABLE[d1];
        let p2 = &ITF_TABLE[d2];

        // Interleave: for each of the 5 element positions,
        // emit bar from d1 then space from d2
        for j in 0..5 {
            push_module(&mut bars, true, p1[j]); // bar
            push_module(&mut bars, false, p2[j]); // space
        }

        i += 2;
    }

    // Stop pattern: WNN = wide-bar, narrow-space, narrow-bar
    bars.push(true); // wide bar (3 modules)
    bars.push(true);
    bars.push(true);
    bars.push(false); // narrow space
    bars.push(true); // narrow bar

    bars
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_even_digits() {
        let out = Itf::encode("12345678").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_odd_digits_padded() {
        // Should prepend a zero and succeed; bars should be identical
        let out_odd = Itf::encode("1234567").unwrap();
        let out_even = Itf::encode("01234567").unwrap();
        assert!(matches!(out_odd, BarcodeOutput::Linear(_)));
        // The bars should be the same (only text label differs)
        match (out_odd, out_even) {
            (BarcodeOutput::Linear(odd), BarcodeOutput::Linear(even)) => {
                assert_eq!(odd.bars, even.bars);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_invalid_characters() {
        assert!(Itf::encode("1234A678").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Itf::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Itf::symbology_name(), "ITF");
    }

    #[test]
    fn test_bar_length_two_digits() {
        // Input "12": 1 pair
        // Start: 4 modules
        // Pair: 5 interleaved elements, each 1 or 3 modules for bar + 1 or 3 for space
        // Stop: 3+1+1 = 5 modules
        let out = Itf::encode("12").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                // Digit 1 = WNNNW, digit 2 = NWNNW
                // Pair encoding: interleave bars of d1 with spaces of d2
                // Pos0: bar W(3) + space N(1) = 4
                // Pos1: bar N(1) + space W(3) = 4
                // Pos2: bar N(1) + space N(1) = 2
                // Pos3: bar N(1) + space N(1) = 2
                // Pos4: bar W(3) + space W(3) = 6
                // Total pair = 18
                // Start = 4, stop = 5
                // Total = 4 + 18 + 5 = 27
                assert_eq!(lb.bars.len(), 27);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_svg_output() {
        let svg = Itf::encode("1234").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
