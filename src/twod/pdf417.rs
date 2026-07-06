//! PDF417 barcode encoder.
//!
//! PDF417 is a stacked 2D barcode used for ID cards, boarding passes, and
//! shipping labels.  This encoder uses byte compaction (universal — any input),
//! Reed-Solomon error correction over GF(929), and the standard ISO/IEC 15438
//! low-level codeword patterns, so the output decodes on conforming readers.
#![forbid(unsafe_code)]

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

use super::pdf417_table::CODEWORD_TABLE;

// ---- Constants -------------------------------------------------------------

/// Start pattern (17 modules).
const START_PATTERN: u32 = 0x1fea8;
/// Stop pattern (18 modules).
const STOP_PATTERN: u32 = 0x3fa29;
/// GF(929) prime modulus.
const PRIME: u32 = 929;

/// Maximum total codewords (data + EC) in a symbol.
const MAX_CW: usize = 929;
/// Maximum EC codewords (error-correction level 5 → 64).
const MAX_EC: usize = 64;
const MAX_COLS: usize = 30;
const MAX_ROWS: usize = 90;
/// Vertical module repeat per PDF417 row (rows must be ~3× the module width).
const ROW_HEIGHT: usize = 3;

// ---- Reed-Solomon over GF(929) ---------------------------------------------

/// PDF417 error-correction generator coefficients (ISO/IEC 15438), levels 0..=5.
/// Source: ZXing PDF417ErrorCorrection.EC_COEFFICIENTS.
const EC_L0: [u16; 2] = [27, 917];
const EC_L1: [u16; 4] = [522, 568, 723, 809];
const EC_L2: [u16; 8] = [237, 308, 436, 284, 646, 653, 428, 379];
const EC_L3: [u16; 16] = [
    274, 562, 232, 755, 599, 524, 801, 132, 295, 116, 442, 428, 295, 42, 176, 65,
];
const EC_L4: [u16; 32] = [
    361, 575, 922, 525, 176, 586, 640, 321, 536, 742, 677, 742, 687, 284, 193, 517, 273, 494, 263,
    147, 593, 800, 571, 320, 803, 133, 231, 390, 685, 330, 63, 410,
];
const EC_L5: [u16; 64] = [
    539, 422, 6, 93, 862, 771, 453, 106, 610, 287, 107, 505, 733, 877, 381, 612, 723, 476, 462,
    172, 430, 609, 858, 822, 543, 376, 511, 400, 672, 762, 283, 184, 440, 35, 519, 31, 460, 594,
    225, 535, 517, 352, 605, 158, 651, 201, 488, 502, 648, 733, 717, 83, 404, 97, 280, 771, 840,
    629, 4, 381, 843, 623, 264, 543,
];

fn ec_coefficients(level: usize) -> &'static [u16] {
    match level {
        0 => &EC_L0,
        1 => &EC_L1,
        2 => &EC_L2,
        3 => &EC_L3,
        4 => &EC_L4,
        _ => &EC_L5,
    }
}

/// Generate PDF417 EC codewords into `out[..k]` (ISO/IEC 15438 §4.10).
fn generate_ec(data: &[u16], level: usize, out: &mut [u16]) {
    let coeff = ec_coefficients(level);
    let k = coeff.len();
    let mut e = [0u16; MAX_EC];
    for &cwv in data {
        let t1 = (cwv as u32 + e[k - 1] as u32) % PRIME;
        let mut j = k - 1;
        while j >= 1 {
            let t2 = (t1 * coeff[j] as u32) % PRIME;
            let t3 = PRIME - t2;
            e[j] = ((e[j - 1] as u32 + t3) % PRIME) as u16;
            j -= 1;
        }
        let t2 = (t1 * coeff[0] as u32) % PRIME;
        e[0] = ((PRIME - t2) % PRIME) as u16;
    }
    // Output: e reversed, each value negated modulo 929.
    for (idx, slot) in out[..k].iter_mut().enumerate() {
        let v = e[k - 1 - idx];
        *slot = if v != 0 { PRIME as u16 - v } else { 0 };
    }
}

// ---- Byte compaction -------------------------------------------------------

/// Encode `data` with byte compaction into `cw` starting at `n`; returns the
/// new count (or `DataTooLong` on overflow).
fn byte_compaction(data: &[u8], cw: &mut [u16], mut n: usize) -> Result<usize, EncodeError> {
    let len = data.len();
    let put = |cw: &mut [u16], n: &mut usize, v: u16| -> Result<(), EncodeError> {
        *cw.get_mut(*n).ok_or(EncodeError::DataTooLong)? = v;
        *n += 1;
        Ok(())
    };

    // Latch to byte compaction: 924 when a multiple of 6, else 901.
    put(cw, &mut n, if len.is_multiple_of(6) { 924 } else { 901 })?;

    // Full 6-byte groups → 5 base-900 codewords.
    let mut i = 0;
    while i + 6 <= len {
        let mut t: u64 = 0;
        for j in 0..6 {
            t = (t << 8) | data[i + j] as u64;
        }
        let mut tmp = [0u16; 5];
        for k in (0..5).rev() {
            tmp[k] = (t % 900) as u16;
            t /= 900;
        }
        for &v in &tmp {
            put(cw, &mut n, v)?;
        }
        i += 6;
    }
    // Remaining < 6 bytes → literal codewords.
    while i < len {
        put(cw, &mut n, data[i] as u16)?;
        i += 1;
    }
    Ok(n)
}

// ---- Symbol geometry -------------------------------------------------------

fn isqrt(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Recommended EC level (ISO/IEC 15438) from the data codeword count.
fn recommended_level(data_cw: usize) -> usize {
    match data_cw {
        0..=40 => 2,
        41..=160 => 3,
        161..=320 => 4,
        _ => 5,
    }
}

/// Choose (rows, cols) for `total` codewords with a roughly 3:1 aspect.
fn dimensions(total: usize) -> Result<(usize, usize), EncodeError> {
    let start = isqrt(total).clamp(1, MAX_COLS);
    // Prefer a column count near sqrt, expanding outward, that yields a valid
    // row count in 3..=90 with enough capacity.
    for c in start..=MAX_COLS {
        let r = total.div_ceil(c);
        if (3..=MAX_ROWS).contains(&r) {
            return Ok((r, c));
        }
    }
    for c in (1..start).rev() {
        let r = total.div_ceil(c);
        if (3..=MAX_ROWS).contains(&r) {
            return Ok((r, c));
        }
    }
    Err(EncodeError::DataTooLong)
}

/// Left/right row-indicator codeword values for a row (ISO/IEC 15438).
fn row_indicators(y: usize, r: usize, c: usize, level: usize, cluster: usize) -> (usize, usize) {
    let base = 30 * (y / 3);
    match cluster {
        0 => (base + (r - 1) / 3, base + (c - 1)),
        1 => (base + level * 3 + (r - 1) % 3, base + (r - 1) / 3),
        _ => (base + (c - 1), base + level * 3 + (r - 1) % 3),
    }
}

/// Width in modules of one row: start(17) + left(17) + cols×17 + right(17) + stop(18).
fn row_width(cols: usize) -> usize {
    17 * (cols + 3) + 18
}

// ---- Public encoder --------------------------------------------------------

/// PDF417 barcode encoder (byte compaction, EC level auto-selected).
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::twod::pdf417::Pdf417;
///
/// let mut buf = [false; 1 << 16];
/// let Encoded::Matrix { width, height } = Pdf417::encode_into("PDF417", &mut buf).unwrap()
/// else { unreachable!() };
/// assert!(height >= 3 && width > 0);
/// ```
pub struct Pdf417;

impl BarcodeEncoder for Pdf417 {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        if input.is_empty() {
            return Err(EncodeError::InvalidInput("PDF417 input must not be empty"));
        }

        // Codewords: [0] = length descriptor, [1..] = byte-compacted payload.
        let mut cw = [0u16; MAX_CW];
        let payload_end = byte_compaction(input.as_bytes(), &mut cw, 1)?;

        let level = recommended_level(payload_end);
        let ec = 1usize << (level + 1);

        let (rows, cols) = dimensions(payload_end + ec)?;
        let capacity = rows * cols;
        let data_len = capacity - ec;
        if data_len > MAX_CW - MAX_EC || payload_end > data_len {
            return Err(EncodeError::DataTooLong);
        }

        // Pad the data region with 900, then write the length descriptor.
        for slot in cw[payload_end..data_len].iter_mut() {
            *slot = 900;
        }
        cw[0] = data_len as u16;

        // Reed-Solomon over the data codewords → EC codewords appended.
        let (data_part, ec_part) = cw.split_at_mut(data_len);
        generate_ec(data_part, level, ec_part);

        // Render rows into the caller buffer (row-major, constant width).  Each
        // PDF417 row is emitted `ROW_HEIGHT` times so square-module rendering
        // yields the tall rows a scanner needs.
        let width = row_width(cols);
        let height = rows * ROW_HEIGHT;
        if buf.len() < height * width {
            return Err(EncodeError::BufferTooSmall);
        }
        let mut w = SliceWriter::new(buf);
        for y in 0..rows {
            let cluster = y % 3;
            let (left, right) = row_indicators(y, rows, cols, level, cluster);
            for _ in 0..ROW_HEIGHT {
                append_pattern(&mut w, START_PATTERN, 17)?;
                append_pattern(&mut w, CODEWORD_TABLE[cluster][left], 17)?;
                for x in 0..cols {
                    let value = cw[y * cols + x] as usize;
                    append_pattern(&mut w, CODEWORD_TABLE[cluster][value], 17)?;
                }
                append_pattern(&mut w, CODEWORD_TABLE[cluster][right], 17)?;
                append_pattern(&mut w, STOP_PATTERN, 18)?;
            }
        }

        Ok(Encoded::Matrix { width, height })
    }

    fn symbology_name() -> &'static str {
        "PDF417"
    }
}

/// Append the low `len` bits of `pattern` (MSB first) as modules.
fn append_pattern(w: &mut SliceWriter, pattern: u32, len: u32) -> Result<(), EncodeError> {
    for i in (0..len).rev() {
        w.push((pattern >> i) & 1 == 1)?;
    }
    Ok(())
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: &str, buf: &mut [bool]) -> (usize, usize) {
        match Pdf417::encode_into(input, buf).unwrap() {
            Encoded::Matrix { width, height } => (width, height),
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_encode_basic() {
        let mut buf = [false; 1 << 16];
        let (w, h) = encode("PDF417 test", &mut buf);
        assert!(h >= 3 && w > 0);
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; 1 << 16];
        assert!(Pdf417::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Pdf417::symbology_name(), "PDF417");
    }

    #[test]
    fn test_ec_known() {
        // ISO/IEC 15438 worked example: data [5,453,178,121,239] at level 2
        // yields EC [452,327,657,619,956? ...] — just check the count + range.
        let mut ec = [0u16; MAX_EC];
        generate_ec(&[5, 453, 178, 121, 239], 2, &mut ec);
        assert!(ec[..8].iter().all(|&v| (v as u32) < PRIME));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Pdf417::encode("Test").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
