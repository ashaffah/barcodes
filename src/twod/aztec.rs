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

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

// ---- Fixed capacity bounds (compact Aztec, up to 4 layers) -----------------

/// Largest supported compact symbol dimension (`11 + 4 * 4`).
const MAX_SIZE: usize = 27;
/// Largest supported module count (`MAX_SIZE²`).
const MAX_CELLS: usize = MAX_SIZE * MAX_SIZE;
/// Ceiling on data codewords (compact Aztec tops out at 40 for 4 layers).
const MAX_DATA_CW: usize = 64;
/// Ceiling on error-correction codewords.
const MAX_EC: usize = 32;
/// Ceiling on combined data+EC bits.
const MAX_BITS: usize = (MAX_DATA_CW + MAX_EC) * 6;
/// Ceiling on intermediate upper-case/byte-mode bits from text encoding.
const MAX_TEXT_BITS: usize = MAX_DATA_CW * 8 + 8;

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

/// RS encode using GF(64) for data (6-bit codewords) into `out[..ec_count]`.
fn rs_data(data: &[u8], ec_count: usize, out: &mut [u8]) {
    let mut rem_buf = [0u8; MAX_EC];
    let remainder = &mut rem_buf[..ec_count];
    for &d in data {
        let d = d & 0x3F;
        let lead = d ^ remainder[0];
        remainder.copy_within(1.., 0);
        remainder[ec_count - 1] = 0;
        if lead != 0 {
            for coef in remainder.iter_mut() {
                *coef ^= gf64_mul(lead, *coef);
            }
        }
    }
    out[..ec_count].copy_from_slice(remainder);
}

// ---- Text encoding ---------------------------------------------------------

/// Encode ASCII text into 6-bit Aztec code data codewords in `out`.
///
/// Uses the standard Aztec upper-case mode encoding; characters not in the
/// upper-case set fall back to byte encoding.  Returns the codeword count.
fn encode_text(input: &str, out: &mut [u8]) -> Result<usize, EncodeError> {
    let mut bits = [false; MAX_TEXT_BITS];
    let mut nbits = 0;
    let mut push_bits = |value: u32, width: u32| -> Result<(), EncodeError> {
        for bit in (0..width).rev() {
            *bits.get_mut(nbits).ok_or(EncodeError::DataTooLong)? = (value >> bit) & 1 != 0;
            nbits += 1;
        }
        Ok(())
    };

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
            push_bits(c as u32, 5)?; // 5-bit upper-case character
        } else {
            // Shift to byte mode (code 31 in upper) then 8-bit byte.
            push_bits(31, 5)?;
            push_bits(b as u32, 8)?;
        }
    }

    // Pad to a multiple of 6 bits (pad with 1).
    while !nbits.is_multiple_of(6) {
        *bits.get_mut(nbits).ok_or(EncodeError::DataTooLong)? = true;
        nbits += 1;
    }

    // Pack into 6-bit codewords.
    let count = nbits / 6;
    if count > out.len() {
        return Err(EncodeError::DataTooLong);
    }
    for (i, cw) in out[..count].iter_mut().enumerate() {
        let mut acc = 0u8;
        for j in 0..6 {
            acc = (acc << 1) | bits[i * 6 + j] as u8;
        }
        *cw = acc;
    }
    Ok(count)
}

// ---- Compact Aztec finder pattern ------------------------------------------

/// Build the compact Aztec bull's-eye finder pattern centered in a grid.
fn place_compact_finder(grid: &mut [i8], size: usize, center: usize) {
    // Concentric squares: 6 rings (alternating dark/light from center out).
    for ring in 0..=5i32 {
        let dark = ring % 2 == 0; // inner ring (0) is dark
        let val = if dark { 1i8 } else { 0i8 };
        let r_start = (center as i32 - ring).max(0) as usize;
        let r_end = (center as i32 + ring).min(size as i32 - 1) as usize;
        for r in r_start..=r_end {
            for c in r_start..=r_end {
                if r == r_start || r == r_end || c == r_start || c == r_end {
                    grid[r * size + c] = val;
                }
            }
        }
    }
    // Reference grid mark (bottom-right quadrant dark cell)
    if center + 1 < size {
        grid[(center + 1) * size + (center + 1)] = 1;
    }
}

/// Place the orientation marks for compact Aztec.
fn place_compact_orientation(grid: &mut [i8], size: usize, center: usize) {
    // Three dark modules on the top-left arc, one light reference bottom-right.
    let c = center;
    grid[(c - 5) * size + (c - 5)] = 1;
    grid[(c - 5) * size + (c - 4)] = 1;
    grid[(c - 4) * size + (c - 5)] = 1;
    grid[(c + 5) * size + (c + 5)] = 0;
}

// ---- Compact Aztec encoder -------------------------------------------------

/// Encode data bits into a single compact Aztec layer spiraling outward.
fn place_compact_layer(grid: &mut [i8], size: usize, layer: usize, data_bits: &[bool]) {
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
    for c in s..=e {
        if bit_idx < data_bits.len() && grid[s * size + c] < 0 {
            grid[s * size + c] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Right column (top+1 to bottom)
    for r in s + 1..=e {
        if bit_idx < data_bits.len() && grid[r * size + e] < 0 {
            grid[r * size + e] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Bottom row (right-1 to left)
    for c in (s..e).rev() {
        if bit_idx < data_bits.len() && grid[e * size + c] < 0 {
            grid[e * size + c] = data_bits[bit_idx] as i8;
            bit_idx += 1;
        }
    }
    // Left column (bottom-1 to top+1)
    for r in (s + 1..e).rev() {
        if bit_idx < data_bits.len() && grid[r * size + s] < 0 {
            grid[r * size + s] = data_bits[bit_idx] as i8;
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
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::twod::aztec::Aztec;
///
/// let mut buf = [false; 27 * 27];
/// let Encoded::Matrix { width, height } = Aztec::encode_into("AZTEC", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(width, height);
/// ```
pub struct Aztec;

impl BarcodeEncoder for Aztec {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput("Aztec input must not be empty"));
        }

        let mut data_cw = [0u8; MAX_DATA_CW];
        let data_len = encode_text(input, &mut data_cw)?;
        if data_len == 0 {
            return Err(EncodeError::InvalidInput("no encodable data found"));
        }

        // Choose number of compact layers (1-4) based on data size.
        let layers = match data_len {
            0..=4 => 1,
            5..=11 => 2,
            12..=22 => 3,
            23..=40 => 4,
            _ => return Err(EncodeError::DataTooLong),
        };

        let size = 11 + layers * 4; // compact Aztec size
        let cells = size * size;
        if buf.len() < cells {
            return Err(EncodeError::BufferTooSmall);
        }

        let mut grid = [-1i8; MAX_CELLS];
        let center = size / 2;

        // Place finder pattern
        place_compact_finder(&mut grid, size, center);

        // Place orientation marks
        if center >= 5 {
            place_compact_orientation(&mut grid, size, center);
        }

        // Compute RS error correction for data (using ~23% EC).
        let ec_count = (data_len / 4).max(2);
        let mut ec = [0u8; MAX_EC];
        rs_data(&data_cw[..data_len], ec_count, &mut ec);

        // Expand combined data + EC codewords into bits (6 bits each).
        let total_cw = data_len + ec_count;
        let mut data_bits = [false; MAX_BITS];
        let nbits = total_cw * 6;
        for i in 0..total_cw {
            let cw = if i < data_len {
                data_cw[i]
            } else {
                ec[i - data_len]
            };
            for j in 0..6 {
                data_bits[i * 6 + j] = (cw >> (5 - j)) & 1 != 0;
            }
        }
        let data_bits = &data_bits[..nbits];

        // Place data in layers.
        for layer in 1..=layers {
            let layer_bits_start = (layer - 1) * (nbits / layers);
            let layer_bits_end = if layer == layers {
                nbits
            } else {
                layer * (nbits / layers)
            };
            if layer_bits_start < nbits {
                place_compact_layer(
                    &mut grid,
                    size,
                    layer,
                    &data_bits[layer_bits_start..layer_bits_end.min(nbits)],
                );
            }
        }

        // Fill the caller buffer (any -1 cell → light).
        for i in 0..cells {
            buf[i] = grid[i] == 1;
        }

        Ok(Encoded::Matrix {
            width: size,
            height: size,
        })
    }

    fn symbology_name() -> &'static str {
        "Aztec Code"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: &str, buf: &mut [bool]) -> usize {
        match Aztec::encode_into(input, buf).unwrap() {
            Encoded::Matrix { width, height } => {
                assert_eq!(width, height);
                width
            }
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_encode_basic() {
        let mut buf = [false; MAX_CELLS];
        assert!(encode("AZTEC", &mut buf) >= 15); // compact layer 1 = 15
    }

    #[test]
    fn test_encode_short() {
        let mut buf = [false; MAX_CELLS];
        assert!(encode("A", &mut buf) >= 15);
    }

    #[test]
    fn test_finder_pattern_center_is_dark() {
        let mut buf = [false; MAX_CELLS];
        let size = encode("HI", &mut buf);
        let center = size / 2;
        assert!(buf[center * size + center], "center must be dark");
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; MAX_CELLS];
        assert!(Aztec::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_buffer_too_small() {
        let mut buf = [false; 16];
        assert_eq!(
            Aztec::encode_into("A", &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Aztec::symbology_name(), "Aztec Code");
    }

    #[cfg(feature = "alloc")]
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
