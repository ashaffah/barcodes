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

use super::ean13::{GUARD_CENTRE, GUARD_NORMAL, L_CODE, R_CODE, check_digit};
use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

/// UPC-A barcode encoder.
///
/// Accepts an 11-digit string (check digit computed automatically) or a
/// 12-digit string (check digit included and validated).
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::ean_upc::upca::UpcA;
///
/// let mut buf = [false; 128];
/// // 11 digits — check digit auto-computed
/// let Encoded::Linear { len, .. } = UpcA::encode_into("01234567890", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(len, 95);
/// ```
pub struct UpcA;

impl BarcodeEncoder for UpcA {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let digits = parse_and_validate(input)?;
        let len = encode_bars(&digits, buf)?;
        Ok(Encoded::Linear { len, height: 69 })
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
            "UPC-A input must contain digits only",
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
                return Err(EncodeError::InvalidInput("UPC-A check digit mismatch"));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "UPC-A input must be 11 or 12 digits",
        )),
    }
}

fn encode_bars(digits: &[u8; 12], buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut w = SliceWriter::new(buf);

    // Start guard: 101
    w.extend(GUARD_NORMAL.iter().copied())?;

    // Left 6 digits — all L-code
    for &d in &digits[0..6] {
        w.extend(L_CODE[d as usize].iter().copied())?;
    }

    // Centre guard: 01010
    w.extend(GUARD_CENTRE.iter().copied())?;

    // Right 6 digits — all R-code
    for &d in &digits[6..12] {
        w.extend(R_CODE[d as usize].iter().copied())?;
    }

    // End guard: 101
    w.extend(GUARD_NORMAL.iter().copied())?;

    Ok(w.len())
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

    fn bars<'a>(input: &str, buf: &'a mut [bool]) -> &'a [bool] {
        match UpcA::encode_into(input, buf).unwrap() {
            Encoded::Linear { len, .. } => &buf[..len],
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_12_digits() {
        let mut buf = [false; 128];
        assert_eq!(bars("012345678905", &mut buf).len(), 95);
    }

    #[test]
    fn test_encode_11_digits_auto_check() {
        let mut buf11 = [false; 128];
        let mut buf12 = [false; 128];
        assert_eq!(
            bars("01234567890", &mut buf11),
            bars("012345678905", &mut buf12)
        );
    }

    #[test]
    fn test_invalid_check_digit() {
        let mut buf = [false; 128];
        assert!(UpcA::encode_into("012345678900", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_characters() {
        let mut buf = [false; 128];
        assert!(UpcA::encode_into("01234567890X", &mut buf).is_err());
    }

    #[test]
    fn test_wrong_length() {
        let mut buf = [false; 128];
        assert!(UpcA::encode_into("01234", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(UpcA::symbology_name(), "UPC-A");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = UpcA::encode("012345678905").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("</svg>"));
    }
}
