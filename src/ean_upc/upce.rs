//! UPC-E barcode encoder.
//!
//! UPC-E is a compressed form of UPC-A that suppresses zeros to fit small
//! packages.  It encodes 6 data digits with an implied number system digit
//! (0 or 1) and check digit.
//!
//! # Structure
//!
//! ```text
//! [quiet] start-guard(101)  6 data digits (L/G mix)  end-guard(010101)  [quiet]
//! ```
//!
//! The parity pattern of the 6 encoded digits is determined by the check digit.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use super::ean13::{GUARD_NORMAL, L_CODE, check_digit};
use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Encoding tables -------------------------------------------------------

/// G-code (even parity) patterns for UPC-E — same as EAN-13 G-code.
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

/// UPC-E end guard: 010101
const GUARD_END: [bool; 6] = [false, true, false, true, false, true];

/// Parity pattern for UPC-E indexed by check digit (0–9).
/// `false` = L-code, `true` = G-code for positions 0..6.
const UPCE_PARITY: [[bool; 6]; 10] = [
    [false, false, false, true, true, true], // 0
    [false, false, true, false, true, true], // 1
    [false, false, true, true, false, true], // 2
    [false, false, true, true, true, false], // 3
    [false, true, false, false, true, true], // 4
    [false, true, true, false, false, true], // 5
    [false, true, true, true, false, false], // 6
    [false, true, false, true, false, true], // 7
    [false, true, false, true, true, false], // 8
    [false, true, true, false, true, false], // 9
];

// ---- Public encoder --------------------------------------------------------

/// UPC-E barcode encoder.
///
/// Accepts 6-digit (data only), 7-digit (with check digit), or 8-digit input
/// (with number system 0/1 prefix and check digit).
///
/// The number system digit must be 0 or 1 when provided.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::ean_upc::upce::UpcE;
///
/// // 8 digits: number system + 6 data + check digit
/// let out = UpcE::encode("01234505").unwrap();
///
/// // 6 digits: data only, number system 0 assumed, check digit auto-computed
/// let out2 = UpcE::encode("123450").unwrap();
/// ```
pub struct UpcE;

impl BarcodeEncoder for UpcE {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        let (number_system, six_digits, check) = parse_and_validate(input)?;
        let bars = encode_bars(number_system, &six_digits, check);
        let text = format_text(number_system, &six_digits, check);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 69,
            text: Some(text),
        }))
    }

    fn symbology_name() -> &'static str {
        "UPC-E"
    }
}

// ---- Helpers ---------------------------------------------------------------

/// Parse and validate input, returning (number_system, [6 digits], check digit).
fn parse_and_validate(input: &str) -> Result<(u8, [u8; 6], u8), EncodeError> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "UPC-E input must contain digits only".into(),
        ));
    }

    match trimmed.len() {
        6 => {
            // Just the 6 data digits; assume number system 0
            let mut six = [0u8; 6];
            for (i, c) in trimmed.chars().enumerate() {
                six[i] = c as u8 - b'0';
            }
            let upca = expand_to_upca(0, &six);
            let check = check_digit(&upca[..11]);
            Ok((0, six, check))
        }
        7 => {
            // 6 data digits + check digit
            let mut buf = [0u8; 7];
            for (i, c) in trimmed.chars().enumerate() {
                buf[i] = c as u8 - b'0';
            }
            let six = [buf[0], buf[1], buf[2], buf[3], buf[4], buf[5]];
            let provided_check = buf[6];
            let upca = expand_to_upca(0, &six);
            let expected = check_digit(&upca[..11]);
            if provided_check != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "check digit mismatch: got {provided_check}, expected {expected}"
                )));
            }
            Ok((0, six, expected))
        }
        8 => {
            // Number system + 6 data digits + check digit
            let mut buf = [0u8; 8];
            for (i, c) in trimmed.chars().enumerate() {
                buf[i] = c as u8 - b'0';
            }
            let ns = buf[0];
            if ns > 1 {
                return Err(EncodeError::InvalidInput(
                    "UPC-E number system must be 0 or 1".into(),
                ));
            }
            let six = [buf[1], buf[2], buf[3], buf[4], buf[5], buf[6]];
            let provided_check = buf[7];
            let upca = expand_to_upca(ns, &six);
            let expected = check_digit(&upca[..11]);
            if provided_check != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "check digit mismatch: got {provided_check}, expected {expected}"
                )));
            }
            Ok((ns, six, expected))
        }
        _ => Err(EncodeError::InvalidInput(
            "UPC-E input must be 6, 7, or 8 digits".into(),
        )),
    }
}

/// Expand a UPC-E number to the equivalent 12-digit UPC-A number.
///
/// The expansion rules depend on the last digit of the 6-digit compressed form:
/// - 0, 1, 2: manufacturer = d[0..2] + last digit + "0000", product = d[2..5]
/// - 3: manufacturer = d[0..3] + "00000", product = d[3..5]  
/// - 4: manufacturer = d[0..4] + "00000", product = d[4..5] (padded)
/// - 5-9: manufacturer = d[0..5] + "00000", product = "0000" + d[5]
pub fn expand_to_upca(number_system: u8, six: &[u8; 6]) -> [u8; 12] {
    let mut upca = [0u8; 12];
    upca[0] = number_system;

    match six[5] {
        0..=2 => {
            // Manufacturer: d[0], d[1], six[5], 0, 0
            upca[1] = six[0];
            upca[2] = six[1];
            upca[3] = six[5];
            upca[4] = 0;
            upca[5] = 0;
            // Product: 0, 0, d[2], d[3], d[4]
            upca[6] = 0;
            upca[7] = 0;
            upca[8] = six[2];
            upca[9] = six[3];
            upca[10] = six[4];
        }
        3 => {
            // Manufacturer: d[0], d[1], d[2], 0, 0
            upca[1] = six[0];
            upca[2] = six[1];
            upca[3] = six[2];
            upca[4] = 0;
            upca[5] = 0;
            // Product: 0, 0, 0, d[3], d[4]
            upca[6] = 0;
            upca[7] = 0;
            upca[8] = 0;
            upca[9] = six[3];
            upca[10] = six[4];
        }
        4 => {
            // Manufacturer: d[0], d[1], d[2], d[3], 0
            upca[1] = six[0];
            upca[2] = six[1];
            upca[3] = six[2];
            upca[4] = six[3];
            upca[5] = 0;
            // Product: 0, 0, 0, 0, d[4]
            upca[6] = 0;
            upca[7] = 0;
            upca[8] = 0;
            upca[9] = 0;
            upca[10] = six[4];
        }
        d => {
            // 5-9: Manufacturer: d[0], d[1], d[2], d[3], d[4]
            upca[1] = six[0];
            upca[2] = six[1];
            upca[3] = six[2];
            upca[4] = six[3];
            upca[5] = six[4];
            // Product: 0, 0, 0, 0, d[5]
            upca[6] = 0;
            upca[7] = 0;
            upca[8] = 0;
            upca[9] = 0;
            upca[10] = d;
        }
    }

    upca
}

fn encode_bars(number_system: u8, six: &[u8; 6], check: u8) -> Vec<bool> {
    // Number system 1 uses inverted parity (all G becomes L and vice versa)
    let parity = UPCE_PARITY[check as usize];

    let mut bars: Vec<bool> = Vec::with_capacity(51);

    // Start guard: 101
    bars.extend_from_slice(&GUARD_NORMAL);

    // 6 data digits using L/G based on parity and number system
    for (pos, &d) in six.iter().enumerate() {
        // For number system 1, invert the parity
        let use_g = if number_system == 0 {
            parity[pos]
        } else {
            !parity[pos]
        };
        let pattern = if use_g {
            &G_CODE[d as usize]
        } else {
            &L_CODE[d as usize]
        };
        bars.extend_from_slice(pattern);
    }

    // End guard: 010101
    bars.extend_from_slice(&GUARD_END);

    bars
}

fn format_text(number_system: u8, six: &[u8; 6], check: u8) -> String {
    let mut s = String::with_capacity(8);
    s.push((b'0' + number_system) as char);
    for &d in six.iter() {
        s.push((b'0' + d) as char);
    }
    s.push((b'0' + check) as char);
    s
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_8_digits() {
        let out = UpcE::encode("01234505").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                // Start(3) + 6×7=42 + End(6) = 51
                assert_eq!(lb.bars.len(), 51);
            }
            _ => panic!("expected linear barcode"),
        }
    }

    #[test]
    fn test_encode_6_digits() {
        let out6 = UpcE::encode("123450").unwrap();
        assert!(matches!(out6, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_expand_to_upca_last0() {
        // Last digit 0: manufacturer = d[0]d[1]d[5]00, product = 00d[2]d[3]d[4]
        let six = [1u8, 2, 3, 4, 5, 0];
        let upca = expand_to_upca(0, &six);
        assert_eq!(upca[0], 0); // number system
        assert_eq!(upca[1], 1);
        assert_eq!(upca[2], 2);
        assert_eq!(upca[3], 0); // six[5]
        assert_eq!(upca[4], 0);
        assert_eq!(upca[5], 0);
        assert_eq!(upca[6], 0);
        assert_eq!(upca[7], 0);
        assert_eq!(upca[8], 3);
        assert_eq!(upca[9], 4);
        assert_eq!(upca[10], 5);
    }

    #[test]
    fn test_expand_to_upca_last3() {
        let six = [1u8, 2, 3, 4, 5, 3];
        let upca = expand_to_upca(0, &six);
        assert_eq!(upca[1], 1);
        assert_eq!(upca[2], 2);
        assert_eq!(upca[3], 3);
        assert_eq!(upca[4], 0);
        assert_eq!(upca[5], 0);
        assert_eq!(upca[9], 4);
        assert_eq!(upca[10], 5);
    }

    #[test]
    fn test_invalid_number_system() {
        assert!(UpcE::encode("21234505").is_err());
    }

    #[test]
    fn test_invalid_characters() {
        assert!(UpcE::encode("0123450X").is_err());
    }

    #[test]
    fn test_wrong_length() {
        assert!(UpcE::encode("12345").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(UpcE::symbology_name(), "UPC-E");
    }

    #[test]
    fn test_svg_output() {
        let svg = UpcE::encode("01234505").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
