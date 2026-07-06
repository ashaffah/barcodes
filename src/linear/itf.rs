//! ITF (Interleaved 2 of 5) barcode encoder.
//!
//! ITF is a numeric-only barcode that encodes pairs of digits: the first digit
//! of each pair is encoded in the bars, and the second digit is encoded in the
//! spaces that separate those bars.
//!
//! # Structure
//!
//! ```text
//! start(NNNN) + digit-pairs + stop(WNN)
//! ```
//!
//! Wide = 3 modules, narrow = 1 module.
//! Input must have an even number of digits.
#![forbid(unsafe_code)]

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

// ---- Encoding table --------------------------------------------------------

/// ITF encoding patterns indexed by digit 0-9.
/// Each pattern is 5 booleans: false=narrow, true=wide.
/// Pattern: NNWWN, WNNNW, NWNNW, WWNNN, NNWNW, WNWNN, NWWNN, NNNWW, WNNWN, NWNWN
const ITF_TABLE: [[bool; 5]; 10] = [
    [false, false, true, true, false], // 0: NNWWN
    [true, false, false, false, true], // 1: WNNNW
    [false, true, false, false, true], // 2: NWNNW
    [true, true, false, false, false], // 3: WWNNN
    [false, false, true, false, true], // 4: NNWNW
    [true, false, true, false, false], // 5: WNWNN
    [false, true, true, false, false], // 6: NWWNN
    [false, false, false, true, true], // 7: NNNWW
    [true, false, false, true, false], // 8: WNNWN
    [false, true, false, true, false], // 9: NWNWN
];

// ---- Public encoder --------------------------------------------------------

/// ITF (Interleaved 2 of 5) barcode encoder.
///
/// Encodes even-length numeric strings.  If the input has an odd number of
/// digits, a leading zero is prepended automatically.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::linear::itf::Itf;
///
/// let mut buf = [false; 256];
/// let Encoded::Linear { len, .. } = Itf::encode_into("12345678", &mut buf).unwrap()
/// else { unreachable!() };
/// let bars = &buf[..len];
/// ```
pub struct Itf;

impl BarcodeEncoder for Itf {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(EncodeError::InvalidInput("ITF input must not be empty"));
        }
        if !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Err(EncodeError::InvalidInput(
                "ITF input must contain digits only",
            ));
        }

        // Pad to even length with a virtual leading zero if necessary — no
        // allocation, handled by an index offset in `encode_bars`.
        let pad = !trimmed.len().is_multiple_of(2);
        let len = encode_bars(trimmed.as_bytes(), pad, buf)?;

        Ok(Encoded::Linear { len, height: 50 })
    }

    fn symbology_name() -> &'static str {
        "ITF"
    }
}

// ---- Helpers ---------------------------------------------------------------

fn encode_bars(digits: &[u8], pad: bool, buf: &mut [bool]) -> Result<usize, EncodeError> {
    let pad = pad as usize;
    let total = digits.len() + pad;
    // Logical digit at position `i`, treating a leading pad zero if present.
    let digit = |i: usize| -> usize {
        if i < pad {
            0
        } else {
            (digits[i - pad] - b'0') as usize
        }
    };

    let mut w = SliceWriter::new(buf);

    // Start pattern: 4 narrow bars/spaces = NNNN = dark, light, dark, light
    w.push(true)?;
    w.push(false)?;
    w.push(true)?;
    w.push(false)?;

    // Encode pairs: first digit in bars, second in spaces.
    let mut i = 0;
    while i + 1 < total {
        let p1 = &ITF_TABLE[digit(i)];
        let p2 = &ITF_TABLE[digit(i + 1)];
        for j in 0..5 {
            w.push_run(true, if p1[j] { 3 } else { 1 })?; // bar
            w.push_run(false, if p2[j] { 3 } else { 1 })?; // space
        }
        i += 2;
    }

    // Stop pattern: WNN = wide-bar, narrow-space, narrow-bar
    w.push_run(true, 3)?; // wide bar
    w.push(false)?; // narrow space
    w.push(true)?; // narrow bar

    Ok(w.len())
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode<'a>(input: &str, buf: &'a mut [bool]) -> &'a [bool] {
        match Itf::encode_into(input, buf).unwrap() {
            Encoded::Linear { len, .. } => &buf[..len],
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_even_digits() {
        let mut buf = [false; 512];
        assert!(!encode("12345678", &mut buf).is_empty());
    }

    #[test]
    fn test_encode_odd_digits_padded() {
        // Odd length gets a virtual leading zero; bars must match the padded form.
        let mut buf_odd = [false; 512];
        let mut buf_even = [false; 512];
        let odd = encode("1234567", &mut buf_odd);
        let even = encode("01234567", &mut buf_even);
        assert_eq!(odd, even);
    }

    #[test]
    fn test_invalid_characters() {
        let mut buf = [false; 512];
        assert!(Itf::encode_into("1234A678", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 512];
        assert!(Itf::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 8];
        assert_eq!(
            Itf::encode_into("12345678", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Itf::symbology_name(), "ITF");
    }

    #[test]
    fn test_bar_length_two_digits() {
        // Input "12": start(4) + pair(18) + stop(5) = 27 modules.
        let mut buf = [false; 512];
        assert_eq!(encode("12", &mut buf).len(), 27);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Itf::encode("1234").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
