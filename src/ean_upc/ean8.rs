//! EAN-8 barcode encoder.
//!
//! EAN-8 is a compressed version of EAN-13 designed for small packages.  It
//! encodes 8 digits (7 data digits + 1 check digit).
//!
//! # Structure
//!
//! ```text
//! [quiet] start-guard  L×4  centre-guard  R×4  end-guard  [quiet]
//! ```
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

use super::ean13::{GUARD_CENTRE, GUARD_NORMAL, L_CODE, R_CODE, check_digit};

/// EAN-8 barcode encoder.
///
/// Accepts either an 8-digit string (check digit included and validated) or a
/// 7-digit string (check digit is computed and appended automatically).
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::ean_upc::ean8::Ean8;
///
/// // 8 digits — check digit must be correct
/// let out = Ean8::encode("96385074").unwrap();
///
/// // 7 digits — check digit appended automatically
/// let out2 = Ean8::encode("9638507").unwrap();
/// ```
pub struct Ean8;

impl BarcodeEncoder for Ean8 {
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
        "EAN-8"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn parse_and_validate(input: &str) -> Result<[u8; 8], EncodeError> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "EAN-8 input must contain digits only".into(),
        ));
    }
    match trimmed.len() {
        7 => {
            let mut digits = [0u8; 8];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            digits[7] = check_digit(&digits[..7]);
            Ok(digits)
        }
        8 => {
            let mut digits = [0u8; 8];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            let expected = check_digit(&digits[..7]);
            if digits[7] != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "check digit mismatch: got {}, expected {expected}",
                    digits[7]
                )));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "EAN-8 input must be 7 or 8 digits".into(),
        )),
    }
}

fn encode_bars(digits: &[u8; 8]) -> Vec<bool> {
    let mut bars: Vec<bool> = Vec::with_capacity(67);

    // Start guard
    bars.extend_from_slice(&GUARD_NORMAL);

    // Left 4 digits — all L-code
    for &d in &digits[0..4] {
        bars.extend_from_slice(&L_CODE[d as usize]);
    }

    // Centre guard
    bars.extend_from_slice(&GUARD_CENTRE);

    // Right 4 digits — all R-code
    for &d in &digits[4..8] {
        bars.extend_from_slice(&R_CODE[d as usize]);
    }

    // End guard
    bars.extend_from_slice(&GUARD_NORMAL);

    bars
}

fn format_text(digits: &[u8; 8]) -> String {
    digits.iter().map(|d| (b'0' + d) as char).collect()
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_digit_known() {
        // 96385074
        let digits: [u8; 7] = [9, 6, 3, 8, 5, 0, 7];
        assert_eq!(check_digit(&digits), 4);
    }

    #[test]
    fn test_encode_8_digits() {
        let out = Ean8::encode("96385074").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                assert_eq!(lb.bars.len(), 67);
                assert_eq!(lb.text.as_deref(), Some("96385074"));
            }
            _ => panic!("expected linear barcode"),
        }
    }

    #[test]
    fn test_encode_7_digits_auto_check() {
        let out7 = Ean8::encode("9638507").unwrap();
        let out8 = Ean8::encode("96385074").unwrap();
        assert_eq!(out7, out8);
    }

    #[test]
    fn test_invalid_check_digit() {
        assert!(Ean8::encode("96385075").is_err());
    }

    #[test]
    fn test_invalid_characters() {
        assert!(Ean8::encode("9638507X").is_err());
    }

    #[test]
    fn test_wrong_length() {
        assert!(Ean8::encode("963850").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Ean8::symbology_name(), "EAN-8");
    }

    #[test]
    fn test_svg_output() {
        let svg = Ean8::encode("96385074").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
