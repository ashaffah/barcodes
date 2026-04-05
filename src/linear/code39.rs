//! Code 39 barcode encoder.
//!
//! Code 39 is a variable-length, discrete barcode symbology supporting:
//! digits 0–9, uppercase letters A–Z, space, and the special characters
//! `-`, `.`, `$`, `/`, `+`, `%`.
//!
//! Each character is represented by 9 elements (5 bars and 4 spaces), of which
//! exactly 3 are wide (wide-to-narrow ratio = 3:1).  A narrow inter-character
//! gap separates consecutive characters.  The symbol begins and ends with the
//! `*` (asterisk) start/stop character.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Character encoding table (for reference) ------------------------------
// The CODE39_TABLE below is the authoritative encoding used in encode_bars.
// This alternative representation is kept for documentation purposes.

// ---- Encoding table --------------------------------------------------------
const CODE39_TABLE: &[(char, [bool; 9])] = &[
    // char  b0     s0     b1     s1     b2     s2     b3     s3     b4
    (
        '0',
        [false, false, false, true, true, false, true, false, false],
    ),
    (
        '1',
        [true, false, false, false, false, true, false, false, true],
    ),
    (
        '2',
        [false, false, true, false, false, true, false, false, true],
    ),
    (
        '3',
        [true, false, true, false, false, true, false, false, false],
    ),
    (
        '4',
        [false, false, false, true, false, true, false, false, true],
    ),
    (
        '5',
        [true, false, false, true, false, true, false, false, false],
    ),
    (
        '6',
        [false, false, true, true, false, true, false, false, false],
    ),
    (
        '7',
        [false, false, false, false, true, true, false, false, true],
    ),
    (
        '8',
        [true, false, false, false, true, true, false, false, false],
    ),
    (
        '9',
        [false, false, true, false, true, true, false, false, false],
    ),
    (
        'A',
        [true, false, false, false, false, false, true, false, true],
    ),
    (
        'B',
        [false, false, true, false, false, false, true, false, true],
    ),
    (
        'C',
        [true, false, true, false, false, false, true, false, false],
    ),
    (
        'D',
        [false, false, false, true, false, false, true, false, true],
    ),
    (
        'E',
        [true, false, false, true, false, false, true, false, false],
    ),
    (
        'F',
        [false, false, true, true, false, false, true, false, false],
    ),
    (
        'G',
        [false, false, false, false, true, false, true, false, true],
    ),
    (
        'H',
        [true, false, false, false, true, false, true, false, false],
    ),
    (
        'I',
        [false, false, true, false, true, false, true, false, false],
    ),
    (
        'J',
        [false, false, false, true, true, false, true, false, false],
    ),
    (
        'K',
        [true, false, false, false, false, false, false, true, true],
    ),
    (
        'L',
        [false, false, true, false, false, false, false, true, true],
    ),
    (
        'M',
        [true, false, true, false, false, false, false, true, false],
    ),
    (
        'N',
        [false, false, false, true, false, false, false, true, true],
    ),
    (
        'O',
        [true, false, false, true, false, false, false, true, false],
    ),
    (
        'P',
        [false, false, true, true, false, false, false, true, false],
    ),
    (
        'Q',
        [false, false, false, false, true, false, false, true, true],
    ),
    (
        'R',
        [true, false, false, false, true, false, false, true, false],
    ),
    (
        'S',
        [false, false, true, false, true, false, false, true, false],
    ),
    (
        'T',
        [false, false, false, true, true, false, false, true, false],
    ),
    (
        'U',
        [true, true, false, false, false, false, false, false, true],
    ),
    (
        'V',
        [false, true, true, false, false, false, false, false, true],
    ),
    (
        'W',
        [true, true, true, false, false, false, false, false, false],
    ),
    (
        'X',
        [false, true, false, true, false, false, false, false, true],
    ),
    (
        'Y',
        [true, true, false, true, false, false, false, false, false],
    ),
    (
        'Z',
        [false, true, true, true, false, false, false, false, false],
    ),
    (
        '-',
        [false, true, false, false, true, false, false, false, true],
    ),
    (
        '.',
        [true, true, false, false, true, false, false, false, false],
    ),
    (
        ' ',
        [false, true, true, false, true, false, false, false, false],
    ),
    (
        '$',
        [false, true, false, true, false, true, false, false, false],
    ),
    (
        '/',
        [false, true, false, false, false, true, false, true, false],
    ),
    (
        '+',
        [false, true, false, false, false, false, false, true, false],
    ),
    (
        '%',
        [false, false, false, true, false, true, false, true, false],
    ),
    (
        '*',
        [false, true, false, false, true, false, true, false, false],
    ), // start/stop
];

// ---- Public encoder --------------------------------------------------------

/// Code 39 barcode encoder.
///
/// Encodes uppercase alphanumeric text and the special characters `-`, `.`,
/// `$`, `/`, `+`, `%`, and space.  The start and stop `*` delimiters are
/// added automatically.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::linear::code39::Code39;
///
/// let out = Code39::encode("CODE39").unwrap();
/// ```
pub struct Code39;

impl BarcodeEncoder for Code39 {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Code 39 input must not be empty".into(),
            ));
        }

        // Validate all characters
        for ch in input.chars() {
            if lookup_pattern(ch).is_none() {
                return Err(EncodeError::InvalidInput(alloc::format!(
                    "character '{ch}' is not valid in Code 39"
                )));
            }
        }

        let bars = encode_bars(input);
        let text = input.into();

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(text),
        }))
    }

    fn symbology_name() -> &'static str {
        "Code 39"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn lookup_pattern(ch: char) -> Option<&'static [bool; 9]> {
    CODE39_TABLE.iter().find(|(c, _)| *c == ch).map(|(_, p)| p)
}

/// Encode a single character's 9-element pattern into bars.
///
/// narrow = 1 module, wide = 3 modules.
/// Elements alternate: bar, space, bar, space, …, bar (9 elements).
fn append_char(bars: &mut Vec<bool>, pattern: &[bool; 9]) {
    for (i, &wide) in pattern.iter().enumerate() {
        let is_bar = i % 2 == 0; // even indices are bars
        let width = if wide { 3 } else { 1 };
        let module = is_bar; // dark for bars, light for spaces
        for _ in 0..width {
            bars.push(module);
        }
    }
}

fn encode_bars(input: &str) -> Vec<bool> {
    // Estimate capacity: start + chars + stop + inter-char gaps
    // Each char: max 3+1+3+1+3+1+3+1+3 = 17 modules (all narrow = 9)
    // Typical: ~13 modules per char
    let mut bars: Vec<bool> = Vec::new();

    let star = lookup_pattern('*').expect("star pattern must exist");

    // Start character
    append_char(&mut bars, star);

    for ch in input.chars() {
        // Inter-character gap: 1 narrow space (light)
        bars.push(false);

        let pattern = lookup_pattern(ch).expect("already validated");
        append_char(&mut bars, pattern);
    }

    // Inter-character gap before stop
    bars.push(false);

    // Stop character
    append_char(&mut bars, star);

    bars
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_basic() {
        let out = Code39::encode("CODE39").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_digits() {
        let out = Code39::encode("12345").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_special_chars() {
        let out = Code39::encode("HELLO WORLD").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_invalid_character() {
        // Lowercase is not valid in Code 39
        assert!(Code39::encode("hello").is_err());
    }

    #[test]
    fn test_invalid_char_symbol() {
        assert!(Code39::encode("ABC!DEF").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Code39::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Code39::symbology_name(), "Code 39");
    }

    #[test]
    fn test_bar_count_single_char() {
        // Single char 'A': start(*) + gap + A + gap + stop(*)
        // * pattern: all narrow = 9 modules  (1+1+1+1+1+1+1+1+1 = 9... actually mix)
        // Let's just verify it produces output with reasonable length
        let out = Code39::encode("A").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                // Start * + gap(1) + A + gap(1) + Stop *
                // * = N W N N W N W N N = 1+3+1+1+3+1+3+1+1 = 15
                // A = W N N N N N W N W = 3+1+1+1+1+1+3+1+3 = 15
                // Total = 15 + 1 + 15 + 1 + 15 = 47
                assert_eq!(lb.bars.len(), 47);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_svg_output() {
        let svg = Code39::encode("TEST").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
