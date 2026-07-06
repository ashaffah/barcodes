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

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
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
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::ean_upc::ean13::Ean13;
///
/// let mut buf = [false; 128];
/// // 12 digits — check digit appended automatically
/// let Encoded::Linear { len, .. } = Ean13::encode_into("590123412345", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(len, 95);
/// ```
pub struct Ean13;

impl BarcodeEncoder for Ean13 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let digits = parse_and_validate(input)?;
        let len = encode_bars(&digits, buf)?;
        Ok(Encoded::Linear { len, height: 69 })
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
            "EAN-13 input must contain digits only",
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
                return Err(EncodeError::InvalidInput("EAN-13 check digit mismatch"));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "EAN-13 input must be 12 or 13 digits",
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

fn encode_bars(digits: &[u8; 13], buf: &mut [bool]) -> Result<usize, EncodeError> {
    let system = digits[0] as usize;
    let parity = PARITY[system];

    let mut w = SliceWriter::new(buf);

    // Start guard
    w.extend(GUARD_NORMAL.iter().copied())?;

    // Left 6 digits (digits[1]..=digits[6])
    for (pos, &d) in digits[1..=6].iter().enumerate() {
        let pattern = if parity[pos] {
            &G_CODE[d as usize]
        } else {
            &L_CODE[d as usize]
        };
        w.extend(pattern.iter().copied())?;
    }

    // Centre guard
    w.extend(GUARD_CENTRE.iter().copied())?;

    // Right 6 digits (digits[7]..=digits[12])
    for &d in &digits[7..=12] {
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
        // 5901234123457 — standard test barcode
        let digits: [u8; 12] = [5, 9, 0, 1, 2, 3, 4, 1, 2, 3, 4, 5];
        assert_eq!(check_digit(&digits), 7);
    }

    fn bars<'a>(input: &str, buf: &'a mut [bool]) -> &'a [bool] {
        match Ean13::encode_into(input, buf).unwrap() {
            Encoded::Linear { len, .. } => &buf[..len],
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_13_digits() {
        let mut buf = [false; 128];
        assert_eq!(bars("5901234123457", &mut buf).len(), 95);
    }

    #[test]
    fn test_encode_12_digits_auto_check() {
        let mut buf12 = [false; 128];
        let mut buf13 = [false; 128];
        assert_eq!(
            bars("590123412345", &mut buf12),
            bars("5901234123457", &mut buf13)
        );
    }

    #[test]
    fn test_invalid_check_digit() {
        let mut buf = [false; 128];
        assert!(Ean13::encode_into("5901234123458", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_characters() {
        let mut buf = [false; 128];
        assert!(Ean13::encode_into("590123412345X", &mut buf).is_err());
    }

    #[test]
    fn test_wrong_length() {
        let mut buf = [false; 128];
        assert!(Ean13::encode_into("590123", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 32];
        assert_eq!(
            Ean13::encode_into("5901234123457", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Ean13::symbology_name(), "EAN-13");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output_contains_svg_tag() {
        let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("</svg>"));
    }
}
