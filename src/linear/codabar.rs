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

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
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
/// use barcodes::common::types::Encoded;
/// use barcodes::linear::codabar::Codabar;
///
/// let mut buf = [false; 256];
/// let Encoded::Linear { len, .. } = Codabar::encode_into("1234567", &mut buf).unwrap()
/// else { unreachable!() };
/// let bars = &buf[..len];
/// ```
pub struct Codabar;

impl BarcodeEncoder for Codabar {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput("Codabar input must not be empty"));
        }

        for ch in input.chars() {
            if data_pattern(ch).is_none() {
                return Err(EncodeError::InvalidCharacter(ch));
            }
        }

        let len = encode_bars(input, buf)?;
        Ok(Encoded::Linear { len, height: 50 })
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

/// Append a character's 7 elements; narrow = 1 module, wide = 3.
fn append_pattern(w: &mut SliceWriter, pattern: &Pattern) -> Result<(), EncodeError> {
    for (i, &wide) in pattern.iter().enumerate() {
        let dark = i % 2 == 0; // even elements are bars
        let width = if wide { 3 } else { 1 };
        w.push_run(dark, width)?;
    }
    Ok(())
}

fn encode_bars(input: &str, buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut w = SliceWriter::new(buf);

    append_pattern(&mut w, guard_pattern(START))?;
    for ch in input.chars() {
        w.push(false)?; // narrow inter-character gap
        append_pattern(&mut w, data_pattern(ch).expect("already validated"))?;
    }
    w.push(false)?; // gap before stop guard
    append_pattern(&mut w, guard_pattern(STOP))?;

    Ok(w.len())
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_len(input: &str) -> usize {
        let mut buf = [false; 1024];
        match Codabar::encode_into(input, &mut buf).unwrap() {
            Encoded::Linear { len, .. } => len,
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_digits() {
        assert!(encode_len("1234567") > 0);
    }

    #[test]
    fn test_encode_special_chars() {
        assert!(encode_len("12-34$56") > 0);
    }

    #[test]
    fn test_invalid_letter() {
        let mut buf = [false; 1024];
        assert!(Codabar::encode_into("12A34", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 1024];
        assert!(Codabar::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 8];
        assert_eq!(
            Codabar::encode_into("1234567", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
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
        assert_eq!(encode_len("0"), 13 + 1 + 11 + 1 + 13);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Codabar::encode("123").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
