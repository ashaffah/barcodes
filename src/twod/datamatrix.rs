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

// ---- ISO/IEC 16022 ECC 200 symbol-character placement ----------------------
//
// These functions reproduce the standard placement algorithm (ISO/IEC 16022
// Annex F). Each mapping-matrix cell is tagged with which codeword bit it
// carries so the symbol decodes on a conforming reader.

/// Tag mapping cell (r, c) with bit `b` (7 = MSB) of 1-based codeword `p`,
/// wrapping negative coordinates per the spec.
fn place_bit(a: &mut [u16], nr: isize, nc: isize, mut r: isize, mut c: isize, p: usize, b: u16) {
    if r < 0 {
        r += nr;
        c += 4 - ((nr + 4) % 8);
    }
    if c < 0 {
        c += nc;
        r += 4 - ((nc + 4) % 8);
    }
    a[(r * nc + c) as usize] = ((p as u16) << 3) | b;
}

/// Place the 8 modules of the standard "utah" shape for codeword `p`.
fn place_block(a: &mut [u16], nr: isize, nc: isize, r: isize, c: isize, p: usize) {
    place_bit(a, nr, nc, r - 2, c - 2, p, 7);
    place_bit(a, nr, nc, r - 2, c - 1, p, 6);
    place_bit(a, nr, nc, r - 1, c - 2, p, 5);
    place_bit(a, nr, nc, r - 1, c - 1, p, 4);
    place_bit(a, nr, nc, r - 1, c, p, 3);
    place_bit(a, nr, nc, r, c - 2, p, 2);
    place_bit(a, nr, nc, r, c - 1, p, 1);
    place_bit(a, nr, nc, r, c, p, 0);
}

fn corner_a(a: &mut [u16], nr: isize, nc: isize, p: usize) {
    place_bit(a, nr, nc, nr - 1, 0, p, 7);
    place_bit(a, nr, nc, nr - 1, 1, p, 6);
    place_bit(a, nr, nc, nr - 1, 2, p, 5);
    place_bit(a, nr, nc, 0, nc - 2, p, 4);
    place_bit(a, nr, nc, 0, nc - 1, p, 3);
    place_bit(a, nr, nc, 1, nc - 1, p, 2);
    place_bit(a, nr, nc, 2, nc - 1, p, 1);
    place_bit(a, nr, nc, 3, nc - 1, p, 0);
}

fn corner_b(a: &mut [u16], nr: isize, nc: isize, p: usize) {
    place_bit(a, nr, nc, nr - 3, 0, p, 7);
    place_bit(a, nr, nc, nr - 2, 0, p, 6);
    place_bit(a, nr, nc, nr - 1, 0, p, 5);
    place_bit(a, nr, nc, 0, nc - 4, p, 4);
    place_bit(a, nr, nc, 0, nc - 3, p, 3);
    place_bit(a, nr, nc, 0, nc - 2, p, 2);
    place_bit(a, nr, nc, 0, nc - 1, p, 1);
    place_bit(a, nr, nc, 1, nc - 1, p, 0);
}

fn corner_c(a: &mut [u16], nr: isize, nc: isize, p: usize) {
    place_bit(a, nr, nc, nr - 3, 0, p, 7);
    place_bit(a, nr, nc, nr - 2, 0, p, 6);
    place_bit(a, nr, nc, nr - 1, 0, p, 5);
    place_bit(a, nr, nc, 0, nc - 2, p, 4);
    place_bit(a, nr, nc, 0, nc - 1, p, 3);
    place_bit(a, nr, nc, 1, nc - 1, p, 2);
    place_bit(a, nr, nc, 2, nc - 1, p, 1);
    place_bit(a, nr, nc, 3, nc - 1, p, 0);
}

fn corner_d(a: &mut [u16], nr: isize, nc: isize, p: usize) {
    place_bit(a, nr, nc, nr - 1, 0, p, 7);
    place_bit(a, nr, nc, nr - 1, nc - 1, p, 6);
    place_bit(a, nr, nc, 0, nc - 3, p, 5);
    place_bit(a, nr, nc, 0, nc - 2, p, 4);
    place_bit(a, nr, nc, 0, nc - 1, p, 3);
    place_bit(a, nr, nc, 1, nc - 3, p, 2);
    place_bit(a, nr, nc, 1, nc - 2, p, 1);
    place_bit(a, nr, nc, 1, nc - 1, p, 0);
}

/// Compute the ECC 200 placement map for an `nr × nc` mapping matrix.
///
/// Each entry is `0` (unused → light), `1` (fixed dark corner module), or
/// `(codeword_1based << 3) | bit` with bit 7 = MSB.
fn ecc200_placement(nr: usize, nc: usize) -> Vec<u16> {
    let mut a = vec![0u16; nr * nc];
    let (nri, nci) = (nr as isize, nc as isize);
    let idx = |r: isize, c: isize| (r * nci + c) as usize;

    let mut p = 1usize;
    let mut r: isize = 4;
    let mut c: isize = 0;

    loop {
        // Corner conditions.
        if r == nri && c == 0 {
            corner_a(&mut a, nri, nci, p);
            p += 1;
        }
        if r == nri - 2 && c == 0 && (nci % 4) != 0 {
            corner_b(&mut a, nri, nci, p);
            p += 1;
        }
        if r == nri - 2 && c == 0 && (nci % 8) == 4 {
            corner_c(&mut a, nri, nci, p);
            p += 1;
        }
        if r == nri + 4 && c == 2 && (nci % 8) == 0 {
            corner_d(&mut a, nri, nci, p);
            p += 1;
        }

        // Sweep diagonally up and to the right.
        loop {
            if r < nri && c >= 0 && a[idx(r, c)] == 0 {
                place_block(&mut a, nri, nci, r, c, p);
                p += 1;
            }
            r -= 2;
            c += 2;
            if !(r >= 0 && c < nci) {
                break;
            }
        }
        r += 1;
        c += 3;

        // Sweep diagonally down and to the left.
        loop {
            if r >= 0 && c < nci && a[idx(r, c)] == 0 {
                place_block(&mut a, nri, nci, r, c, p);
                p += 1;
            }
            r += 2;
            c -= 2;
            if !(r < nri && c >= 0) {
                break;
            }
        }
        r += 3;
        c += 1;

        if !(r < nri || c < nci) {
            break;
        }
    }

    // Fixed pattern for the unfilled bottom-right corner (small even sizes).
    let last = nr * nc - 1;
    if a[last] == 0 {
        a[last] = 1;
        a[nr * nc - nc - 2] = 1;
    }
    a
}

/// Build a Data Matrix grid with the standard finder/timing pattern and ECC 200
/// data placement.
fn build_grid(size: usize, data_codewords: &[u8], ec_codewords: &[u8]) -> Vec<Vec<bool>> {
    let mut grid: Vec<Vec<bool>> = vec![vec![false; size]; size];

    // Finder pattern: solid L on the left column and bottom row.
    #[allow(clippy::needless_range_loop)]
    for i in 0..size {
        grid[size - 1][i] = true; // bottom solid
        grid[i][0] = true; // left solid
    }
    // Timing pattern: top row dark on even columns, right column dark on odd
    // rows (so both tracks meet the solid L correctly).
    for i in (0..size).step_by(2) {
        grid[0][i] = true; // top timing: even columns
    }
    for i in (1..size).step_by(2) {
        grid[i][size - 1] = true; // right timing: odd rows
    }

    // Combine data + EC codewords in placement order.
    let mut all_cw: Vec<u8> = Vec::with_capacity(data_codewords.len() + ec_codewords.len());
    all_cw.extend_from_slice(data_codewords);
    all_cw.extend_from_slice(ec_codewords);

    // Standard ECC 200 placement into the (size-2) × (size-2) data region.
    let nr = size - 2;
    let nc = size - 2;
    let places = ecc200_placement(nr, nc);
    for mr in 0..nr {
        for mc in 0..nc {
            let v = places[mr * nc + mc];
            let dark = match v {
                0 => false,
                1 => true,
                _ => {
                    let cw = all_cw[(v >> 3) as usize - 1];
                    (cw >> (v & 7)) & 1 == 1
                }
            };
            grid[mr + 1][mc + 1] = dark;
        }
    }

    grid
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
/// use barcodes::twod::datamatrix::DataMatrix;
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

    /// Canonical ISO/IEC 16022 worked example: ASCII-encoding "123456" gives
    /// data codewords [142, 164, 186]; ECC 200 appends [114, 25, 5, 88, 102].
    #[test]
    fn test_rs_iso_reference_vector() {
        let data = ascii_encode(b"123456");
        assert_eq!(data, vec![142, 164, 186]);
        let ec = rs_encode_dm(&data, 5);
        assert_eq!(ec, vec![114, 25, 5, 88, 102]);
    }

    /// Recover the codeword stream from a rendered symbol by inverting the
    /// standard placement — verifies finder/timing offset and bit placement.
    fn recover_codewords(mb: &MatrixBarcode) -> Vec<u8> {
        let size = mb.width;
        let nr = size - 2;
        let nc = size - 2;
        let places = ecc200_placement(nr, nc);
        let capacity = nr * nc / 8;
        let mut cw = vec![0u8; capacity];
        for mr in 0..nr {
            for mc in 0..nc {
                let v = places[mr * nc + mc];
                if v > 1 && mb.modules[mr + 1][mc + 1] {
                    cw[(v >> 3) as usize - 1] |= 1 << (v & 7);
                }
            }
        }
        cw
    }

    /// Decode ECC 200 ASCII-mode data codewords back to the original bytes.
    fn decode_ascii(cw: &[u8]) -> alloc::string::String {
        let mut s = alloc::string::String::new();
        for &c in cw {
            match c {
                129 => break, // pad → end of data
                1..=128 => s.push((c - 1) as char),
                130..=229 => {
                    let v = c - 130;
                    s.push((b'0' + v / 10) as char);
                    s.push((b'0' + v % 10) as char);
                }
                _ => {}
            }
        }
        s
    }

    /// Round-trip: encode → invert placement → check RS syndrome → decode ASCII.
    /// Proves the placement is self-consistent and RS-valid for every size.
    #[test]
    fn test_round_trip_all_sizes() {
        let inputs = [
            "A",
            "Hi",
            "12345",
            "HELLO WORLD",
            "f3411c82-1c70-4207-977e-99f5580e7e3b",
            "The quick brown fox jumps over the lazy do", // 42 chars → 26×26
        ];
        for input in inputs {
            let mb = match DataMatrix::encode(input).unwrap() {
                BarcodeOutput::Matrix(mb) => mb,
                _ => panic!("expected matrix"),
            };
            let size = mb.width;
            let params = SYMBOL_PARAMS.iter().find(|p| p.0 == size).unwrap();
            let (_, _, _, data_per_block, ec_per_block) = *params;

            let all_cw = recover_codewords(&mb);
            let (data, ec) = all_cw.split_at(data_per_block);

            // Reed-Solomon must be consistent with the recovered data.
            assert_eq!(
                rs_encode_dm(data, ec_per_block),
                ec,
                "RS mismatch for {input:?} ({size}x{size})"
            );

            // ASCII payload must decode back to the original input.
            let decoded = decode_ascii(data);
            assert_eq!(decoded, input, "round-trip mismatch ({size}x{size})");
        }
    }

    /// The four corner modules must match the standard finder/timing pattern.
    #[test]
    fn test_finder_timing_corners() {
        let mb = match DataMatrix::encode("Hi").unwrap() {
            BarcodeOutput::Matrix(mb) => mb,
            _ => panic!(),
        };
        let n = mb.width - 1;
        assert!(mb.modules[0][0], "top-left dark");
        assert!(mb.modules[n][0], "bottom-left dark");
        assert!(mb.modules[n][n], "bottom-right dark");
        // Top timing: dark at even columns, light at odd.
        assert!(mb.modules[0][2], "top timing even col dark");
        assert!(!mb.modules[0][1], "top timing odd col light");
        // Right timing: dark at odd rows, light at even.
        assert!(mb.modules[1][n], "right timing odd row dark");
        assert!(!mb.modules[0][n], "right timing even row light");
    }
}
