//! Aztec Code barcode encoder.
//!
//! Aztec Code is a 2D matrix barcode used for transport tickets and other
//! applications.  This encoder uses Binary Shift high-level encoding (universal
//! — any bytes), Reed-Solomon over the Aztec Galois fields, and the standard
//! bull's-eye / mode-message / spiral layout (ISO/IEC 24778), so the output
//! decodes on conforming readers.  Compact (1–4 layers) and full-range
//! (1–12 layers) symbols are supported.
#![forbid(unsafe_code)]

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

// ---- Bounds ----------------------------------------------------------------

const MAX_LAYERS_FULL: usize = 12;
const MAX_BITS: usize = 4096;
const MAX_WORDS: usize = 1024;
const MAX_EC: usize = 512;
const MAX_MATRIX: usize = 67;
/// Largest module count (used to size test buffers).
#[cfg(test)]
const MAX_CELLS: usize = MAX_MATRIX * MAX_MATRIX;

/// Word size (bits per codeword) indexed by layer count.
const WORD_SIZE: [usize; 33] = [
    4, 6, 6, 8, 8, 8, 8, 8, 8, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 12, 12, 12,
    12, 12, 12, 12, 12, 12, 12,
];

// ---- Galois field GF(2^m) --------------------------------------------------

struct Gf {
    exp: [u16; 8192],
    log: [u16; 4096],
}

impl Gf {
    /// Build exp/log tables for GF(2^m) with the given `primitive` and `size`.
    fn new(primitive: u16, size: usize) -> Gf {
        let mut gf = Gf {
            exp: [0; 8192],
            log: [0; 4096],
        };
        let mut x = 1u32;
        for i in 0..size - 1 {
            gf.exp[i] = x as u16;
            gf.log[x as usize] = i as u16;
            x <<= 1;
            if x >= size as u32 {
                x ^= primitive as u32;
            }
        }
        // Duplicate to avoid modulo on multiply.
        for i in 0..size - 1 {
            gf.exp[size - 1 + i] = gf.exp[i];
        }
        gf
    }

    #[inline]
    fn mul(&self, a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
        }
    }
}

/// GF for a given word size (ISO/IEC 24778 / ZXing GenericGF primitives).
fn field_for(word_size: usize) -> Gf {
    match word_size {
        4 => Gf::new(0x13, 16),
        6 => Gf::new(0x43, 64),
        8 => Gf::new(0x12d, 256),
        10 => Gf::new(0x409, 1024),
        _ => Gf::new(0x1069, 4096),
    }
}

/// Reed-Solomon encode: `words[..data_len]` are data, EC written to the next
/// `ec` slots (generator roots a^1..a^ec, generatorBase = 1).
fn rs_encode(gf: &Gf, words: &mut [u16], data_len: usize, ec: usize) {
    // Monic generator g(x) = ∏_{i=1}^{ec} (x - a^i).
    let mut genp = [0u16; MAX_EC + 1];
    genp[0] = 1;
    for i in 0..ec {
        let root = gf.exp[1 + i];
        let cur = i + 1; // current generator length before this multiply
        let mut ng = [0u16; MAX_EC + 1];
        for j in 0..cur {
            ng[j] ^= genp[j];
            ng[j + 1] ^= gf.mul(genp[j], root);
        }
        genp[..cur + 1].copy_from_slice(&ng[..cur + 1]);
    }
    // Synthetic division; remainder is the EC codewords.
    let mut rem = [0u16; MAX_EC];
    #[allow(clippy::needless_range_loop)]
    for i in 0..data_len {
        let factor = words[i] ^ rem[0];
        for k in 0..ec - 1 {
            rem[k] = rem[k + 1];
        }
        rem[ec - 1] = 0;
        if factor != 0 {
            for k in 0..ec {
                rem[k] ^= gf.mul(factor, genp[k + 1]);
            }
        }
    }
    words[data_len..data_len + ec].copy_from_slice(&rem[..ec]);
}

// ---- Bit buffer ------------------------------------------------------------

struct Bits {
    bits: [bool; MAX_BITS],
    len: usize,
}

impl Bits {
    fn new() -> Bits {
        Bits {
            bits: [false; MAX_BITS],
            len: 0,
        }
    }
    fn push(&mut self, b: bool) -> Result<(), EncodeError> {
        *self
            .bits
            .get_mut(self.len)
            .ok_or(EncodeError::DataTooLong)? = b;
        self.len += 1;
        Ok(())
    }
    fn push_bits(&mut self, value: u32, count: u32) -> Result<(), EncodeError> {
        for i in (0..count).rev() {
            self.push((value >> i) & 1 == 1)?;
        }
        Ok(())
    }
}

// ---- Aztec geometry helpers ------------------------------------------------

fn total_bits_in_layer(layers: usize, compact: bool) -> usize {
    ((if compact { 88 } else { 112 }) + 16 * layers) * layers
}

/// Stuff bits into `out`: split into words, avoid all-0 / all-1 words.
fn stuff_bits(input: &Bits, word_size: usize, out: &mut Bits) -> Result<(), EncodeError> {
    let n = input.len;
    let mask = (1u32 << word_size) - 2;
    let mut i = 0isize;
    while (i as usize) < n {
        let mut word = 0u32;
        for j in 0..word_size {
            let idx = i + j as isize;
            if idx as usize >= n || input.bits[idx as usize] {
                word |= 1 << (word_size - 1 - j);
            }
        }
        if word & mask == mask {
            out.push_bits(word & mask, word_size as u32)?;
            i -= 1;
        } else if word & mask == 0 {
            out.push_bits(word | 1, word_size as u32)?;
            i -= 1;
        } else {
            out.push_bits(word, word_size as u32)?;
        }
        i += word_size as isize;
    }
    Ok(())
}

/// Reed-Solomon check-word generation over the message bit stream.
fn generate_check_words(
    input: &Bits,
    total_bits: usize,
    word_size: usize,
    out: &mut Bits,
) -> Result<(), EncodeError> {
    let message_words = input.len / word_size;
    let total_words = total_bits / word_size;
    let gf = field_for(word_size);

    let mut words = [0u16; MAX_WORDS];
    #[allow(clippy::needless_range_loop)]
    for i in 0..message_words {
        let mut v = 0u16;
        for j in 0..word_size {
            if input.bits[i * word_size + j] {
                v |= 1 << (word_size - j - 1);
            }
        }
        words[i] = v;
    }
    rs_encode(&gf, &mut words, message_words, total_words - message_words);

    let start_pad = total_bits % word_size;
    out.push_bits(0, start_pad as u32)?;
    for &w in &words[..total_words] {
        out.push_bits(w as u32, word_size as u32)?;
    }
    Ok(())
}

/// Generate the mode message bits (layers/word count + its own RS).
fn generate_mode_message(
    compact: bool,
    layers: usize,
    message_words: usize,
    out: &mut Bits,
) -> Result<(), EncodeError> {
    let mut m = Bits::new();
    if compact {
        m.push_bits((layers - 1) as u32, 2)?;
        m.push_bits((message_words - 1) as u32, 6)?;
        generate_check_words(&m, 28, 4, out)?;
    } else {
        m.push_bits((layers - 1) as u32, 5)?;
        m.push_bits((message_words - 1) as u32, 11)?;
        generate_check_words(&m, 40, 4, out)?;
    }
    Ok(())
}

// ---- Matrix drawing --------------------------------------------------------

struct Matrix<'a> {
    buf: &'a mut [bool],
    size: usize,
}

impl Matrix<'_> {
    #[inline]
    fn set(&mut self, x: usize, y: usize) {
        self.buf[y * self.size + x] = true;
    }
}

fn draw_bulls_eye(m: &mut Matrix, center: usize, size: usize) {
    let mut i = 0;
    while i < size {
        for j in (center - i)..=(center + i) {
            m.set(j, center - i);
            m.set(j, center + i);
            m.set(center - i, j);
            m.set(center + i, j);
        }
        i += 2;
    }
    m.set(center - size, center - size);
    m.set(center - size + 1, center - size);
    m.set(center - size, center - size + 1);
    m.set(center + size, center - size);
    m.set(center + size, center - size + 1);
    m.set(center + size, center + size - 1);
}

fn draw_mode_message(m: &mut Matrix, compact: bool, size: usize, mode: &Bits) {
    let center = size / 2;
    if compact {
        for i in 0..7 {
            let offset = center - 3 + i;
            if mode.bits[i] {
                m.set(offset, center - 5);
            }
            if mode.bits[i + 7] {
                m.set(center + 5, offset);
            }
            if mode.bits[20 - i] {
                m.set(offset, center + 5);
            }
            if mode.bits[27 - i] {
                m.set(center - 5, offset);
            }
        }
    } else {
        for i in 0..10 {
            let offset = center - 5 + i + i / 5;
            if mode.bits[i] {
                m.set(offset, center - 7);
            }
            if mode.bits[i + 10] {
                m.set(center + 7, offset);
            }
            if mode.bits[29 - i] {
                m.set(offset, center + 7);
            }
            if mode.bits[39 - i] {
                m.set(center - 7, offset);
            }
        }
    }
}

// ---- Public encoder --------------------------------------------------------

/// Aztec Code barcode encoder.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::twod::aztec::Aztec;
///
/// let mut buf = [false; 67 * 67];
/// let Encoded::Matrix { width, height } = Aztec::encode_into("AZTEC", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(width, height);
/// ```
pub struct Aztec;

impl BarcodeEncoder for Aztec {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let data = input.as_bytes();
        if data.is_empty() {
            return Err(EncodeError::InvalidInput("Aztec input must not be empty"));
        }

        // High-level: single Binary Shift run of the whole input (from UPPER).
        let mut bits = Bits::new();
        bits.push_bits(31, 5)?; // B/S latch
        let count = data.len();
        if count <= 31 {
            bits.push_bits(count as u32, 5)?;
        } else {
            bits.push_bits(0, 5)?;
            bits.push_bits((count - 31) as u32, 11)?;
        }
        for &b in data {
            bits.push_bits(b as u32, 8)?;
        }

        // Choose the smallest symbol that fits (compact 1-4, then full 1-12).
        let ecc_bits = bits.len * 23 / 100 + 11; // ~23% ECC
        let total_size_bits = bits.len + ecc_bits;

        let mut compact = true;
        let mut layers = 0;
        let mut word_size = 0;
        let mut total_bits_layer = 0;
        let mut stuffed = Bits::new();
        let mut found = false;
        for i in 0..=(MAX_LAYERS_FULL + 3) {
            compact = i <= 3;
            layers = if compact { i + 1 } else { i };
            if !compact && layers > MAX_LAYERS_FULL {
                break;
            }
            total_bits_layer = total_bits_in_layer(layers, compact);
            if total_size_bits > total_bits_layer {
                continue;
            }
            if word_size != WORD_SIZE[layers] {
                word_size = WORD_SIZE[layers];
                stuffed = Bits::new();
                stuff_bits(&bits, word_size, &mut stuffed)?;
            }
            let usable = total_bits_layer - (total_bits_layer % word_size);
            if compact && stuffed.len > word_size * 64 {
                continue;
            }
            if stuffed.len + ecc_bits <= usable {
                found = true;
                break;
            }
        }
        if !found {
            return Err(EncodeError::DataTooLong);
        }

        // Message bits (data + Reed-Solomon check words) and mode message.
        let mut message = Bits::new();
        generate_check_words(&stuffed, total_bits_layer, word_size, &mut message)?;
        let message_words = stuffed.len / word_size;
        let mut mode = Bits::new();
        generate_mode_message(compact, layers, message_words, &mut mode)?;

        // Allocate the symbol and the alignment map.
        let base = (if compact { 11 } else { 14 }) + layers * 4;
        let mut amap = [0usize; MAX_MATRIX];
        let size;
        if compact {
            size = base;
            for (i, slot) in amap.iter_mut().enumerate().take(base) {
                *slot = i;
            }
        } else {
            size = base + 1 + 2 * ((base / 2 - 1) / 15);
            let orig_center = base / 2;
            let center = size / 2;
            for i in 0..orig_center {
                let new_offset = i + i / 15;
                amap[orig_center - i - 1] = center - new_offset - 1;
                amap[orig_center + i] = center + new_offset + 1;
            }
        }

        let cells = size * size;
        if buf.len() < cells {
            return Err(EncodeError::BufferTooSmall);
        }
        for slot in buf[..cells].iter_mut() {
            *slot = false;
        }
        let mut m = Matrix { buf, size };

        // Draw the data bits in the spiral.
        let mut row_offset = 0;
        for i in 0..layers {
            let row_size = (layers - i) * 4 + if compact { 9 } else { 12 };
            for j in 0..row_size {
                let column_offset = j * 2;
                for k in 0..2 {
                    if message.bits[row_offset + column_offset + k] {
                        m.set(amap[i * 2 + k], amap[i * 2 + j]);
                    }
                    if message.bits[row_offset + row_size * 2 + column_offset + k] {
                        m.set(amap[i * 2 + j], amap[base - 1 - i * 2 - k]);
                    }
                    if message.bits[row_offset + row_size * 4 + column_offset + k] {
                        m.set(amap[base - 1 - i * 2 - k], amap[base - 1 - i * 2 - j]);
                    }
                    if message.bits[row_offset + row_size * 6 + column_offset + k] {
                        m.set(amap[base - 1 - i * 2 - j], amap[i * 2 + k]);
                    }
                }
            }
            row_offset += row_size * 8;
        }

        // Draw the mode message and bull's-eye / reference grid.
        draw_mode_message(&mut m, compact, size, &mode);
        if compact {
            draw_bulls_eye(&mut m, size / 2, 5);
        } else {
            draw_bulls_eye(&mut m, size / 2, 7);
            let mut i = 0;
            let mut j = 0;
            while i < base / 2 - 1 {
                let mut k = (size / 2) & 1;
                while k < size {
                    m.set(size / 2 - j, k);
                    m.set(size / 2 + j, k);
                    m.set(k, size / 2 - j);
                    m.set(k, size / 2 + j);
                    k += 2;
                }
                i += 15;
                j += 16;
            }
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
        assert!(encode("AZTEC", &mut buf) >= 15);
    }

    #[test]
    fn test_encode_longer() {
        let mut buf = [false; MAX_CELLS];
        assert!(encode("Hello, Aztec Code! 1234567890", &mut buf) >= 15);
    }

    #[test]
    fn test_empty_input() {
        let mut buf = [false; MAX_CELLS];
        assert!(Aztec::encode_into("", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Aztec::symbology_name(), "Aztec Code");
    }

    #[test]
    fn test_gf_mul() {
        let gf = field_for(8);
        assert_eq!(gf.mul(0, 5), 0);
        assert_eq!(gf.mul(1, 7), 7);
    }
}
