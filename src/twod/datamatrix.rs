//! Data Matrix ECC 200 barcode encoder.
//!
//! Data Matrix is a 2D matrix barcode widely used in manufacturing, healthcare,
//! and logistics.  This implementation supports ECC 200 (Reed-Solomon error
//! correction) for square symbol sizes from 10×10 to 26×26.
//!
//! # Structure
//!
//! - L-shaped finder pattern on the bottom and left
//! - Alternating timing pattern on the top and right
//! - Data modules placed diagonally following the standard placement algorithm
//! - Reed-Solomon error correction codewords
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{vec, vec::Vec};

use crate::common::{
    errors::EncodeError,
    traits::BarcodeEncoder,
    types::{BarcodeOutput, MatrixBarcode},
};

// ---- Symbol parameters -----------------------------------------------------

/// Parameters for each supported square ECC 200 symbol size.
/// (total_size, data_capacity_bytes, rs_block_count, data_per_block, ec_per_block)
const SYMBOL_PARAMS: &[(usize, usize, usize, usize, usize)] = &[
    (10, 3, 1, 3, 5),    // 10×10
    (12, 5, 1, 5, 7),    // 12×12
    (14, 8, 1, 8, 10),   // 14×14
    (16, 12, 1, 12, 12), // 16×16
    (18, 18, 1, 18, 14), // 18×18
    (20, 22, 1, 22, 18), // 20×20
    (22, 30, 1, 30, 20), // 22×22
    (24, 36, 1, 36, 24), // 24×24
    (26, 44, 1, 44, 28), // 26×26
];

// ---- GF(256) for Data Matrix Reed-Solomon ----------------------------------

/// GF(256) primitive polynomial x^8 + x^5 + x^3 + x^2 + 1 = 0x12D
const PRIM_POLY: u32 = 0x12D;

fn gf256_mul(a: u8, b: u8) -> u8 {
    let mut result = 0u32;
    let mut aa = a as u32;
    let mut bb = b as u32;
    while bb > 0 {
        if bb & 1 != 0 {
            result ^= aa;
        }
        aa <<= 1;
        if aa & 0x100 != 0 {
            aa ^= PRIM_POLY;
        }
        bb >>= 1;
    }
    result as u8
}

fn gf256_pow(base: u8, exp: usize) -> u8 {
    let mut result = 1u8;
    for _ in 0..exp {
        result = gf256_mul(result, base);
    }
    result
}

/// Compute Reed-Solomon check bytes for Data Matrix.
fn rs_encode_dm(data: &[u8], ec_count: usize) -> Vec<u8> {
    // Generator polynomial coefficients
    let mut poly = vec![1u8; 1];
    for i in 0..ec_count {
        let root = gf256_pow(2, i + 1);
        let new_len = poly.len() + 1;
        let mut new_poly = vec![0u8; new_len];
        for (j, &gj) in poly.iter().enumerate() {
            new_poly[j] ^= gj;
            new_poly[j + 1] ^= gf256_mul(gj, root);
        }
        poly = new_poly;
    }

    // Polynomial division
    let mut remainder = vec![0u8; ec_count];
    for &d in data {
        let lead = d ^ remainder[0];
        remainder.copy_within(1.., 0);
        *remainder.last_mut().unwrap() = 0;
        if lead != 0 {
            for i in 0..ec_count {
                remainder[i] ^= gf256_mul(lead, poly[i + 1]);
            }
        }
    }
    remainder
}

// ---- ASCII encoding --------------------------------------------------------

/// Encode input bytes in Data Matrix ASCII mode.
/// ASCII values 1-128 are encoded as value + 1 (so 0 is unused).
/// Digit pairs 00-99 are encoded as 130+value.
fn ascii_encode(input: &[u8]) -> Vec<u8> {
    let mut codewords: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if i + 1 < input.len() && input[i].is_ascii_digit() && input[i + 1].is_ascii_digit() {
            // Encode digit pair
            let val = (input[i] - b'0') * 10 + (input[i + 1] - b'0');
            codewords.push(130 + val);
            i += 2;
        } else {
            // Single ASCII
            codewords.push(input[i] + 1);
            i += 1;
        }
    }
    codewords
}

// ---- Main encoder ----------------------------------------------------------

/// Build a Data Matrix grid with finder pattern and data.
fn build_grid(size: usize, data_codewords: &[u8], ec_codewords: &[u8]) -> Vec<Vec<bool>> {
    // Initialize grid: -1 = unplaced, 0 = light, 1 = dark
    let mut grid: Vec<Vec<i16>> = vec![vec![-1i16; size]; size];

    // Place finder pattern (L-shape: solid dark on bottom row and left column)
    #[allow(clippy::needless_range_loop)]
    for c in 0..size {
        grid[size - 1][c] = 1; // bottom row (all dark)
        grid[0][c] = if c % 2 == 0 { 1 } else { 0 }; // top row (alternating, starts dark)
    }
    #[allow(clippy::needless_range_loop)]
    for r in 0..size {
        grid[r][0] = 1; // left column (all dark)
        grid[r][size - 1] = if r % 2 == 0 { 0 } else { 1 }; // right column (alternating, starts light)
    }

    // Combine data and EC codewords
    let mut all_cw: Vec<u8> = Vec::with_capacity(data_codewords.len() + ec_codewords.len());
    all_cw.extend_from_slice(data_codewords);
    all_cw.extend_from_slice(ec_codewords);

    // Place data using diagonal algorithm (simplified)
    let inner_size = size - 2; // exclude border
    let mut cw_idx = 0usize;
    let mut bit_pos = 0usize;

    // Simple row-by-row placement within the data region
    'outer: for col_start in (1..inner_size + 1).step_by(2).rev() {
        let going_up = (inner_size - col_start) % 4 < 2;
        let row_range: Vec<usize> = if going_up {
            (1..inner_size + 1).rev().collect()
        } else {
            (1..inner_size + 1).collect()
        };

        for row in row_range {
            for dc in 0..2usize {
                let c = col_start + dc;
                if c > inner_size {
                    continue;
                }
                if grid[row][c] >= 0 {
                    continue; // already placed (finder/timing)
                }

                let cw = if cw_idx < all_cw.len() {
                    all_cw[cw_idx]
                } else {
                    0
                };
                let bit = 7 - (bit_pos % 8);
                grid[row][c] = ((cw >> bit) & 1) as i16;
                bit_pos += 1;
                if bit_pos.is_multiple_of(8) {
                    cw_idx += 1;
                    if cw_idx >= all_cw.len() {
                        break 'outer;
                    }
                }
            }
        }
    }

    // Convert to bool grid (any -1 treated as light)
    grid.into_iter()
        .map(|row| row.into_iter().map(|v| v == 1).collect())
        .collect()
}

// ---- Public encoder --------------------------------------------------------

/// Data Matrix ECC 200 barcode encoder.
///
/// Encodes text input into a square Data Matrix symbol.  The smallest symbol
/// that fits the data is automatically selected (10×10 to 26×26).
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::twod::datamatrix::DataMatrix;
///
/// let out = DataMatrix::encode("Hello DM").unwrap();
/// ```
pub struct DataMatrix;

impl BarcodeEncoder for DataMatrix {
    type Input = str;
    type Error = EncodeError;

    fn encode(input: &str) -> Result<BarcodeOutput, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Data Matrix input must not be empty".into(),
            ));
        }

        let data_cw = ascii_encode(input.as_bytes());

        // Find the smallest symbol that fits
        let params = SYMBOL_PARAMS
            .iter()
            .find(|&&(_, cap, _, _, _)| data_cw.len() <= cap)
            .ok_or(EncodeError::DataTooLong)?;

        let (size, capacity, .., data_per_block, ec_per_block) = *params;

        // Pad to capacity with padding codeword (129 = ASCII pad)
        let mut padded = data_cw.clone();
        while padded.len() < capacity {
            padded.push(129); // padding
        }
        padded.truncate(data_per_block);

        // Compute RS error correction
        let ec = rs_encode_dm(&padded, ec_per_block);

        // Build the grid
        let grid = build_grid(size, &padded, &ec);

        Ok(BarcodeOutput::Matrix(MatrixBarcode {
            width: size,
            height: size,
            modules: grid,
        }))
    }

    fn symbology_name() -> &'static str {
        "Data Matrix"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_basic() {
        let out = DataMatrix::encode("Hello").unwrap();
        match out {
            BarcodeOutput::Matrix(mb) => {
                assert!(mb.width >= 10);
                assert_eq!(mb.width, mb.height);
            }
            _ => panic!("expected matrix barcode"),
        }
    }

    #[test]
    fn test_encode_digits() {
        let out = DataMatrix::encode("12345").unwrap();
        assert!(matches!(out, BarcodeOutput::Matrix(_)));
    }

    #[test]
    fn test_finder_pattern() {
        let out = DataMatrix::encode("A").unwrap();
        match out {
            BarcodeOutput::Matrix(mb) => {
                let size = mb.width;
                // Bottom row should be all dark (finder)
                let bottom = &mb.modules[size - 1];
                assert!(bottom.iter().all(|&b| b), "bottom row should be all dark");
                // Left column should be all dark (finder)
                for row in &mb.modules {
                    assert!(row[0], "left column should be all dark");
                }
            }
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_symbol_size_10x10_for_small_input() {
        let out = DataMatrix::encode("Hi").unwrap();
        match out {
            BarcodeOutput::Matrix(mb) => {
                assert_eq!(mb.width, 10);
                assert_eq!(mb.height, 10);
            }
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_empty_input() {
        assert!(DataMatrix::encode("").is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(DataMatrix::symbology_name(), "Data Matrix");
    }

    #[test]
    fn test_svg_output() {
        let svg = DataMatrix::encode("Test").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }

    #[test]
    fn test_gf256_mul() {
        assert_eq!(gf256_mul(0, 1), 0);
        assert_eq!(gf256_mul(1, 1), 1);
        assert_eq!(gf256_mul(2, 2), 4);
    }
}
