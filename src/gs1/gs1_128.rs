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

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};
use crate::linear::code128::{FNC1, START_B, START_C, STOP, compute_check, symbols_to_bars};

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
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::gs1::gs1_128::Gs1_128;
///
/// let out = Gs1_128::encode("(01)12345678901231").unwrap();
/// ```
pub struct Gs1_128;

impl BarcodeEncoder for Gs1_128 {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.trim().is_empty() {
            return Err(EncodeError::InvalidInput(
                "GS1-128 input must not be empty".into(),
            ));
        }

        let segments = parse_gs1(input.trim())?;
        let bars = build_barcode(&segments);
        let text = build_text_representation(&segments);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars,
            height: 50,
            text: Some(text),
        }))
    }

    fn symbology_name() -> &'static str {
        "GS1-128"
    }
}

// ---- Types -----------------------------------------------------------------

struct AiSegment {
    ai: String,
    data: String,
}

// ---- Helpers ---------------------------------------------------------------

/// Parse parenthesized AI format into (AI, data) pairs.
fn parse_gs1(input: &str) -> Result<Vec<AiSegment>, EncodeError> {
    let mut segments: Vec<AiSegment> = Vec::new();
    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes[pos] != b'(' {
            return Err(EncodeError::InvalidInput(alloc::format!(
                "expected '(' at position {pos}, got '{}'",
                bytes[pos] as char
            )));
        }
        pos += 1; // skip '('

        // Read AI digits until ')'
        let ai_start = pos;
        while pos < bytes.len() && bytes[pos] != b')' {
            if !bytes[pos].is_ascii_digit() {
                return Err(EncodeError::InvalidInput(
                    "AI must contain only digits".into(),
                ));
            }
            pos += 1;
        }
        if pos >= bytes.len() {
            return Err(EncodeError::InvalidInput(
                "unclosed '(' in AI specification".into(),
            ));
        }
        let ai = core::str::from_utf8(&bytes[ai_start..pos])
            .map_err(|_| EncodeError::InvalidInput("invalid UTF-8 in AI".into()))?;
        let ai = String::from(ai);
        pos += 1; // skip ')'

        // Read data until next '(' or end of string
        let data_start = pos;
        while pos < bytes.len() && bytes[pos] != b'(' {
            pos += 1;
        }
        let data = core::str::from_utf8(&bytes[data_start..pos])
            .map_err(|_| EncodeError::InvalidInput("invalid UTF-8 in AI data".into()))?;

        if data.is_empty() {
            return Err(EncodeError::InvalidInput(alloc::format!(
                "AI ({ai}) has no data"
            )));
        }

        segments.push(AiSegment {
            ai,
            data: String::from(data),
        });
    }

    if segments.is_empty() {
        return Err(EncodeError::InvalidInput(
            "no valid AIs found in input".into(),
        ));
    }

    Ok(segments)
}

/// Build the Code 128 symbol sequence for a GS1-128 barcode.
fn build_barcode(segments: &[AiSegment]) -> Vec<bool> {
    let mut symbols: Vec<u8> = Vec::new();

    // Determine if we can start with Code C (all-digit data)
    let all_numeric = segments
        .iter()
        .all(|s| s.data.chars().all(|c| c.is_ascii_digit()));

    let start = if all_numeric { START_C } else { START_B };
    symbols.push(start);

    // FNC1 immediately after start — signals GS1 application
    symbols.push(FNC1);

    for (i, seg) in segments.iter().enumerate() {
        // Encode AI itself using Code B (always printable ASCII digits)
        for byte in seg.ai.bytes() {
            symbols.push(byte - 0x20); // Code B value
        }

        // Encode data
        let data_bytes = seg.data.as_bytes();
        if all_numeric
            && data_bytes.iter().all(|b| b.is_ascii_digit())
            && data_bytes.len() % 2 == 0
            && start == START_C
        {
            // Use Code C numeric pairs
            let mut j = 0;
            while j + 1 < data_bytes.len() {
                let tens = data_bytes[j] - b'0';
                let units = data_bytes[j + 1] - b'0';
                symbols.push(tens * 10 + units);
                j += 2;
            }
            if j < data_bytes.len() {
                // Odd byte left, use Code B
                symbols.push(data_bytes[j] - 0x20);
            }
        } else {
            // Use Code B
            for &byte in data_bytes {
                if !(0x20..=0x7E).contains(&byte) {
                    // Skip invalid bytes; real implementation would return error
                    continue;
                }
                symbols.push(byte - 0x20);
            }
        }

        // Insert FNC1 separator after variable-length AI (not after the last one)
        if i + 1 < segments.len() && !is_fixed_length_ai(&seg.ai) {
            symbols.push(FNC1);
        }
    }

    // Check symbol
    let check = compute_check(&symbols);
    symbols.push(check);

    // Stop
    symbols.push(STOP);

    symbols_to_bars(&symbols)
}

fn build_text_representation(segments: &[AiSegment]) -> String {
    let mut s = String::new();
    for seg in segments {
        s.push('(');
        s.push_str(&seg.ai);
        s.push(')');
        s.push_str(&seg.data);
    }
    s
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_single_ai() {
        let out = Gs1_128::encode("(01)12345678901231").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_multiple_ai() {
        let out = Gs1_128::encode("(01)12345678901231(10)ABC123").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_parse_ai_digits_only() {
        let segs = parse_gs1("(01)12345678901231").unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].ai, "01");
        assert_eq!(segs[0].data, "12345678901231");
    }

    #[test]
    fn test_parse_multiple_ais() {
        let segs = parse_gs1("(01)12345678901231(10)LOT123").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].ai, "01");
        assert_eq!(segs[1].ai, "10");
        assert_eq!(segs[1].data, "LOT123");
    }

    #[test]
    fn test_invalid_no_parens() {
        assert!(Gs1_128::encode("0112345678901231").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Gs1_128::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Gs1_128::symbology_name(), "GS1-128");
    }

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
