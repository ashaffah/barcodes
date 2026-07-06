//! Codabar (NW-7 / USD-4) barcode encoder.
//!
//! Codabar is a discrete, variable-length symbology encoding digits 0–9 and
//! the special characters `-`, `$`, `:`, `/`, `.`, `+`.  Each character is
//! made of 7 elements (4 bars and 3 spaces) that are either narrow or wide,
//! separated from the next character by a narrow space.
//!
//! Every symbol is framed by a start and stop character chosen from `A`, `B`,
//! `C`, `D`.  This encoder frames the data with `A` (start) and `B` (stop).
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Encoding table --------------------------------------------------------

/// Element widths for a Codabar character: 7 elements in bar/space order
/// (bar, space, bar, space, bar, space, bar).  `true` = wide, `false` = narrow.
type Pattern = [bool; 7];

const N: bool = false;
const W: bool = true;

/// Data characters paired with their element-width patterns.
const CODABAR_CHARS: &[(char, Pattern)] = &[
    ('0', [N, N, N, N, N, W, W]),
    ('1', [N, N, N, N, W, W, N]),
    ('2', [N, N, N, W, N, N, W]),
    ('3', [W, W, N, N, N, N, N]),
    ('4', [N, N, W, N, N, W, N]),
    ('5', [W, N, N, N, N, W, N]),
    ('6', [N, W, N, N, N, N, W]),
    ('7', [N, W, N, N, W, N, N]),
    ('8', [N, W, W, N, N, N, N]),
    ('9', [W, N, N, W, N, N, N]),
    ('-', [N, N, N, W, W, N, N]),
    ('$', [N, N, W, W, N, N, N]),
    (':', [W, N, N, N, W, N, W]),
    ('/', [W, N, W, N, N, N, W]),
    ('.', [W, N, W, N, W, N, N]),
    ('+', [N, N, W, N, W, N, W]),
];

/// Start/stop characters paired with their element-width patterns.
const CODABAR_GUARDS: &[(char, Pattern)] = &[
    ('A', [N, N, W, W, N, W, N]),
    ('B', [N, N, N, W, N, W, W]),
    ('C', [N, W, N, W, N, N, W]),
    ('D', [N, N, N, W, W, W, N]),
];

/// Start guard used to frame the data.
const START: char = 'A';
/// Stop guard used to frame the data.
const STOP: char = 'B';

// ---- Public encoder --------------------------------------------------------

/// Codabar barcode encoder.
///
/// Encodes digits 0–9 and the characters `-`, `$`, `:`, `/`, `.`, `+`.  The
/// `A`/`B` start and stop guards are added automatically.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::linear::codabar::Codabar;
///
/// let out = Codabar::encode("1234567").unwrap();
/// ```
pub struct Codabar;

impl BarcodeEncoder for Codabar {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Codabar input must not be empty".into(),
            ));
        }

        for ch in input.chars() {
            if data_pattern(ch).is_none() {
                return Err(EncodeError::InvalidInput(alloc::format!(
                    "character '{ch}' is not valid in Codabar"
                )));
            }
        }

        let bars = encode_bars(input);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(input.into()),
        }))
    }

    fn symbology_name() -> &'static str {
        "Codabar"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn data_pattern(ch: char) -> Option<&'static Pattern> {
    CODABAR_CHARS.iter().find(|(c, _)| *c == ch).map(|(_, p)| p)
}

fn guard_pattern(ch: char) -> &'static Pattern {
    CODABAR_GUARDS
        .iter()
        .find(|(c, _)| *c == ch)
        .map(|(_, p)| p)
        .expect("guard character must exist")
}

/// Append a character's 7 elements to `bars`; narrow = 1 module, wide = 3.
fn append_pattern(bars: &mut Vec<bool>, pattern: &Pattern) {
    for (i, &wide) in pattern.iter().enumerate() {
        let dark = i % 2 == 0; // even elements are bars
        let width = if wide { 3 } else { 1 };
        for _ in 0..width {
            bars.push(dark);
        }
    }
}

fn encode_bars(input: &str) -> Vec<bool> {
    let mut bars: Vec<bool> = Vec::new();

    append_pattern(&mut bars, guard_pattern(START));
    for ch in input.chars() {
        bars.push(false); // narrow inter-character gap
        append_pattern(&mut bars, data_pattern(ch).expect("already validated"));
    }
    bars.push(false); // gap before stop guard
    append_pattern(&mut bars, guard_pattern(STOP));

    bars
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_digits() {
        let out = Codabar::encode("1234567").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_special_chars() {
        let out = Codabar::encode("12-34$56").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_invalid_letter() {
        assert!(Codabar::encode("12A34").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Codabar::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Codabar::symbology_name(), "Codabar");
    }

    #[test]
    fn test_bar_count_single_char() {
        // start(A) + gap + '0' + gap + stop(B).
        // Each 7-element char = (7 - wide) narrow*1 + wide*3 modules.
        // A has 3 wide -> 4 + 9 = 13; '0' has 2 wide -> 5 + 6 = 11;
        // B has 3 wide -> 13. Plus two 1-module gaps.
        let out = Codabar::encode("0").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => assert_eq!(lb.bars.len(), 13 + 1 + 11 + 1 + 13),
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_svg_output() {
        let svg = Codabar::encode("123").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
