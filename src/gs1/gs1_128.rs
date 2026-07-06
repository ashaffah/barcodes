//! GS1-128 barcode encoder.
//!
//! GS1-128 is a variant of Code 128 that uses the FNC1 character as the first
//! data character after the start code to signal GS1 application.
//!
//! Application Identifiers (AIs) are parsed from parenthesized input such as:
//! `"(01)12345678901231(10)ABC123"`
//!
//! Variable-length AIs are terminated by inserting an FNC1 separator.
//!
//! # Structure
//!
//! ```text
//! Start-B  FNC1  [AI data]  [FNC1 separator]  [AI data...]  Check  Stop
//! ```
#![forbid(unsafe_code)]

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};
use crate::linear::code128::{FNC1, MAX_SYMBOLS, START_B, STOP, compute_check, symbols_to_bars};

/// Maximum number of AI segments supported in a single symbol.
const MAX_SEGMENTS: usize = 32;

// ---- AI definitions --------------------------------------------------------

/// Returns `true` if the given AI has a fixed length (no FNC1 separator needed).
///
/// Based on GS1 General Specifications fixed-length AIs.
fn is_fixed_length_ai(ai: &str) -> bool {
    matches!(
        ai,
        "00" | "01"
            | "02"
            | "03"
            | "04"
            | "11"
            | "12"
            | "13"
            | "14"
            | "15"
            | "16"
            | "17"
            | "18"
            | "19"
            | "20"
            | "31"
            | "32"
            | "33"
            | "34"
            | "35"
            | "36"
            | "41"
    )
}

// ---- Public encoder --------------------------------------------------------

/// GS1-128 barcode encoder.
///
/// Accepts parenthesized AI format such as `"(01)12345678901231(10)ABC123"`.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::gs1::gs1_128::Gs1_128;
///
/// let mut buf = [false; 1024];
/// let Encoded::Linear { len, .. } = Gs1_128::encode_into("(01)12345678901231", &mut buf).unwrap()
/// else { unreachable!() };
/// let bars = &buf[..len];
/// ```
pub struct Gs1_128;

impl BarcodeEncoder for Gs1_128 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.trim().is_empty() {
            return Err(EncodeError::InvalidInput("GS1-128 input must not be empty"));
        }

        let mut segments = [AiSegment { ai: "", data: "" }; MAX_SEGMENTS];
        let count = parse_gs1(input.trim(), &mut segments)?;
        let len = build_barcode(&segments[..count], buf)?;

        Ok(Encoded::Linear { len, height: 50 })
    }

    fn symbology_name() -> &'static str {
        "GS1-128"
    }
}

// ---- Types -----------------------------------------------------------------

/// An (AI, data) pair borrowing slices of the input string — no allocation.
#[derive(Clone, Copy)]
struct AiSegment<'a> {
    ai: &'a str,
    data: &'a str,
}

// ---- Helpers ---------------------------------------------------------------

/// Parse parenthesized AI format into `out`, returning the number of segments.
fn parse_gs1<'a>(
    input: &'a str,
    out: &mut [AiSegment<'a>; MAX_SEGMENTS],
) -> Result<usize, EncodeError> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut count = 0;

    while pos < bytes.len() {
        if bytes[pos] != b'(' {
            return Err(EncodeError::InvalidInput("expected '(' at start of AI"));
        }
        pos += 1; // skip '('

        // Read AI digits until ')'
        let ai_start = pos;
        while pos < bytes.len() && bytes[pos] != b')' {
            if !bytes[pos].is_ascii_digit() {
                return Err(EncodeError::InvalidInput("AI must contain only digits"));
            }
            pos += 1;
        }
        if pos >= bytes.len() {
            return Err(EncodeError::InvalidInput(
                "unclosed '(' in AI specification",
            ));
        }
        let ai = &input[ai_start..pos];
        pos += 1; // skip ')'

        // Read data until next '(' or end of string
        let data_start = pos;
        while pos < bytes.len() && bytes[pos] != b'(' {
            pos += 1;
        }
        let data = &input[data_start..pos];

        if data.is_empty() {
            return Err(EncodeError::InvalidInput("AI has no data"));
        }

        if count >= MAX_SEGMENTS {
            return Err(EncodeError::DataTooLong);
        }
        out[count] = AiSegment { ai, data };
        count += 1;
    }

    if count == 0 {
        return Err(EncodeError::InvalidInput("no valid AIs found in input"));
    }

    Ok(count)
}

/// Build the Code 128 symbol sequence for a GS1-128 barcode, writing bars into `buf`.
///
/// The whole message is encoded in Code Set B with a leading FNC1 (the GS1
/// indicator) and FNC1 separators after variable-length AIs.  Code B encodes
/// every AI and data byte consistently, so the symbol decodes correctly (Code C
/// numeric compaction is intentionally not used to avoid mode-switch errors).
fn build_barcode(segments: &[AiSegment], buf: &mut [bool]) -> Result<usize, EncodeError> {
    let mut symbols = [0u8; MAX_SYMBOLS];
    let mut n = 0;
    macro_rules! push {
        ($v:expr) => {{
            if n >= MAX_SYMBOLS {
                return Err(EncodeError::DataTooLong);
            }
            symbols[n] = $v;
            n += 1;
        }};
    }

    // Start Code B, then FNC1 to mark the symbol as GS1.
    push!(START_B);
    push!(FNC1);

    for (i, seg) in segments.iter().enumerate() {
        // AI digits, then data — all as Code B values.
        for byte in seg.ai.bytes() {
            push!(byte - 0x20);
        }
        for &byte in seg.data.as_bytes() {
            if (0x20..=0x7E).contains(&byte) {
                push!(byte - 0x20);
            }
        }

        // FNC1 separator after a variable-length AI (not after the last one).
        if i + 1 < segments.len() && !is_fixed_length_ai(seg.ai) {
            push!(FNC1);
        }
    }

    // Check symbol, then stop.
    let check = compute_check(&symbols[..n]);
    push!(check);
    push!(STOP);

    symbols_to_bars(&symbols[..n], buf)
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_len(input: &str) -> usize {
        let mut buf = [false; 2048];
        match Gs1_128::encode_into(input, &mut buf).unwrap() {
            Encoded::Linear { len, .. } => len,
            _ => panic!("expected linear"),
        }
    }

    fn parse(input: &str) -> ([AiSegment<'_>; MAX_SEGMENTS], usize) {
        let mut segs = [AiSegment { ai: "", data: "" }; MAX_SEGMENTS];
        let n = parse_gs1(input, &mut segs).unwrap();
        (segs, n)
    }

    #[test]
    fn test_encode_single_ai() {
        assert!(encode_len("(01)12345678901231") > 0);
    }

    #[test]
    fn test_encode_multiple_ai() {
        assert!(encode_len("(01)12345678901231(10)ABC123") > 0);
    }

    #[test]
    fn test_parse_ai_digits_only() {
        let (segs, n) = parse("(01)12345678901231");
        assert_eq!(n, 1);
        assert_eq!(segs[0].ai, "01");
        assert_eq!(segs[0].data, "12345678901231");
    }

    #[test]
    fn test_parse_multiple_ais() {
        let (segs, n) = parse("(01)12345678901231(10)LOT123");
        assert_eq!(n, 2);
        assert_eq!(segs[0].ai, "01");
        assert_eq!(segs[1].ai, "10");
        assert_eq!(segs[1].data, "LOT123");
    }

    #[test]
    fn test_invalid_no_parens() {
        let mut buf = [false; 2048];
        assert!(Gs1_128::encode_into("0112345678901231", &mut buf).is_err());
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 2048];
        assert!(Gs1_128::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Gs1_128::symbology_name(), "GS1-128");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Gs1_128::encode("(01)12345678901231")
            .unwrap()
            .to_svg_string();
        assert!(svg.starts_with("<svg "));
    }

    #[test]
    fn test_fixed_length_ai() {
        assert!(is_fixed_length_ai("01"));
        assert!(is_fixed_length_ai("02"));
        assert!(!is_fixed_length_ai("10"));
        assert!(!is_fixed_length_ai("21"));
    }
}
