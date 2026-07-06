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

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

// ---- Character encoding table (for reference) ------------------------------
// The CODE39_TABLE below is the authoritative encoding used in encode_bars.
// This alternative representation is kept for documentation purposes.

// ---- Encoding table --------------------------------------------------------
const CODE39_TABLE: &[(char, [bool; 9])] = &[
    (
        '0',
        [false, false, false, true, true, false, true, false, false],
    ), // 0
    (
        '1',
        [true, false, false, true, false, false, false, false, true],
    ), // 1
    (
        '2',
        [false, false, true, true, false, false, false, false, true],
    ), // 2
    (
        '3',
        [true, false, true, true, false, false, false, false, false],
    ), // 3
    (
        '4',
        [false, false, false, true, true, false, false, false, true],
    ), // 4
    (
        '5',
        [true, false, false, true, true, false, false, false, false],
    ), // 5
    (
        '6',
        [false, false, true, true, true, false, false, false, false],
    ), // 6
    (
        '7',
        [false, false, false, true, false, false, true, false, true],
    ), // 7
    (
        '8',
        [true, false, false, true, false, false, true, false, false],
    ), // 8
    (
        '9',
        [false, false, true, true, false, false, true, false, false],
    ), // 9
    (
        'A',
        [true, false, false, false, false, true, false, false, true],
    ), // A
    (
        'B',
        [false, false, true, false, false, true, false, false, true],
    ), // B
    (
        'C',
        [true, false, true, false, false, true, false, false, false],
    ), // C
    (
        'D',
        [false, false, false, false, true, true, false, false, true],
    ), // D
    (
        'E',
        [true, false, false, false, true, true, false, false, false],
    ), // E
    (
        'F',
        [false, false, true, false, true, true, false, false, false],
    ), // F
    (
        'G',
        [false, false, false, false, false, true, true, false, true],
    ), // G
    (
        'H',
        [true, false, false, false, false, true, true, false, false],
    ), // H
    (
        'I',
        [false, false, true, false, false, true, true, false, false],
    ), // I
    (
        'J',
        [false, false, false, false, true, true, true, false, false],
    ), // J
    (
        'K',
        [true, false, false, false, false, false, false, true, true],
    ), // K
    (
        'L',
        [false, false, true, false, false, false, false, true, true],
    ), // L
    (
        'M',
        [true, false, true, false, false, false, false, true, false],
    ), // M
    (
        'N',
        [false, false, false, false, true, false, false, true, true],
    ), // N
    (
        'O',
        [true, false, false, false, true, false, false, true, false],
    ), // O
    (
        'P',
        [false, false, true, false, true, false, false, true, false],
    ), // P
    (
        'Q',
        [false, false, false, false, false, false, true, true, true],
    ), // Q
    (
        'R',
        [true, false, false, false, false, false, true, true, false],
    ), // R
    (
        'S',
        [false, false, true, false, false, false, true, true, false],
    ), // S
    (
        'T',
        [false, false, false, false, true, false, true, true, false],
    ), // T
    (
        'U',
        [true, true, false, false, false, false, false, false, true],
    ), // U
    (
        'V',
        [false, true, true, false, false, false, false, false, true],
    ), // V
    (
        'W',
        [true, true, true, false, false, false, false, false, false],
    ), // W
    (
        'X',
        [false, true, false, false, true, false, false, false, true],
    ), // X
    (
        'Y',
        [true, true, false, false, true, false, false, false, false],
    ), // Y
    (
        'Z',
        [false, true, true, false, true, false, false, false, false],
    ), // Z
    (
        '-',
        [false, true, false, false, false, false, true, false, true],
    ), // -
    (
        '.',
        [true, true, false, false, false, false, true, false, false],
    ), // .
    (
        ' ',
        [false, true, true, false, false, false, true, false, false],
    ), // space
    (
        '$',
        [false, true, false, true, false, true, false, false, false],
    ), // $
    (
        '/',
        [false, true, false, true, false, false, false, true, false],
    ), // /
    (
        '+',
        [false, true, false, false, false, true, false, true, false],
    ), // +
    (
        '%',
        [false, false, false, true, false, true, false, true, false],
    ), // %
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
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::linear::code39::Code39;
///
/// let mut buf = [false; 256];
/// let Encoded::Linear { len, .. } = Code39::encode_into("CODE39", &mut buf).unwrap()
/// else { unreachable!() };
/// let bars = &buf[..len];
/// ```
pub struct Code39;

impl BarcodeEncoder for Code39 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput("Code 39 input must not be empty"));
        }

        // Validate all characters
        for ch in input.chars() {
            if lookup_pattern(ch).is_none() {
                return Err(EncodeError::InvalidCharacter(ch));
            }
        }

        let len = encode_bars(input, buf)?;
        Ok(Encoded::Linear { len, height: 50 })
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
fn append_char(w: &mut SliceWriter, pattern: &[bool; 9]) -> Result<(), EncodeError> {
    for (i, &wide) in pattern.iter().enumerate() {
        let is_bar = i % 2 == 0; // even indices are bars
        let width = if wide { 3 } else { 1 };
        w.push_run(is_bar, width)?; // dark for bars, light for spaces
    }
    Ok(())
}

fn encode_bars(input: &str, buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut w = SliceWriter::new(buf);

    let star = lookup_pattern('*').expect("star pattern must exist");

    // Start character
    append_char(&mut w, star)?;

    for ch in input.chars() {
        // Inter-character gap: 1 narrow space (light)
        w.push(false)?;

        let pattern = lookup_pattern(ch).expect("already validated");
        append_char(&mut w, pattern)?;
    }

    // Inter-character gap before stop
    w.push(false)?;

    // Stop character
    append_char(&mut w, star)?;

    Ok(w.len())
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_len(input: &str) -> usize {
        let mut buf = [false; 1024];
        match Code39::encode_into(input, &mut buf).unwrap() {
            Encoded::Linear { len, .. } => len,
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_basic() {
        assert!(encode_len("CODE39") > 0);
    }

    #[test]
    fn test_encode_digits() {
        assert!(encode_len("12345") > 0);
    }

    #[test]
    fn test_encode_special_chars() {
        assert!(encode_len("HELLO WORLD") > 0);
    }

    #[test]
    fn test_invalid_character() {
        // Lowercase is not valid in Code 39
        let mut buf = [false; 1024];
        assert!(Code39::encode_into("hello", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_char_symbol() {
        let mut buf = [false; 1024];
        assert!(Code39::encode_into("ABC!DEF", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 1024];
        assert!(Code39::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 8];
        assert_eq!(
            Code39::encode_into("A", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Code39::symbology_name(), "Code 39");
    }

    #[test]
    fn test_bar_count_single_char() {
        // Single char 'A': start(*) + gap + A + gap + stop(*)
        // * = N W N N W N W N N = 1+3+1+1+3+1+3+1+1 = 15
        // A = W N N N N N W N W = 3+1+1+1+1+1+3+1+3 = 15
        // Total = 15 + 1 + 15 + 1 + 15 = 47
        assert_eq!(encode_len("A"), 47);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Code39::encode("TEST").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
