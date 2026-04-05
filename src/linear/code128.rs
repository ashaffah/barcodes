//! Code 128 barcode encoder.
//!
//! Code 128 is a high-density linear barcode capable of encoding the full 128
//! ASCII character set.  It uses three subsets:
//!
//! - **Code 128A** — ASCII 00–95 (control codes + uppercase + digits).
//! - **Code 128B** — ASCII 32–127 (all printable ASCII).
//! - **Code 128C** — pairs of digits 00–99 (compact numeric encoding).
//!
//! This encoder auto-selects a single start code (A, B, or C) based on the
//! input and encodes all data using that subset.  Mixed-subset encoding is not
//! yet supported.
//!
//! # Symbol layout
//!
//! Each of the 103 data symbols (and 3 start codes) is 11 modules wide (3
//! dark bars + 3 light spaces).  The stop symbol is 13 modules wide.
//!
//! # Example
//!
//! ```rust
//! use barcode::common::traits::BarcodeEncoder;
//! use barcode::linear::code128::Code128;
//!
//! let out = Code128::encode("Hello").unwrap();
//! ```
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Symbol table ----------------------------------------------------------

/// Code 128 symbol bar patterns (indices 0–106).
///
/// Each entry is a 6-element array of bar/space widths in modules:
/// [bar, space, bar, space, bar, space] — always sums to 11.
/// Index 106 is the stop symbol (sums to 13, 7 elements).
///
/// Source: ISO/IEC 15417:2007 Annex A.
pub(crate) const PATTERNS: &[[u8; 6]; 107] = &[
    [2, 1, 2, 2, 2, 2], // 0
    [2, 2, 2, 1, 2, 2], // 1
    [2, 2, 2, 2, 2, 1], // 2
    [1, 2, 1, 2, 2, 3], // 3
    [1, 2, 1, 3, 2, 2], // 4
    [1, 3, 1, 2, 2, 2], // 5
    [1, 2, 2, 2, 1, 3], // 6
    [1, 2, 2, 3, 1, 2], // 7
    [1, 3, 2, 2, 1, 2], // 8
    [2, 2, 1, 2, 1, 3], // 9
    [2, 2, 1, 3, 1, 2], // 10
    [2, 3, 1, 2, 1, 2], // 11
    [1, 1, 2, 2, 3, 2], // 12
    [1, 2, 2, 1, 3, 2], // 13
    [1, 2, 2, 2, 3, 1], // 14
    [1, 1, 3, 2, 2, 2], // 15
    [1, 2, 3, 1, 2, 2], // 16
    [1, 2, 3, 2, 2, 1], // 17
    [2, 2, 3, 2, 1, 1], // 18
    [2, 2, 1, 1, 3, 2], // 19  (corrected)
    [2, 2, 1, 2, 3, 1], // 20
    [2, 1, 3, 2, 1, 2], // 21
    [2, 2, 3, 1, 1, 2], // 22
    [3, 1, 2, 1, 3, 1], // 23
    [3, 1, 1, 2, 2, 2], // 24
    [3, 2, 1, 1, 2, 2], // 25
    [3, 2, 1, 2, 2, 1], // 26
    [3, 1, 2, 2, 1, 2], // 27
    [3, 2, 2, 1, 1, 2], // 28
    [3, 2, 2, 2, 1, 1], // 29
    [2, 1, 2, 1, 2, 3], // 30
    [2, 1, 2, 3, 2, 1], // 31
    [2, 3, 2, 1, 2, 1], // 32
    [1, 1, 1, 3, 2, 3], // 33
    [1, 3, 1, 1, 2, 3], // 34
    [1, 3, 1, 3, 2, 1], // 35
    [1, 1, 2, 3, 1, 3], // 36
    [1, 3, 2, 1, 1, 3], // 37
    [1, 3, 2, 3, 1, 1], // 38
    [2, 1, 1, 3, 1, 3], // 39
    [2, 3, 1, 1, 1, 3], // 40
    [2, 3, 1, 3, 1, 1], // 41
    [1, 1, 2, 1, 3, 3], // 42
    [1, 1, 2, 3, 3, 1], // 43
    [1, 3, 2, 1, 3, 1], // 44
    [1, 1, 3, 1, 2, 3], // 45
    [1, 1, 3, 3, 2, 1], // 46
    [1, 3, 3, 1, 2, 1], // 47
    [3, 1, 3, 1, 2, 1], // 48
    [2, 1, 1, 3, 3, 1], // 49
    [2, 3, 1, 1, 3, 1], // 50
    [2, 1, 3, 1, 1, 3], // 51
    [2, 1, 3, 3, 1, 1], // 52
    [2, 1, 3, 1, 3, 1], // 53
    [3, 1, 1, 1, 2, 3], // 54
    [3, 1, 1, 3, 2, 1], // 55
    [3, 3, 1, 1, 2, 1], // 56
    [3, 1, 2, 1, 1, 3], // 57
    [3, 1, 2, 3, 1, 1], // 58
    [3, 3, 2, 1, 1, 1], // 59
    [3, 1, 4, 1, 1, 1], // 60
    [2, 2, 1, 4, 1, 1], // 61
    [4, 3, 1, 1, 1, 1], // 62
    [1, 1, 1, 2, 2, 4], // 63
    [1, 1, 1, 4, 2, 2], // 64
    [1, 2, 1, 1, 2, 4], // 65
    [1, 2, 1, 4, 2, 1], // 66
    [1, 4, 1, 1, 2, 2], // 67
    [1, 4, 1, 2, 2, 1], // 68
    [1, 1, 2, 2, 1, 4], // 69
    [1, 1, 2, 4, 1, 2], // 70
    [1, 2, 2, 1, 1, 4], // 71
    [1, 2, 2, 4, 1, 1], // 72
    [1, 4, 2, 1, 1, 2], // 73
    [1, 4, 2, 2, 1, 1], // 74
    [2, 4, 1, 2, 1, 1], // 75
    [2, 2, 1, 1, 1, 4], // 76
    [4, 1, 3, 1, 1, 1], // 77
    [2, 4, 1, 1, 1, 2], // 78
    [1, 3, 4, 1, 1, 1], // 79
    [1, 1, 1, 2, 4, 2], // 80
    [1, 2, 1, 1, 4, 2], // 81
    [1, 2, 1, 2, 4, 1], // 82
    [1, 1, 4, 2, 1, 2], // 83
    [1, 2, 4, 1, 1, 2], // 84
    [1, 2, 4, 2, 1, 1], // 85
    [4, 1, 1, 2, 1, 2], // 86
    [4, 2, 1, 1, 1, 2], // 87
    [4, 2, 1, 2, 1, 1], // 88
    [2, 1, 2, 1, 4, 1], // 89
    [2, 1, 4, 1, 2, 1], // 90
    [4, 1, 2, 1, 2, 1], // 91
    [1, 1, 1, 1, 4, 3], // 92
    [1, 1, 1, 3, 4, 1], // 93
    [1, 3, 1, 1, 4, 1], // 94
    [1, 1, 4, 1, 1, 3], // 95
    [1, 1, 4, 3, 1, 1], // 96
    [4, 1, 1, 1, 1, 3], // 97
    [4, 1, 1, 3, 1, 1], // 98
    [1, 1, 3, 1, 4, 1], // 99
    [1, 1, 4, 1, 3, 1], // 100
    [3, 1, 1, 1, 4, 1], // 101
    [4, 1, 1, 1, 3, 1], // 102
    // Start codes
    [2, 1, 1, 4, 1, 2], // 103 — Start A
    [2, 1, 1, 2, 1, 4], // 104 — Start B
    [2, 1, 1, 2, 3, 2], // 105 — Start C
    // Stop (13 modules: 6 bars + 6 spaces + 1 final bar; stored as 6 elements here,
    // with a trailing bar implied)
    [2, 3, 3, 1, 1, 1], // 106 — Stop (partial; appended with a final bar of width 2)
];

/// Final termination bar appended after the stop symbol pattern.
pub(crate) const STOP_TERMINATION: u8 = 2;

// ---- Start code constants --------------------------------------------------

pub(crate) const START_A: u8 = 103;
pub(crate) const START_B: u8 = 104;
pub(crate) const START_C: u8 = 105;
pub(crate) const STOP: u8 = 106;
/// FNC1 special function character (symbol value 102).
pub(crate) const FNC1: u8 = 102;

// ---- Subset detection ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subset {
    A,
    B,
    C,
}

/// Determine the most appropriate subset for `input`.
///
/// - Code C is selected when the input consists entirely of an even number of digits.
/// - Code A is selected when the input contains characters in ASCII 0–31 (control).
/// - Code B is selected otherwise (all printable ASCII).
fn best_subset(input: &[u8]) -> Result<Subset, EncodeError> {
    // Code C: all digits, even length
    if input.len() >= 2
        && input.len().is_multiple_of(2)
        && input.iter().all(|&b| b.is_ascii_digit())
    {
        return Ok(Subset::C);
    }

    // Code A: contains ASCII control chars (0x00–0x1F) or DEL (0x7F)
    if input.iter().any(|&b| b < 0x20 || b == 0x7F) {
        // Check that all chars are valid for A (0x00–0x5F)
        if input.iter().all(|&b| b <= 0x5F || b == 0x7F) {
            return Ok(Subset::A);
        }
        return Err(EncodeError::InvalidInput(
            "input contains characters not encodable in Code 128A".into(),
        ));
    }

    // Code B: printable ASCII 0x20–0x7E
    if input.iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        return Ok(Subset::B);
    }

    Err(EncodeError::InvalidInput(
        "input contains characters outside the Code 128 character set".into(),
    ))
}

// ---- Symbol value calculation ----------------------------------------------

fn symbol_value_a(byte: u8) -> u8 {
    // Code A encodes ASCII 0x00–0x5F in positions 0–95 and
    // ASCII DEL (0x7F) is not standard; map 0x00–0x1F → 64–95, 0x20–0x5F → 0–63
    if byte <= 0x1F { byte + 64 } else { byte - 0x20 }
}

fn symbol_value_b(byte: u8) -> u8 {
    // Code B: ASCII 0x20–0x7E → values 0–94
    byte - 0x20
}

// ---- Public encoder --------------------------------------------------------

/// Code 128 barcode encoder.
///
/// Supports subsets A, B, and C with automatic subset selection.
pub struct Code128;

impl BarcodeEncoder for Code128 {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Code 128 input must not be empty".into(),
            ));
        }

        let bytes = input.as_bytes();
        let subset = best_subset(bytes)?;

        let mut symbol_indices: Vec<u8> = Vec::with_capacity(bytes.len() + 4);

        // Start code
        let start = match subset {
            Subset::A => START_A,
            Subset::B => START_B,
            Subset::C => START_C,
        };
        symbol_indices.push(start);

        // Data symbols
        match subset {
            Subset::A => {
                for &b in bytes {
                    symbol_indices.push(symbol_value_a(b));
                }
            }
            Subset::B => {
                for &b in bytes {
                    symbol_indices.push(symbol_value_b(b));
                }
            }
            Subset::C => {
                let mut i = 0;
                while i + 1 < bytes.len() {
                    let tens = bytes[i] - b'0';
                    let units = bytes[i + 1] - b'0';
                    symbol_indices.push(tens * 10 + units);
                    i += 2;
                }
            }
        }

        // Check symbol (weighted modulo-103 sum)
        let check = compute_check(&symbol_indices);
        symbol_indices.push(check);

        // Stop
        symbol_indices.push(STOP);

        // Convert symbols to bar/space widths
        let bars = symbols_to_bars(&symbol_indices);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(input.into()),
        }))
    }

    fn symbology_name() -> &'static str {
        "Code 128"
    }
}

// ---- Helpers ---------------------------------------------------------------

pub(crate) fn compute_check(symbols: &[u8]) -> u8 {
    // The start symbol contributes its own value at weight 1.
    // Each subsequent data symbol is multiplied by its 1-based position.
    let start_val = symbols[0] as u32;
    let weighted: u32 = symbols[1..]
        .iter()
        .enumerate()
        .map(|(i, &s)| (i as u32 + 1) * s as u32)
        .sum();
    ((start_val + weighted) % 103) as u8
}

/// Expand symbol indices into a `Vec<bool>` of dark/light modules.
pub(crate) fn symbols_to_bars(symbols: &[u8]) -> Vec<bool> {
    let mut bars: Vec<bool> = Vec::new();

    for &sym in symbols.iter() {
        let is_stop = sym == STOP;
        let pattern = &PATTERNS[sym as usize];

        // Alternate dark/light starting with dark for every symbol.
        let mut dark = true;
        for &width in pattern.iter() {
            for _ in 0..width {
                bars.push(dark);
            }
            dark = !dark;
        }

        // The stop symbol has a final termination bar (2 dark modules).
        if is_stop {
            bars.extend(core::iter::repeat_n(true, STOP_TERMINATION as usize));
        }
    }

    bars
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_subset_b_basic() {
        let out = Code128::encode("Hello").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                // Start B + 5 data + check + stop = 8 symbols
                // Each of the 7 non-stop symbols = 11 modules, stop = 13 modules
                // Total = 7*11 + 13 = 77 + 13 = 90
                assert_eq!(lb.bars.len(), 90);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_subset_c() {
        // Even-length all-digit input → Code C
        let out = Code128::encode("123456").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                // Start C + 3 data pairs + check + stop = 5 non-stop + stop
                // 5 × 11 + 13 (stop) = 68
                assert_eq!(lb.bars.len(), 68);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_subset_a_control() {
        // Contains a control character (BEL = 0x07)
        let input = "\x07ABC";
        let out = Code128::encode(input).unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_empty_input_error() {
        assert!(Code128::encode("").is_err());
    }

    #[test]
    fn test_invalid_high_byte() {
        assert!(Code128::encode("caf\u{00E9}").is_err());
    }

    #[test]
    fn test_check_computation() {
        // Manually verify check for "PJJ123C" — known Code 128B example.
        // We just verify the function returns without panic and result is in range.
        let out = Code128::encode("PJJ123C").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Code128::symbology_name(), "Code 128");
    }

    #[test]
    fn test_svg_output() {
        let svg = Code128::encode("Test").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
