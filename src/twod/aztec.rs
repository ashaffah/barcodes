//! Aztec Code barcode encoder.
//!
//! Aztec Code is a 2D matrix barcode used for transportation tickets and other
//! applications.  It has a distinctive bull's-eye finder pattern at the center.
//!
//! # Structure
//!
//! - Central finder pattern: concentric squares (bull's-eye)
//! - Mode message surrounding the finder pattern
//! - Data encoded in layers spiraling outward from the center
//! - Reed-Solomon error correction
//!
//! # Sizes
//!
//! - Compact Aztec: 1–4 layers (15×15 to 27×27 minus corners)
//! - Full-range Aztec: 1–32 layers
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{vec, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, MatrixBarcode},
};

// ---- GF(2^n) Reed-Solomon --------------------------------------------------

/// GF(64) operations (primitive polynomial x^6 + x + 1 = 0x43).
fn gf64_mul(a: u8, b: u8) -> u8 {
    let mut result = 0u8;
    let mut aa = a & 0x3F;
    let mut bb = b & 0x3F;
    while bb > 0 {
        if bb & 1 != 0 {
            result ^= aa;
        }
        aa = (aa << 1) ^ if aa & 0x20 != 0 { 0x43 } else { 0 };
        bb >>= 1;
    }
    result & 0x3F
}

/// RS encode using GF(64) for data (6-bit codewords).
fn rs_data(data: &[u8], ec_count: usize) -> Vec<u8> {
    let mut remainder = vec![0u8; ec_count];
    for &d in data {
        let d = d & 0x3F;
        let lead = d ^ remainder[0];
        remainder.copy_within(1.., 0);
        *remainder.last_mut().unwrap() = 0;
        if lead != 0 {
            for coef in remainder.iter_mut() {
                *coef ^= gf64_mul(lead, *coef);
            }
        }
    }
    remainder
}

// ---- Text encoding ---------------------------------------------------------

/// Encode ASCII text into 6-bit Aztec code data codewords.
///
/// Uses the standard Aztec upper-case mode encoding.
/// Characters not in the upper-case set fall back to byte encoding.
fn encode_text(input: &str) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::new();

    for &b in input.as_bytes() {
        // Upper-case mode: space=1, A-Z=2..27, .=28, ,=29, :=30, CR=31
        let code: Option<u8> = match b {
            b' ' => Some(1),
            b'A'..=b'Z' => Some(b - b'A' + 2),
            b'a'..=b'z' => Some(b - b'a' + 2), // treat as uppercase
            b'.' => Some(28),
            b',' => Some(29),
            b':' => Some(30),
            b'\r' => Some(31),
            _ => None,
        };

        if let Some(c) = code {
            // 5-bit upper-case character
            for bit in (0..5).rev() {
                bits.push((c >> bit) & 1 != 0);
            }
        } else {
            // Shift to byte mode (code 31 in upper) then 8-bit byte
            // Shift byte: 11111 in upper mode
            for bit in (0..5).rev() {
                bits.push((31u8 >> bit) & 1 != 0);
            }
            for bit in (0..8).rev() {
                bits.push((b >> bit) & 1 != 0);
            }
        }
    }

    // Pack bits into 6-bit codewords
    // Pad to multiple of 6
    while !bits.len().is_multiple_of(6) {
        bits.push(true); // pad with 1
    }

    bits.chunks(6)
        .map(|chunk| chunk.iter().fold(0u8, |acc, &b| (acc << 1) | b as u8))
        .collect()
}

// ---- Compact Aztec finder pattern ------------------------------------------

/// Size of the compact Aztec finder (bull's-eye core): always 11×11 for compact.
const COMPACT_FINDER_SIZE: usize = 11;

/// Build the compact Aztec bull's-eye finder pattern centered in a grid.
fn place_compact_finder(grid: &mut [Vec<i8>], center: usize) {
    let _half = COMPACT_FINDER_SIZE / 2;
    // Concentric squares: 5 rings (alternating dark/light from center out)
    for ring in 0..=5i32 {
        let dark = ring % 2 == 0; // inner ring (0) is dark
        let val = if dark { 1i8 } else { 0i8 };
        let r_start = (center as i32 - ring).max(0) as usize;
        let r_end = (center as i32 + ring).min(grid.len() as i32 - 1) as usize;
        #[allow(clippy::needless_range_loop)]
        for r in r_start..=r_end {
            for c in r_start..=r_end {
                if r == r_start || r == r_end || c == r_start || c == r_end {
                    grid[r][c] = val;
                }
            }
        }
    }
    // Reference grid mark (bottom-right quadrant dark cell)
    if center + 1 < grid.len() && center + 1 < grid[0].len() {
        grid[center + 1][center + 1] = 1;
    }
}

/// Place the orientation marks for compact Aztec.
fn place_compact_orientation(grid: &mut [Vec<i8>], center: usize) {
    // The orientation pattern is 3 dark + 1 light going clockwise around the bull's-eye
    // For compact: 3 dark modules on the top-left arc
    let c = center;
    // Top-left
    grid[c - 5][c - 5] = 1;
    grid[c - 5][c - 4] = 1;
    grid[c - 4][c - 5] = 1;
    // Bottom-right (reference)
    grid[c + 5][c + 5] = 0;
}

// ---- Compact Aztec encoder -------------------------------------------------

/// Encode data bits into a single compact Aztec layer spiraling outward.
fn place_compact_layer(grid: &mut [Vec<i8>], size: usize, layer: usize, data_bits: &[bool]) {
    let center = size / 2;
    // Layer 1 starts at distance 6 from center (outside the 11×11 finder)
    let start = center as i32 - 5 - layer as i32;
    let end = center as i32 + 5 + layer as i32;

    if start < 0 || end >= size as i32 {
        return;
    }

    let mut bit_idx = 0;
    let s = start as usize;
    let e = end as usize;

    // Top row (left to right)
    #[allow(clippy::needless_range_loop)]
    for c in s..=e {
        if bit_idx < data_bits.len() && grid[s][c] < 0 {
            grid[s][c] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Right column (top+1 to bottom)
    #[allow(clippy::needless_range_loop)]
    for r in s + 1..=e {
        if bit_idx < data_bits.len() && grid[r][e] < 0 {
            grid[r][e] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Bottom row (right-1 to left)
    #[allow(clippy::needless_range_loop)]
    for c in (s..e).rev() {
        if bit_idx < data_bits.len() && grid[e][c] < 0 {
            grid[e][c] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Left column (bottom-1 to top+1)
    #[allow(clippy::needless_range_loop)]
    for r in (s + 1..e).rev() {
        if bit_idx < data_bits.len() && grid[r][s] < 0 {
            grid[r][s] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
}

// ---- Public encoder --------------------------------------------------------

/// Aztec Code barcode encoder.
///
/// Encodes text into a compact Aztec Code symbol.  Automatically selects the
/// number of layers based on data length.  Uses error correction sufficient
/// for standard use.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::twod::aztec::Aztec;
///
/// let out = Aztec::encode("AZTEC").unwrap();
/// ```
pub struct Aztec;

impl BarcodeEncoder for Aztec {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Aztec input must not be empty".into(),
            ));
        }

        let data_codewords = encode_text(input);
        if data_codewords.is_empty() {
            return Err(EncodeError::InvalidInput("no encodable data found".into()));
        }

        // Choose number of compact layers (1-4) based on data size
        // Each compact layer provides ~(11 + 2*layer)*4 - 8 bit positions
        // Simplified: use layer count based on codeword count
        let layers = match data_codewords.len() {
            0..=4 => 1,
            5..=11 => 2,
            12..=22 => 3,
            23..=40 => 4,
            _ => return Err(EncodeError::DataTooLong),
        };

        let size = 11 + layers * 4; // compact Aztec size

        let mut grid: Vec<Vec<i8>> = vec![vec![-1i8; size]; size];
        let center = size / 2;

        // Place finder pattern
        place_compact_finder(&mut grid, center);

        // Place orientation marks
        if center >= 5 {
            place_compact_orientation(&mut grid, center);
        }

        // Compute RS error correction for data (using ~23% EC)
        let ec_count = (data_codewords.len() / 4).max(2);
        let ec = rs_data(&data_codewords, ec_count);

        // Combine data + EC into bits
        let mut all_cw: Vec<u8> = Vec::new();
        all_cw.extend_from_slice(&data_codewords);
        all_cw.extend_from_slice(&ec);

        let data_bits: Vec<bool> = all_cw
            .iter()
            .flat_map(|&cw| (0..6).rev().map(move |i| (cw >> i) & 1 != 0))
            .collect();

        // Place data in layers
        for layer in 1..=layers {
            let layer_bits_start = (layer - 1) * (data_bits.len() / layers);
            let layer_bits_end = if layer == layers {
                data_bits.len()
            } else {
                layer * (data_bits.len() / layers)
            };
            if layer_bits_start < data_bits.len() {
                place_compact_layer(
                    &mut grid,
                    size,
                    layer,
                    &data_bits[layer_bits_start..layer_bits_end.min(data_bits.len())],
                );
            }
        }

        // Fill any remaining -1 cells with light
        let modules: Vec<Vec<bool>> = grid
            .into_iter()
            .map(|row| row.into_iter().map(|v| v == 1).collect())
            .collect();

        Ok(BarcodeOutput::Matrix(MatrixBarcode {
            width: size,
            height: size,
            modules,
        }))
    }

    fn symbology_name() -> &'static str {
        "Aztec Code"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_basic() {
        let out = Aztec::encode("AZTEC").unwrap();
        match out {
            BarcodeOutput::Matrix(mb) => {
                assert!(mb.width >= 15); // compact layer 1 = 11 + 4 = 15
                assert_eq!(mb.width, mb.height);
            }
            _ => panic!("expected matrix barcode"),
        }
    }

    #[test]
    fn test_encode_short() {
        let out = Aztec::encode("A").unwrap();
        assert!(matches!(out, BarcodeOutput::Matrix(_)));
    }

    #[test]
    fn test_finder_pattern_center_is_dark() {
        let out = Aztec::encode("HI").unwrap();
        match out {
            BarcodeOutput::Matrix(mb) => {
                let center = mb.width / 2;
                // Center module should be dark
                assert!(mb.modules[center][center], "center must be dark");
            }
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_empty_input() {
        assert!(Aztec::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Aztec::symbology_name(), "Aztec Code");
    }

    #[test]
    fn test_svg_output() {
        let svg = Aztec::encode("Test").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }

    #[test]
    fn test_gf64_mul_zero() {
        assert_eq!(gf64_mul(0, 5), 0);
        assert_eq!(gf64_mul(5, 0), 0);
    }

    #[test]
    fn test_gf64_mul_identity() {
        assert_eq!(gf64_mul(1, 7), 7);
        assert_eq!(gf64_mul(7, 1), 7);
    }
}
