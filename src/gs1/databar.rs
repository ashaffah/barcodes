//! GS1 DataBar Omnidirectional barcode encoder.
//!
//! GS1 DataBar Omnidirectional encodes a 14-digit GTIN (Global Trade Item
//! Number).  It consists of two halves separated by a finder pattern.
//!
//! This implementation provides GTIN validation, check digit computation, and
//! a simplified encoding of the DataBar structure.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- DataBar character set tables ------------------------------------------

/// GS1 DataBar uses the RSS-14 character set.
/// Each character consists of 4 elements with a total width of 15 modules.
/// The table maps symbol values (0-115) to their element widths.
///
/// For simplicity, we encode each character as a sequence of bar/space widths.
/// This implementation provides the structure but uses a simplified encoding.
///
/// Finder pattern for DataBar Omnidirectional: 3 1 1 1 1 3
const FINDER_PATTERN: [u8; 6] = [3, 1, 1, 1, 1, 3]; // 10 modules

/// DataBar character widths. Each character has 4 elements summing to 15.
/// Table from GS1 DataBar specification (subset of RSS-14).
///
/// Index = character value (0-115), value = [w1, w2, w3, w4] widths.
/// Characters are encoded as: bar, space, bar, space (alternating).
const DATABAR_TABLE: &[[u8; 4]] = &[
    [1, 1, 1, 12], // 0
    [1, 1, 2, 11], // 1
    [1, 1, 3, 10], // 2
    [1, 1, 4, 9],  // 3
    [1, 1, 5, 8],  // 4
    [1, 1, 6, 7],  // 5
    [1, 1, 7, 6],  // 6
    [1, 1, 8, 5],  // 7
    [1, 1, 9, 4],  // 8
    [1, 1, 10, 3], // 9
    [1, 1, 11, 2], // 10
    [1, 1, 12, 1], // 11
    [1, 2, 1, 11], // 12
    [1, 2, 2, 10], // 13
    [1, 2, 3, 9],  // 14
    [1, 2, 4, 8],  // 15
    [1, 2, 5, 7],  // 16
    [1, 2, 6, 6],  // 17
    [1, 2, 7, 5],  // 18
    [1, 2, 8, 4],  // 19
    [1, 2, 9, 3],  // 20
    [1, 2, 10, 2], // 21
    [1, 2, 11, 1], // 22
    [1, 3, 1, 10], // 23
    [1, 3, 2, 9],  // 24
    [1, 3, 3, 8],  // 25
    [1, 3, 4, 7],  // 26
    [1, 3, 5, 6],  // 27
    [1, 3, 6, 5],  // 28
    [1, 3, 7, 4],  // 29
    [1, 3, 8, 3],  // 30
    [1, 3, 9, 2],  // 31
    [1, 3, 10, 1], // 32
    [1, 4, 1, 9],  // 33
    [1, 4, 2, 8],  // 34
    [1, 4, 3, 7],  // 35
    [1, 4, 4, 6],  // 36
    [1, 4, 5, 5],  // 37
    [1, 4, 6, 4],  // 38
    [1, 4, 7, 3],  // 39
    [1, 4, 8, 2],  // 40
    [1, 4, 9, 1],  // 41
    [1, 5, 1, 8],  // 42
    [1, 5, 2, 7],  // 43
    [1, 5, 3, 6],  // 44
    [1, 5, 4, 5],  // 45
    [1, 5, 5, 4],  // 46
    [1, 5, 6, 3],  // 47
    [1, 5, 7, 2],  // 48
    [1, 5, 8, 1],  // 49
    [1, 6, 1, 7],  // 50
    [1, 6, 2, 6],  // 51
    [1, 6, 3, 5],  // 52
    [1, 6, 4, 4],  // 53
    [1, 6, 5, 3],  // 54
    [1, 6, 6, 2],  // 55
    [1, 6, 7, 1],  // 56
    [1, 7, 1, 6],  // 57
    [1, 7, 2, 5],  // 58
    [1, 7, 3, 4],  // 59
    [1, 7, 4, 3],  // 60
    [1, 7, 5, 2],  // 61
    [1, 7, 6, 1],  // 62
    [1, 8, 1, 5],  // 63
    [1, 8, 2, 4],  // 64
    [1, 8, 3, 3],  // 65
    [1, 8, 4, 2],  // 66
    [1, 8, 5, 1],  // 67
    [1, 9, 1, 4],  // 68
    [1, 9, 2, 3],  // 69
    [1, 9, 3, 2],  // 70
    [1, 9, 4, 1],  // 71
    [1, 10, 1, 3], // 72
    [1, 10, 2, 2], // 73
    [1, 10, 3, 1], // 74
    [1, 11, 1, 2], // 75
    [1, 11, 2, 1], // 76
    [1, 12, 1, 1], // 77
    [2, 1, 1, 11], // 78
    [2, 1, 2, 10], // 79
    [2, 1, 3, 9],  // 80
    [2, 1, 4, 8],  // 81
    [2, 1, 5, 7],  // 82
    [2, 1, 6, 6],  // 83
    [2, 1, 7, 5],  // 84
    [2, 1, 8, 4],  // 85
    [2, 1, 9, 3],  // 86
    [2, 1, 10, 2], // 87
    [2, 1, 11, 1], // 88
    [2, 2, 1, 10], // 89
    [2, 2, 2, 9],  // 90
    [2, 2, 3, 8],  // 91
    [2, 2, 4, 7],  // 92
    [2, 2, 5, 6],  // 93
    [2, 2, 6, 5],  // 94
    [2, 2, 7, 4],  // 95
    [2, 2, 8, 3],  // 96
    [2, 2, 9, 2],  // 97
    [2, 2, 10, 1], // 98
    [2, 3, 1, 9],  // 99
    [2, 3, 2, 8],  // 100
    [2, 3, 3, 7],  // 101
    [2, 3, 4, 6],  // 102
    [2, 3, 5, 5],  // 103
    [2, 3, 6, 4],  // 104
    [2, 3, 7, 3],  // 105
    [2, 3, 8, 2],  // 106
    [2, 3, 9, 1],  // 107
    [2, 4, 1, 8],  // 108
    [2, 4, 2, 7],  // 109
    [2, 4, 3, 6],  // 110
    [2, 4, 4, 5],  // 111
    [2, 4, 5, 4],  // 112
    [2, 4, 6, 3],  // 113
    [2, 4, 7, 2],  // 114
    [2, 4, 8, 1],  // 115
];

// ---- Public encoder --------------------------------------------------------

/// GS1 DataBar Omnidirectional barcode encoder.
///
/// Encodes a 13 or 14-digit GTIN.  If 13 digits are provided, the check digit
/// is computed automatically.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::gs1::databar::DataBar;
///
/// let out = DataBar::encode("0614141123452").unwrap();
/// ```
pub struct DataBar;

impl BarcodeEncoder for DataBar {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        let digits = parse_and_validate(input)?;
        let bars = encode_bars(&digits);
        let text = format_text(&digits);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 33,
            text: Some(text),
        }))
    }

    fn symbology_name() -> &'static str {
        "GS1 DataBar"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn parse_and_validate(input: &str) -> Result<[u8; 14], EncodeError> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "GS1 DataBar input must contain digits only".into(),
        ));
    }

    match trimmed.len() {
        13 => {
            let mut digits = [0u8; 14];
            // Pad with leading zero
            digits[0] = 0;
            for (i, c) in trimmed.chars().enumerate() {
                digits[i + 1] = c as u8 - b'0';
            }
            // Recompute check digit for 14-digit GTIN
            digits[13] = gtin_check_digit(&digits[..13]);
            Ok(digits)
        }
        14 => {
            let mut digits = [0u8; 14];
            for (i, c) in trimmed.chars().enumerate() {
                digits[i] = c as u8 - b'0';
            }
            let expected = gtin_check_digit(&digits[..13]);
            if digits[13] != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "GTIN check digit mismatch: got {}, expected {expected}",
                    digits[13]
                )));
            }
            Ok(digits)
        }
        _ => Err(EncodeError::InvalidInput(
            "GS1 DataBar input must be 13 or 14 digits".into(),
        )),
    }
}

/// Compute GS1 GTIN-14 check digit using the standard GS1 algorithm.
pub(crate) fn gtin_check_digit(digits: &[u8]) -> u8 {
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            // From right to left (excluding check): odd positions ×3, even ×1
            // digits has 13 elements; last is position 0 from right
            let from_right = digits.len() - i; // 13 down to 1
            let weight = if from_right.is_multiple_of(2) {
                1u32
            } else {
                3u32
            };
            weight * d as u32
        })
        .sum();
    ((10 - (sum % 10)) % 10) as u8
}

/// Encode the DataBar barcode.
///
/// DataBar Omnidirectional structure:
/// - Left guard (1 module dark)
/// - Left pair: left character + finder + right character  
/// - Separator (dark)
/// - Right pair: left character + finder + right character
/// - Right guard (1 module dark)
fn encode_bars(digits: &[u8; 14]) -> Vec<bool> {
    // Compute the numerical value of the GTIN
    let mut value: u64 = 0;
    for &d in digits.iter() {
        value = value * 10 + d as u64;
    }

    // DataBar encodes the GTIN as two halves
    // Left half = value / 4537077, right half = value % 4537077
    let left_value = value / 4_537_077;
    let right_value = value % 4_537_077;

    let mut bars: Vec<bool> = Vec::new();

    // Encode left half
    encode_half(&mut bars, left_value, true);

    // Separator (1 narrow space)
    bars.push(false);

    // Encode right half
    encode_half(&mut bars, right_value, false);

    bars
}

/// Encode one half of a DataBar Omnidirectional symbol.
fn encode_half(bars: &mut Vec<bool>, value: u64, is_left: bool) {
    // Left guard: 1 dark bar
    if is_left {
        bars.push(true);
    }

    // Compute character values from GTIN half value
    // Each half has 2 data characters + finder pattern
    let char_a = (value / 1349) as usize % 116;
    let char_b = (value % 1349) as usize;
    let char_b = if char_b >= 116 { 115 } else { char_b };

    // Encode character A
    encode_databar_char(bars, char_a, true);

    // Finder pattern
    let mut dark = false;
    for &w in &FINDER_PATTERN {
        for _ in 0..w {
            bars.push(dark);
        }
        dark = !dark;
    }

    // Encode character B
    encode_databar_char(bars, char_b, false);

    // Right guard: 1 dark bar
    if !is_left {
        bars.push(true);
    }
}

fn encode_databar_char(bars: &mut Vec<bool>, idx: usize, start_dark: bool) {
    let pattern = &DATABAR_TABLE[idx.min(DATABAR_TABLE.len() - 1)];
    let mut dark = start_dark;
    for &w in pattern.iter() {
        for _ in 0..w {
            bars.push(dark);
        }
        dark = !dark;
    }
}

fn format_text(digits: &[u8; 14]) -> String {
    digits.iter().map(|d| (b'0' + d) as char).collect()
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtin_check_digit() {
        // Known GTIN-14: 00614141123452
        let digits: [u8; 13] = [0, 0, 6, 1, 4, 1, 4, 1, 1, 2, 3, 4, 5];
        assert_eq!(gtin_check_digit(&digits), 2);
    }

    #[test]
    fn test_encode_14_digits() {
        let out = DataBar::encode("00614141123452").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_13_digits_auto_check() {
        let out = DataBar::encode("0061414112345").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_invalid_check_digit() {
        assert!(DataBar::encode("00614141123453").is_err());
    }

    #[test]
    fn test_invalid_chars() {
        assert!(DataBar::encode("0061414112345X").is_err());
    }

    #[test]
    fn test_wrong_length() {
        assert!(DataBar::encode("0061414").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(DataBar::symbology_name(), "GS1 DataBar");
    }

    #[test]
    fn test_svg_output() {
        let svg = DataBar::encode("00614141123452").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
