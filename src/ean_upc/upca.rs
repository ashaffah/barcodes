//! UPC-A barcode encoder.
//!
//! UPC-A encodes 12 digits (11 data digits + 1 check digit). It is the standard
//! retail barcode used in the United States and Canada.
//!
//! # Structure
//!
//! ```text
//! [quiet] start-guard  L×6  centre-guard  R×6  end-guard  [quiet]
//! ```
//!
//! All six left-hand digits use L-code; all six right-hand digits use R-code.
//! The check digit uses the same weighted-sum algorithm as EAN-13.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use super::ean13::{GUARD_CENTRE, GUARD_NORMAL, L_CODE, R_CODE, check_digit};
use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

/// UPC-A barcode encoder.
///
/// Accepts an 11-digit string (check digit computed automatically) or a
/// 12-digit string (check digit included and validated).
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::ean_upc::upca::UpcA;
///
/// // 12 digits — check digit validated
/// let out = UpcA::encode("012345678905").unwrap();
///
/// // 11 digits — check digit auto-computed
/// let out2 = UpcA::encode("01234567890").unwrap();
/// ```
pub struct UpcA;

impl BarcodeEncoder for UpcA {
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
        "UPC-A"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn parse_and_validate(input: &str) -> Result<[u8; 12], EncodeError> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "UPC-A input must contain digits only".into(),
        ));
    }
    match trimmed.len() {
        11 => {
            let mut digits = [0u8; 12];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            digits[11] = check_digit(&digits[..11]);
            Ok(digits)
        }
        12 => {
            let mut digits = [0u8; 12];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            let expected = check_digit(&digits[..11]);
            if digits[11] != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "check digit mismatch: got {}, expected {expected}",
                    digits[11]
                )));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "UPC-A input must be 11 or 12 digits".into(),
        )),
    }
}

fn encode_bars(digits: &[u8; 12]) -> Vec<bool> {
    let mut bars: Vec<bool> = Vec::with_capacity(95);

    // Start guard: 101
    bars.extend_from_slice(&GUARD_NORMAL);

    // Left 6 digits — all L-code
    for &d in &digits[0..6] {
        bars.extend_from_slice(&L_CODE[d as usize]);
    }

    // Centre guard: 01010
    bars.extend_from_slice(&GUARD_CENTRE);

    // Right 6 digits — all R-code
    for &d in &digits[6..12] {
        bars.extend_from_slice(&R_CODE[d as usize]);
    }

    // End guard: 101
    bars.extend_from_slice(&GUARD_NORMAL);

    bars
}

fn format_text(digits: &[u8; 12]) -> String {
    digits.iter().map(|d| (b'0' + d) as char).collect()
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_digit_known() {
        // 012345678905 — first digit is number system digit
        let digits: [u8; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        assert_eq!(check_digit(&digits), 5);
    }

    #[test]
    fn test_encode_12_digits() {
        let out = UpcA::encode("012345678905").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                assert_eq!(lb.bars.len(), 95);
                assert_eq!(lb.text.as_deref(), Some("012345678905"));
            }
            _ => panic!("expected linear barcode"),
        }
    }

    #[test]
    fn test_encode_11_digits_auto_check() {
        let out11 = UpcA::encode("01234567890").unwrap();
        let out12 = UpcA::encode("012345678905").unwrap();
        assert_eq!(out11, out12);
    }

    #[test]
    fn test_invalid_check_digit() {
        assert!(UpcA::encode("012345678900").is_err());
    }

    #[test]
    fn test_invalid_characters() {
        assert!(UpcA::encode("01234567890X").is_err());
    }

    #[test]
    fn test_wrong_length() {
        assert!(UpcA::encode("01234").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(UpcA::symbology_name(), "UPC-A");
    }

    #[test]
    fn test_svg_output() {
        let svg = UpcA::encode("012345678905").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("</svg>"));
    }
}
