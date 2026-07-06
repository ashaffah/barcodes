//! Data Matrix ECC 200 barcode encoder.
//!
//! Data Matrix is a 2D matrix barcode widely used in manufacturing, healthcare,
//! and logistics.  This implementation supports ECC 200 (Reed-Solomon error
//! correction) for square symbol sizes from 10×10 to 48×48 (up to 174 data
//! codewords), including the multi-region sizes 32×32–48×48.
//!
//! # Structure
//!
//! - L-shaped finder pattern on the bottom and left of each data region
//! - Alternating timing pattern on the top and right of each data region
//! - Data placed with the standard ISO/IEC 16022 symbol-character algorithm
//! - Reed-Solomon error correction codewords
#![forbid(unsafe_code)]

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

// ---- Fixed capacity bounds (largest supported 48×48 symbol) ----------------

/// Largest supported symbol dimension (used to size test buffers).
#[cfg(test)]
const MAX_SIZE: usize = 48;
/// Largest supported module count (`MAX_SIZE²`).
#[cfg(test)]
const MAX_CELLS: usize = MAX_SIZE * MAX_SIZE;
/// Largest mapping-matrix side (`regions · data_region`, 2·22 for 48×48).
const MAX_MAPPING: usize = 44;
/// Largest mapping-matrix cell count.
const MAX_MAP_CELLS: usize = MAX_MAPPING * MAX_MAPPING;
/// Largest data-codeword capacity across supported symbols.
const MAX_DATA_CW: usize = 174;
/// Largest error-correction codeword count across supported symbols.
const MAX_EC: usize = 68;

// ---- Symbol parameters -----------------------------------------------------

/// Parameters for each supported square ECC 200 symbol size.
///
/// `(symbol_size, data_region, regions_per_side, data_codewords, ec_codewords)`
/// where `symbol_size = regions_per_side * (data_region + 2)`.  Only symbols
/// using a single Reed-Solomon block are listed (sizes 10×10 – 48×48); larger
/// sizes need interleaved RS blocks and are not yet supported.
const SYMBOL_PARAMS: &[(usize, usize, usize, usize, usize)] = &[
    (10, 8, 1, 3, 5),     // 10×10
    (12, 10, 1, 5, 7),    // 12×12
    (14, 12, 1, 8, 10),   // 14×14
    (16, 14, 1, 12, 12),  // 16×16
    (18, 16, 1, 18, 14),  // 18×18
    (20, 18, 1, 22, 18),  // 20×20
    (22, 20, 1, 30, 20),  // 22×22
    (24, 22, 1, 36, 24),  // 24×24
    (26, 24, 1, 44, 28),  // 26×26
    (32, 14, 2, 62, 36),  // 32×32 (2×2 regions)
    (36, 16, 2, 86, 42),  // 36×36
    (40, 18, 2, 114, 48), // 40×40
    (44, 20, 2, 144, 56), // 44×44
    (48, 22, 2, 174, 68), // 48×48
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

/// Compute the ECC 200 placement map into `a[..nr*nc]`.
///
/// Each entry is `0` (unused → light), `1` (fixed dark corner module), or
/// `(codeword_1based << 3) | bit` with bit 7 = MSB.
fn ecc200_placement(nr: usize, nc: usize, a: &mut [u16]) {
    for x in a[..nr * nc].iter_mut() {
        *x = 0;
    }
    let (nri, nci) = (nr as isize, nc as isize);
    let idx = |r: isize, c: isize| (r * nci + c) as usize;

    let mut p = 1usize;
    let mut r: isize = 4;
    let mut c: isize = 0;

    loop {
        // Corner conditions.
        if r == nri && c == 0 {
            corner_a(a, nri, nci, p);
            p += 1;
        }
        if r == nri - 2 && c == 0 && (nci % 4) != 0 {
            corner_b(a, nri, nci, p);
            p += 1;
        }
        if r == nri - 2 && c == 0 && (nci % 8) == 4 {
            corner_c(a, nri, nci, p);
            p += 1;
        }
        if r == nri + 4 && c == 2 && (nci % 8) == 0 {
            corner_d(a, nri, nci, p);
            p += 1;
        }

        // Sweep diagonally up and to the right.
        loop {
            if r < nri && c >= 0 && a[idx(r, c)] == 0 {
                place_block(a, nri, nci, r, c, p);
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
                place_block(a, nri, nci, r, c, p);
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
}

// ---- Main encoder ----------------------------------------------------------

/// Build a Data Matrix grid with the standard finder/timing pattern and ECC 200
/// data placement, writing the row-major module grid into `buf[..size * size]`.
///
/// `data_region` is the interior data size of one region and `regions` is the
/// number of regions per side (1 for sizes ≤ 26, 2 for 32–48).
fn build_grid(
    size: usize,
    data_region: usize,
    regions: usize,
    data_codewords: &[u8],
    ec_codewords: &[u8],
    buf: &mut [bool],
) -> Result<(), EncodeError> {
    let cells = size * size;
    if buf.len() < cells {
        return Err(EncodeError::BufferTooSmall);
    }
    for x in buf[..cells].iter_mut() {
        *x = false;
    }

    // Finder/timing pattern around every data region.
    let block = data_region + 2;
    for br in 0..regions {
        for bc in 0..regions {
            let r0 = br * block;
            let c0 = bc * block;
            for i in 0..block {
                buf[(r0 + block - 1) * size + c0 + i] = true; // bottom solid
                buf[(r0 + i) * size + c0] = true; // left solid
            }
            let mut i = 0;
            while i < block {
                buf[r0 * size + c0 + i] = true; // top timing: even columns
                i += 2;
            }
            let mut i = 1;
            while i < block {
                buf[(r0 + i) * size + c0 + block - 1] = true; // right timing: odd rows
                i += 2;
            }
        }
    }

    // Standard ECC 200 placement over the combined mapping matrix, then map each
    // logical cell into its region's interior (offset past that region's border).
    let mapping = regions * data_region;
    let mut places = [0u16; MAX_MAP_CELLS];
    ecc200_placement(mapping, mapping, &mut places);

    let data_len = data_codewords.len();
    for mr in 0..mapping {
        for mc in 0..mapping {
            let v = places[mr * mapping + mc];
            let dark = match v {
                0 => false,
                1 => true,
                _ => {
                    let cw_idx = (v >> 3) as usize - 1;
                    let cw = if cw_idx < data_len {
                        data_codewords[cw_idx]
                    } else {
                        ec_codewords[cw_idx - data_len]
                    };
                    (cw >> (v & 7)) & 1 == 1
                }
            };
            let pr = (mr / data_region) * block + 1 + (mr % data_region);
            let pc = (mc / data_region) * block + 1 + (mc % data_region);
            buf[pr * size + pc] = dark;
        }
    }

    Ok(())
}

// ---- Public encoder --------------------------------------------------------

/// Data Matrix ECC 200 barcode encoder.
///
/// Encodes text input into a square Data Matrix symbol.  The smallest symbol
/// that fits the data is automatically selected (10×10 to 48×48).
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::twod::datamatrix::DataMatrix;
///
/// let mut buf = [false; 48 * 48];
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

        // Find the smallest symbol whose data capacity fits.
        let params = SYMBOL_PARAMS
            .iter()
            .find(|&&(_, _, _, data_cap, _)| n <= data_cap)
            .ok_or(EncodeError::DataTooLong)?;

        let (size, data_region, regions, data_cap, ec_count) = *params;

        // Pad to the data capacity. ECC 200 uses codeword 129 for the first
        // pad, then the "253-state" pseudo-random algorithm for the rest.
        let mut padded = [0u8; MAX_DATA_CW];
        padded[..n].copy_from_slice(&data_cw[..n]);
        if n < data_cap {
            padded[n] = 129;
            let mut i = n + 1;
            while i < data_cap {
                let pos = i + 1; // 1-based codeword position
                let r = ((149 * pos) % 253) + 1;
                let v = 129 + r;
                padded[i] = if v > 254 { (v - 254) as u8 } else { v as u8 };
                i += 1;
            }
        }
        let data = &padded[..data_cap];

        // Compute RS error correction.
        let mut ec = [0u8; MAX_EC];
        rs_encode_dm(data, ec_count, &mut ec);

        // Build the grid directly into the caller buffer.
        build_grid(size, data_region, regions, data, &ec[..ec_count], buf)?;

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

    /// Recover the codeword stream from a rendered symbol by inverting the
    /// standard placement (returns the number of codewords).
    fn recover(
        buf: &[bool],
        size: usize,
        data_region: usize,
        regions: usize,
        out: &mut [u8],
    ) -> usize {
        let block = data_region + 2;
        let mapping = regions * data_region;
        let mut places = [0u16; MAX_MAP_CELLS];
        ecc200_placement(mapping, mapping, &mut places);
        let capacity = mapping * mapping / 8;
        for x in out[..capacity].iter_mut() {
            *x = 0;
        }
        for mr in 0..mapping {
            for mc in 0..mapping {
                let v = places[mr * mapping + mc];
                if v > 1 {
                    let pr = (mr / data_region) * block + 1 + (mr % data_region);
                    let pc = (mc / data_region) * block + 1 + (mc % data_region);
                    if buf[pr * size + pc] {
                        out[(v >> 3) as usize - 1] |= 1 << (v & 7);
                    }
                }
            }
        }
        capacity
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
    fn test_finder_timing() {
        let mut buf = [false; MAX_CELLS];
        let (size, _) = encode("Hi", &mut buf);
        let n = size - 1;
        assert!(buf[(size - 1) * size], "bottom-left dark");
        assert!(buf[n * size + n], "bottom-right dark");
        // Bottom row all dark, left column all dark (finder).
        assert!(buf[(size - 1) * size..size * size].iter().all(|&b| b));
        for r in 0..size {
            assert!(buf[r * size], "left column dark");
        }
        // Timing: top even col dark / odd light; right odd row dark / even light.
        assert!(buf[2], "top timing even col dark");
        assert!(!buf[1], "top timing odd col light");
        assert!(buf[size + n], "right timing odd row dark");
        assert!(!buf[n], "right timing even row light");
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

    #[test]
    fn test_gf256_mul() {
        assert_eq!(gf256_mul(0, 1), 0);
        assert_eq!(gf256_mul(1, 1), 1);
        assert_eq!(gf256_mul(2, 2), 4);
    }

    /// Canonical ISO/IEC 16022 vector: "123456" → [142,164,186] + [114,25,5,88,102].
    #[test]
    fn test_rs_iso_reference_vector() {
        let mut data = [0u8; MAX_DATA_CW + 1];
        let n = ascii_encode(b"123456", &mut data).unwrap();
        assert_eq!(&data[..n], &[142, 164, 186]);
        let mut ec = [0u8; MAX_EC];
        rs_encode_dm(&data[..n], 5, &mut ec);
        assert_eq!(&ec[..5], &[114, 25, 5, 88, 102]);
    }

    /// Round-trip: encode → invert placement → check RS syndrome → decode ASCII.
    #[test]
    fn test_round_trip_all_sizes() {
        let a62 = [b'A'; 62];
        let inputs: &[&str] = &[
            "A",
            "Hi",
            "12345",
            "HELLO WORLD",
            "f3411c82-1c70-4207-977e-99f5580e7e3b",
            core::str::from_utf8(&a62).unwrap(), // → 32×32 (multi-region)
        ];
        for &input in inputs {
            let mut buf = [false; MAX_CELLS];
            let (size, _) = encode(input, &mut buf);
            let (_, dr, regions, data_cap, ec_count) =
                *SYMBOL_PARAMS.iter().find(|p| p.0 == size).unwrap();

            let mut cw = [0u8; MAX_DATA_CW + MAX_EC];
            recover(&buf, size, dr, regions, &mut cw);
            let (data, rest) = cw.split_at(data_cap);
            let ec = &rest[..ec_count];

            let mut ec_check = [0u8; MAX_EC];
            rs_encode_dm(data, ec_count, &mut ec_check);
            assert_eq!(&ec_check[..ec_count], ec, "RS mismatch ({size}x{size})");

            // Decode ASCII payload and compare to the input bytes.
            let mut dec = [0u8; MAX_DATA_CW * 2];
            let mut dn = 0;
            for &c in data {
                match c {
                    129 => break,
                    1..=128 => {
                        dec[dn] = c - 1;
                        dn += 1;
                    }
                    130..=229 => {
                        let vv = c - 130;
                        dec[dn] = b'0' + vv / 10;
                        dec[dn + 1] = b'0' + vv % 10;
                        dn += 2;
                    }
                    _ => {}
                }
            }
            assert_eq!(&dec[..dn], input.as_bytes(), "round-trip ({size}x{size})");
        }
    }

    /// ECC 200 padding: first codeword 129, then the 253-state sequence — pinned
    /// to the values produced by libdmtx's `dmtxwrite`.
    #[test]
    fn test_padding_253_state() {
        let a50 = [b'A'; 50];
        let mut buf = [false; MAX_CELLS];
        let (size, _) = encode(core::str::from_utf8(&a50).unwrap(), &mut buf);
        assert_eq!(size, 32);
        let mut cw = [0u8; MAX_DATA_CW + MAX_EC];
        recover(&buf, size, 14, 2, &mut cw);
        assert_eq!(
            &cw[50..62],
            &[129, 34, 184, 79, 229, 124, 20, 170, 65, 215, 110, 6]
        );
    }

    /// Multi-region symbols hold much more data than the old 44-codeword cap.
    #[test]
    fn test_large_capacity() {
        let a174 = [b'A'; 174];
        let mut buf = [false; MAX_CELLS];
        assert_eq!(
            encode(core::str::from_utf8(&a174).unwrap(), &mut buf),
            (48, 48)
        );
        // Beyond the largest single-block symbol is rejected cleanly.
        let a200 = [b'A'; 200];
        let mut buf2 = [false; MAX_CELLS];
        assert_eq!(
            DataMatrix::encode_into(core::str::from_utf8(&a200).unwrap(), &mut buf2),
            Err(EncodeError::DataTooLong)
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = DataMatrix::encode("Test").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
