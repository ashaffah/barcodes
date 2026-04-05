//! EAN-13 barcode encoder.
//!
//! EAN-13 encodes 13 digits (12 data digits + 1 check digit).  It is the most
//! widely used retail barcode standard worldwide.
//!
//! # Structure
//!
//! ```text
//! [quiet] start-guard  L/G×6  centre-guard  R×6  end-guard  [quiet]
//! ```
//!
//! The first digit (system digit) is not directly encoded as bars; instead it
//! selects the L/G parity pattern for the left-hand six digits.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Encoding tables -------------------------------------------------------

/// L-code (odd parity) patterns for digits 0–9.  7 modules each.
pub(crate) const L_CODE: [[bool; 7]; 10] = [
    [false, false, false, true, true, false, true],  // 0
    [false, false, true, true, false, false, true],  // 1
    [false, false, true, false, false, true, true],  // 2
    [false, true, true, true, true, false, true],    // 3
    [false, true, false, false, false, true, true],  // 4
    [false, true, true, false, false, false, true],  // 5
    [false, true, false, true, true, true, false],   // 6  (corrected EAN spec)
    [false, true, true, true, false, true, false],   // 7
    [false, true, true, false, true, true, false],   // 8  (corrected EAN spec)
    [false, false, false, true, false, true, false], // 9
];

/// G-code (even parity) patterns for digits 0–9.  7 modules each.
const G_CODE: [[bool; 7]; 10] = [
    [false, true, false, false, true, true, true],   // 0
    [false, true, true, false, false, true, true],   // 1
    [false, false, true, true, false, true, true],   // 2
    [false, true, false, false, false, false, true], // 3
    [false, false, true, true, true, false, true],   // 4
    [false, true, true, true, false, false, true],   // 5
    [false, false, false, false, true, false, true], // 6
    [false, false, true, false, false, false, true], // 7
    [false, false, false, true, false, false, true], // 8
    [false, false, true, false, true, true, true],   // 9
];

/// R-code (right-hand) patterns for digits 0–9.  7 modules each.
pub(crate) const R_CODE: [[bool; 7]; 10] = [
    [true, true, true, false, false, true, false],   // 0
    [true, true, false, false, true, true, false],   // 1
    [true, true, false, true, true, false, false],   // 2
    [true, false, false, false, false, true, false], // 3
    [true, false, true, true, true, false, false],   // 4
    [true, false, false, true, true, true, false],   // 5
    [true, false, true, false, false, false, false], // 6
    [true, false, false, false, true, false, false], // 7
    [true, false, false, true, false, false, false], // 8
    [true, true, true, false, true, false, false],   // 9
];

/// Parity selection for the left-hand 6 digits indexed by system digit 0–9.
/// `false` = L-code, `true` = G-code for positions 0..6 (left digits 1..6).
const PARITY: [[bool; 6]; 10] = [
    [false, false, false, false, false, false], // 0
    [false, false, true, false, true, true],    // 1
    [false, false, true, true, false, true],    // 2
    [false, false, true, true, true, false],    // 3
    [false, true, false, false, true, true],    // 4
    [false, true, true, false, false, true],    // 5
    [false, true, true, true, false, false],    // 6
    [false, true, false, true, false, true],    // 7
    [false, true, false, true, true, false],    // 8
    [false, true, true, false, true, false],    // 9
];

// ---- Guard patterns --------------------------------------------------------

/// Normal guard bar (start / end): 101
pub(crate) const GUARD_NORMAL: [bool; 3] = [true, false, true];
/// Centre guard bar: 01010
pub(crate) const GUARD_CENTRE: [bool; 5] = [false, true, false, true, false];

// ---- Public encoder --------------------------------------------------------

/// EAN-13 barcode encoder.
///
/// Accepts either a 13-digit string (check digit included and validated) or a
/// 12-digit string (check digit is computed and appended automatically).
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::ean_upc::ean13::Ean13;
///
/// // 13 digits — check digit must be correct
/// let out = Ean13::encode("5901234123457").unwrap();
///
/// // 12 digits — check digit appended automatically
/// let out2 = Ean13::encode("590123412345").unwrap();
/// ```
pub struct Ean13;

impl BarcodeEncoder for Ean13 {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        let digits = parse_and_validate(input)?;
        let bars = encode_bars(&digits);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 69,
            text: Some(format_text(&digits)),
        }))
    }

    fn symbology_name() -> &'static str {
        "EAN-13"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn parse_and_validate(input: &str) -> Result<[u8; 13], EncodeError> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "EAN-13 input must contain digits only".into(),
        ));
    }
    match trimmed.len() {
        12 => {
            let mut digits = [0u8; 13];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            digits[12] = check_digit(&digits[..12]);
            Ok(digits)
        }
        13 => {
            let mut digits = [0u8; 13];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            let expected = check_digit(&digits[..12]);
            if digits[12] != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "check digit mismatch: got {}, expected {expected}",
                    digits[12]
                )));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "EAN-13 input must be 12 or 13 digits".into(),
        )),
    }
}

/// Compute EAN-13 / EAN-8 check digit from a slice of digit values (without check).
pub(crate) fn check_digit(digits: &[u8]) -> u8 {
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let weight = if i % 2 == 0 { 1u32 } else { 3u32 };
            weight * d as u32
        })
        .sum();
    ((10 - (sum % 10)) % 10) as u8
}

fn encode_bars(digits: &[u8; 13]) -> Vec<bool> {
    let system = digits[0] as usize;
    let parity = PARITY[system];

    let mut bars: Vec<bool> = Vec::with_capacity(95);

    // Start guard
    bars.extend_from_slice(&GUARD_NORMAL);

    // Left 6 digits (digits[1]..=digits[6])
    for (pos, &d) in digits[1..=6].iter().enumerate() {
        let pattern = if parity[pos] {
            &G_CODE[d as usize]
        } else {
            &L_CODE[d as usize]
        };
        bars.extend_from_slice(pattern);
    }

    // Centre guard
    bars.extend_from_slice(&GUARD_CENTRE);

    // Right 6 digits (digits[7]..=digits[12])
    for &d in &digits[7..=12] {
        bars.extend_from_slice(&R_CODE[d as usize]);
    }

    // End guard
    bars.extend_from_slice(&GUARD_NORMAL);

    bars
}

fn format_text(digits: &[u8; 13]) -> String {
    digits.iter().map(|d| (b'0' + d) as char).collect()
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_digit_known() {
        // 5901234123457 — standard test barcode
        let digits: [u8; 12] = [5, 9, 0, 1, 2, 3, 4, 1, 2, 3, 4, 5];
        assert_eq!(check_digit(&digits), 7);
    }

    #[test]
    fn test_encode_13_digits() {
        let out = Ean13::encode("5901234123457").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                assert_eq!(lb.bars.len(), 95);
                assert_eq!(lb.text.as_deref(), Some("5901234123457"));
            }
            _ => panic!("expected linear barcode"),
        }
    }

    #[test]
    fn test_encode_12_digits_auto_check() {
        let out12 = Ean13::encode("590123412345").unwrap();
        let out13 = Ean13::encode("5901234123457").unwrap();
        assert_eq!(out12, out13);
    }

    #[test]
    fn test_invalid_check_digit() {
        assert!(Ean13::encode("5901234123458").is_err());
    }

    #[test]
    fn test_invalid_characters() {
        assert!(Ean13::encode("590123412345X").is_err());
    }

    #[test]
    fn test_wrong_length() {
        assert!(Ean13::encode("590123").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Ean13::symbology_name(), "EAN-13");
    }

    #[test]
    fn test_svg_output_contains_svg_tag() {
        let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("</svg>"));
    }
}
