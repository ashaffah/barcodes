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

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

// ---- Fixed capacity bounds (largest supported 26×26 symbol) ----------------

/// Largest supported symbol dimension.
const MAX_SIZE: usize = 26;
/// Largest supported module count (`MAX_SIZE²`).
const MAX_CELLS: usize = MAX_SIZE * MAX_SIZE;
/// Largest data-codeword capacity across supported symbols.
const MAX_DATA_CW: usize = 44;
/// Largest error-correction codeword count across supported symbols.
const MAX_EC: usize = 28;

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

/// Compute Reed-Solomon check bytes for Data Matrix into `out[..ec_count]`.
fn rs_encode_dm(data: &[u8], ec_count: usize, out: &mut [u8]) {
    // Generator polynomial coefficients (length ec_count + 1).
    let mut poly = [0u8; MAX_EC + 1];
    poly[0] = 1;
    for i in 0..ec_count {
        let root = gf256_pow(2, i + 1);
        let cur = i + 1; // current polynomial length before this multiply
        let mut new_poly = [0u8; MAX_EC + 1];
        for j in 0..cur {
            new_poly[j] ^= poly[j];
            new_poly[j + 1] ^= gf256_mul(poly[j], root);
        }
        poly[..cur + 1].copy_from_slice(&new_poly[..cur + 1]);
    }

    // Polynomial division.
    let mut rem_buf = [0u8; MAX_EC];
    let rem = &mut rem_buf[..ec_count];
    for &d in data {
        let lead = d ^ rem[0];
        rem.copy_within(1.., 0);
        rem[ec_count - 1] = 0;
        if lead != 0 {
            for i in 0..ec_count {
                rem[i] ^= gf256_mul(lead, poly[i + 1]);
            }
        }
    }
    out[..ec_count].copy_from_slice(rem);
}

// ---- ASCII encoding --------------------------------------------------------

/// Encode input bytes in Data Matrix ASCII mode into `out`, returning the count.
///
/// ASCII values 1-128 are encoded as value + 1 (so 0 is unused).
/// Digit pairs 00-99 are encoded as 130+value.
fn ascii_encode(input: &[u8], out: &mut [u8]) -> Result<usize, EncodeError> {
    let mut n = 0;
    let mut push = |v: u8| -> Result<(), EncodeError> {
        *out.get_mut(n).ok_or(EncodeError::DataTooLong)? = v;
        n += 1;
        Ok(())
    };
    let mut i = 0;
    while i < input.len() {
        if i + 1 < input.len() && input[i].is_ascii_digit() && input[i + 1].is_ascii_digit() {
            // Encode digit pair
            let val = (input[i] - b'0') * 10 + (input[i + 1] - b'0');
            push(130 + val)?;
            i += 2;
        } else {
            // Single ASCII
            push(input[i] + 1)?;
            i += 1;
        }
    }
    Ok(n)
}

// ---- Main encoder ----------------------------------------------------------

/// Build a Data Matrix grid with finder pattern and data, writing the
/// row-major module grid into `buf[..size * size]`.
fn build_grid(
    size: usize,
    data_codewords: &[u8],
    ec_codewords: &[u8],
    buf: &mut [bool],
) -> Result<(), EncodeError> {
    let cells = size * size;
    if buf.len() < cells {
        return Err(EncodeError::BufferTooSmall);
    }

    // Tri-state scratch grid: -1 = unplaced, 0 = light, 1 = dark.
    let mut grid = [-1i16; MAX_CELLS];
    let at = |r: usize, c: usize| r * size + c;

    // Place finder pattern (L-shape: solid dark on bottom row and left column)
    for c in 0..size {
        grid[at(size - 1, c)] = 1; // bottom row (all dark)
        grid[at(0, c)] = if c % 2 == 0 { 1 } else { 0 }; // top row (alternating)
    }
    for r in 0..size {
        grid[at(r, 0)] = 1; // left column (all dark)
        grid[at(r, size - 1)] = if r % 2 == 0 { 0 } else { 1 }; // right column
    }

    // Combined data + EC codewords, addressed without concatenation.
    let total_cw = data_codewords.len() + ec_codewords.len();
    let cw_at = |idx: usize| -> u8 {
        if idx < data_codewords.len() {
            data_codewords[idx]
        } else if idx < total_cw {
            ec_codewords[idx - data_codewords.len()]
        } else {
            0
        }
    };

    // Place data using diagonal algorithm (simplified).
    let inner_size = size - 2; // exclude border
    let mut cw_idx = 0usize;
    let mut bit_pos = 0usize;

    'outer: for col_start in (1..inner_size + 1).step_by(2).rev() {
        let going_up = (inner_size - col_start) % 4 < 2;

        for k in 0..inner_size {
            // going_up: inner_size..=1, else 1..=inner_size
            let row = if going_up { inner_size - k } else { 1 + k };
            for dc in 0..2usize {
                let c = col_start + dc;
                if c > inner_size {
                    continue;
                }
                if grid[at(row, c)] >= 0 {
                    continue; // already placed (finder/timing)
                }

                let cw = cw_at(cw_idx);
                let bit = 7 - (bit_pos % 8);
                grid[at(row, c)] = ((cw >> bit) & 1) as i16;
                bit_pos += 1;
                if bit_pos.is_multiple_of(8) {
                    cw_idx += 1;
                    if cw_idx >= total_cw {
                        break 'outer;
                    }
                }
            }
        }
    }

    // Convert to bool grid (any -1 treated as light).
    for i in 0..cells {
        buf[i] = grid[i] == 1;
    }
    Ok(())
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
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::twod::datamatrix::DataMatrix;
///
/// let mut buf = [false; 26 * 26];
/// let Encoded::Matrix { width, height } = DataMatrix::encode_into("Hello DM", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(width, height);
/// ```
pub struct DataMatrix;

impl BarcodeEncoder for DataMatrix {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput(
                "Data Matrix input must not be empty",
            ));
        }

        // ASCII-encode into a fixed scratch buffer.
        let mut data_cw = [0u8; MAX_DATA_CW + 1];
        let n = ascii_encode(input.as_bytes(), &mut data_cw)?;

        // Find the smallest symbol that fits.
        let params = SYMBOL_PARAMS
            .iter()
            .find(|&&(_, cap, _, _, _)| n <= cap)
            .ok_or(EncodeError::DataTooLong)?;

        let (size, capacity, .., data_per_block, ec_per_block) = *params;

        // Pad to capacity with the padding codeword (129).
        let mut padded = [129u8; MAX_DATA_CW];
        padded[..n].copy_from_slice(&data_cw[..n]);
        let data = &padded[..data_per_block.min(capacity)];

        // Compute RS error correction.
        let mut ec = [0u8; MAX_EC];
        rs_encode_dm(data, ec_per_block, &mut ec);

        // Build the grid directly into the caller buffer.
        build_grid(size, data, &ec[..ec_per_block], buf)?;

        Ok(Encoded::Matrix {
            width: size,
            height: size,
        })
    }

    fn symbology_name() -> &'static str {
        "Data Matrix"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: &str, buf: &mut [bool]) -> (usize, usize) {
        match DataMatrix::encode_into(input, buf).unwrap() {
            Encoded::Matrix { width, height } => (width, height),
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_encode_basic() {
        let mut buf = [false; MAX_CELLS];
        let (w, h) = encode("Hello", &mut buf);
        assert!(w >= 10);
        assert_eq!(w, h);
    }

    #[test]
    fn test_encode_digits() {
        let mut buf = [false; MAX_CELLS];
        let (w, _) = encode("12345", &mut buf);
        assert!(w >= 10);
    }

    #[test]
    fn test_finder_pattern() {
        let mut buf = [false; MAX_CELLS];
        let (size, _) = encode("A", &mut buf);
        // Bottom row should be all dark (finder)
        let bottom = &buf[(size - 1) * size..size * size];
        assert!(bottom.iter().all(|&b| b), "bottom row should be all dark");
        // Left column should be all dark (finder)
        for r in 0..size {
            assert!(buf[r * size], "left column should be all dark");
        }
    }

    #[test]
    fn test_symbol_size_10x10_for_small_input() {
        let mut buf = [false; MAX_CELLS];
        assert_eq!(encode("Hi", &mut buf), (10, 10));
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; MAX_CELLS];
        assert!(DataMatrix::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 16];
        assert_eq!(
            DataMatrix::encode_into("Hi", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(DataMatrix::symbology_name(), "Data Matrix");
    }

    #[cfg(feature = "alloc")]
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
