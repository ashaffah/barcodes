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

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
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
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::ean_upc::ean8::Ean8;
///
/// let mut buf = [false; 128];
/// // 7 digits — check digit appended automatically
/// let Encoded::Linear { len, .. } = Ean8::encode_into("9638507", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(len, 67);
/// ```
pub struct Ean8;

impl BarcodeEncoder for Ean8 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let digits = parse_and_validate(input)?;
        let len = encode_bars(&digits, buf)?;
        Ok(Encoded::Linear { len, height: 69 })
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
            "EAN-8 input must contain digits only",
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
                return Err(EncodeError::InvalidInput("EAN-8 check digit mismatch"));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "EAN-8 input must be 7 or 8 digits",
        )),
    }
}

fn encode_bars(digits: &[u8; 8], buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut w = SliceWriter::new(buf);

    // Start guard
    w.extend(GUARD_NORMAL.iter().copied())?;

    // Left 4 digits — all L-code
    for &d in &digits[0..4] {
        w.extend(L_CODE[d as usize].iter().copied())?;
    }

    // Centre guard
    w.extend(GUARD_CENTRE.iter().copied())?;

    // Right 4 digits — all R-code
    for &d in &digits[4..8] {
        w.extend(R_CODE[d as usize].iter().copied())?;
    }

    // End guard
    w.extend(GUARD_NORMAL.iter().copied())?;

    Ok(w.len())
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

    fn bars<'a>(input: &str, buf: &'a mut [bool]) -> &'a [bool] {
        match Ean8::encode_into(input, buf).unwrap() {
            Encoded::Linear { len, .. } => &buf[..len],
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_8_digits() {
        let mut buf = [false; 128];
        assert_eq!(bars("96385074", &mut buf).len(), 67);
    }

    #[test]
    fn test_encode_7_digits_auto_check() {
        let mut buf7 = [false; 128];
        let mut buf8 = [false; 128];
        assert_eq!(bars("9638507", &mut buf7), bars("96385074", &mut buf8));
    }

    #[test]
    fn test_invalid_check_digit() {
        let mut buf = [false; 128];
        assert!(Ean8::encode_into("96385075", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_characters() {
        let mut buf = [false; 128];
        assert!(Ean8::encode_into("9638507X", &mut buf).is_err());
    }

    #[test]
    fn test_wrong_length() {
        let mut buf = [false; 128];
        assert!(Ean8::encode_into("963850", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Ean8::symbology_name(), "EAN-8");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Ean8::encode("96385074").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
