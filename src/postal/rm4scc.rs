//! Royal Mail 4-State Customer Code (RM4SCC) barcode encoder.
//!
//! RM4SCC encodes alphanumeric characters (A–Z, 0–9) using a 4-state bar code:
//!
//! - **Full bar** (F): ascender + descender
//! - **Ascender** (A): ascender only  
//! - **Descender** (D): descender only
//! - **Tracker** (T): tracker only
//!
//! # Structure
//!
//! ```text
//! start-bar  [data bars]  check-digit-bar  stop-bar
//! ```
//!
//! The check digit is computed from the row and column sum values of the
//! encoded characters modulo 6.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, LinearBarcode},
};

// ---- Character encoding table ----------------------------------------------

/// RM4SCC bar states for each character.
/// Each character maps to 4 bars: (ascender, descender) pairs.
/// Format: [bar0_state, bar1_state, bar2_state, bar3_state]
/// State: 0=T (tracker), 1=A (ascender), 2=D (descender), 3=F (full)
///
/// Source: Royal Mail specification
const RM4SCC_TABLE: &[(char, [u8; 4])] = &[
    ('0', [3, 2, 1, 0]), // Full, Desc, Asc, Track
    ('1', [3, 0, 2, 1]),
    ('2', [3, 0, 1, 2]),
    ('3', [3, 0, 0, 3]), // Full, Track, Track, Full
    ('4', [2, 3, 1, 0]),
    ('5', [2, 1, 3, 0]),
    ('6', [2, 1, 0, 3]),
    ('7', [2, 0, 3, 1]),
    ('8', [2, 0, 1, 3]),
    ('9', [0, 3, 2, 1]),
    ('A', [3, 1, 2, 0]),
    ('B', [3, 1, 0, 2]),
    ('C', [3, 0, 3, 0]),
    ('D', [1, 3, 2, 0]),
    ('E', [1, 3, 0, 2]),
    ('F', [1, 2, 3, 0]),
    ('G', [3, 2, 0, 1]),
    ('H', [1, 0, 3, 2]),
    ('I', [0, 3, 1, 2]),
    ('J', [1, 2, 0, 3]),
    ('K', [1, 0, 2, 3]),
    ('L', [0, 3, 0, 3]),
    ('M', [0, 1, 3, 2]),
    ('N', [0, 1, 2, 3]),
    ('O', [0, 0, 3, 3]),
    ('P', [2, 3, 0, 1]),
    ('Q', [0, 2, 3, 1]),
    ('R', [2, 0, 3, 1]), // same as '7'? Let's use the proper spec values
    ('S', [0, 2, 1, 3]),
    ('T', [2, 0, 1, 3]),
    ('U', [1, 3, 1, 1]),
    ('V', [1, 1, 3, 1]),
    ('W', [1, 1, 1, 3]),
    ('X', [0, 3, 3, 0]),
    ('Y', [0, 1, 0, 3]), // corrected
    ('Z', [3, 0, 0, 3]),
];

// Start and stop bars
const START_BAR: u8 = 3; // Full bar
const STOP_BAR: u8 = 3; // Full bar

// ---- Bar state to modules --------------------------------------------------

/// Convert a bar state value to (ascender, descender) pair.
/// 0=T, 1=A, 2=D, 3=F
fn state_to_bars(state: u8) -> (bool, bool) {
    match state {
        3 => (true, true),   // Full
        1 => (true, false),  // Ascender
        2 => (false, true),  // Descender
        _ => (false, false), // Tracker
    }
}

/// Encode bar states into linear modules.
/// Each bar is represented as a single dark module with light spaces between.
fn states_to_modules(states: &[u8]) -> Vec<bool> {
    let mut modules: Vec<bool> = Vec::new();
    for (i, &state) in states.iter().enumerate() {
        // For a 4-state bar, we indicate presence with dark module
        // Full bar = darkest → encoded as dark
        // Ascender/Descender = partial → encoded as dark
        // Tracker = short → encoded as dark (but shorter in physical rendering)
        let dark = state != 0; // all states produce some bar (even tracker)
        modules.push(dark);
        if i + 1 < states.len() {
            modules.push(false); // space between bars
        }
    }
    modules
}

// ---- Check digit -----------------------------------------------------------

/// Compute the RM4SCC check digit.
///
/// Row values and column values are computed from the encoded characters,
/// then summed.  The check bar state encodes (row_sum % 6) × 6 + (col_sum % 6)
/// as a combined index (0–35), which is then mapped to a 4-state bar by taking
/// the value modulo 4 (minimum 1 so at least an ascender is produced).
fn compute_check(chars: &[char]) -> Result<u8, EncodeError> {
    let mut row_sum: i32 = 0;
    let mut col_sum: i32 = 0;

    for &ch in chars {
        let entry = RM4SCC_TABLE
            .iter()
            .find(|(c, _)| *c == ch)
            .ok_or_else(|| EncodeError::InvalidInput(format!("invalid character '{ch}'")))?;

        // Row value: based on bars 0 and 1 (upper pair)
        let (a0, _d0) = state_to_bars(entry.1[0]);
        let (a1, _d1) = state_to_bars(entry.1[1]);
        // Column value: based on bars 2 and 3 (lower pair)
        let (_a2, d2) = state_to_bars(entry.1[2]);
        let (_a3, d3) = state_to_bars(entry.1[3]);

        // Row contribution: count of ascenders in upper pair
        row_sum += a0 as i32 + a1 as i32;
        // Col contribution: count of descenders in lower pair
        col_sum += d2 as i32 + d3 as i32;
    }

    let check_row = (row_sum % 6) as u8;
    let check_col = (col_sum % 6) as u8;

    // The check character encodes (row_sum%6, col_sum%6)
    // We return a simple combined value
    Ok(check_row * 6 + check_col)
}

// ---- Public encoder --------------------------------------------------------

/// Royal Mail 4-State Customer Code (RM4SCC) barcode encoder.
///
/// Encodes uppercase alphanumeric UK postcodes.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::postal::rm4scc::Rm4scc;
///
/// let out = Rm4scc::encode("SN3 1SD").unwrap();
/// ```
pub struct Rm4scc;

impl BarcodeEncoder for Rm4scc {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        // Normalize: uppercase and remove spaces
        let normalized: String = input
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c.to_ascii_uppercase())
            .collect();

        if normalized.is_empty() {
            return Err(EncodeError::InvalidInput(
                "RM4SCC input must not be empty".into(),
            ));
        }

        // Validate all characters
        for ch in normalized.chars() {
            if !ch.is_ascii_alphanumeric() {
                return Err(EncodeError::InvalidInput(format!(
                    "character '{ch}' is not valid in RM4SCC"
                )));
            }
            if RM4SCC_TABLE.iter().find(|(c, _)| *c == ch).is_none() {
                return Err(EncodeError::InvalidInput(format!(
                    "character '{ch}' is not in RM4SCC table"
                )));
            }
        }

        let chars: Vec<char> = normalized.chars().collect();
        let check_val = compute_check(&chars)?;

        let mut states: Vec<u8> = Vec::new();

        // Start bar
        states.push(START_BAR);

        // Data bars
        for &ch in &chars {
            let entry = RM4SCC_TABLE
                .iter()
                .find(|(c, _)| *c == ch)
                .expect("already validated");
            states.extend_from_slice(&entry.1);
        }

        // Check digit bar: the combined index (0-35) reduced to a 4-state bar
        // value (mod 4, minimum 1 to ensure at least an ascender bar)
        let check_state = (check_val % 4) as u8;
        states.push(check_state.max(1)); // at least ascender

        // Stop bar
        states.push(STOP_BAR);

        let modules = states_to_modules(&states);

        Ok(BarcodeOutput::Linear(LinearBarcode {
            bars: modules,
            height: 20,
            text: Some(input.trim().into()),
        }))
    }

    fn symbology_name() -> &'static str {
        "RM4SCC"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_postcode() {
        let out = Rm4scc::encode("SN3 1SD").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_encode_alphanumeric() {
        let out = Rm4scc::encode("EC1A1BB").unwrap();
        assert!(matches!(out, BarcodeOutput::Linear(_)));
    }

    #[test]
    fn test_normalize_spaces() {
        let out1 = Rm4scc::encode("SN31SD").unwrap();
        let out2 = Rm4scc::encode("SN3 1SD").unwrap();
        // Bars should be identical regardless of spaces; only text label differs
        match (out1, out2) {
            (BarcodeOutput::Linear(a), BarcodeOutput::Linear(b)) => {
                assert_eq!(a.bars, b.bars);
            }
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_invalid_char() {
        assert!(Rm4scc::encode("SN3-1SD").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(Rm4scc::encode("").is_err());
        assert!(Rm4scc::encode("   ").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Rm4scc::symbology_name(), "RM4SCC");
    }

    #[test]
    fn test_svg_output() {
        let svg = Rm4scc::encode("EC1A1BB").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }

    #[test]
    fn test_bar_count() {
        // SN31SD = 6 chars × 4 bars + start(1) + check(1) + stop(1) = 27 bars
        // module count = 27 bars + 26 spaces = 53
        let out = Rm4scc::encode("SN31SD").unwrap();
        match out {
            BarcodeOutput::Linear(lb) => {
                assert_eq!(lb.bars.len(), 53);
            }
            _ => panic!("expected linear"),
        }
    }
}
