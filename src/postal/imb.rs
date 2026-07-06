//! USPS Intelligent Mail Barcode (IMb / OneCode) encoder.
//!
//! Encodes a 20-digit tracking code and an optional routing (ZIP) code of 0, 5,
//! 9 or 11 digits into the 65-bar 4-state Intelligent Mail Barcode
//! (USPS-B-3200).  The output is a 3-row matrix: row 0 is the ascender, row 1
//! the tracker (always present), row 2 the descender.
#![forbid(unsafe_code)]

use crate::common::{errors::EncodeError, traits::BarcodeEncoder, types::Encoded};

use super::imb_table::{APPX_D_I, APPX_D_II, APPX_D_IV};

/// 11-bit CRC frame check sequence (USPS-B-3200) over a 13-byte array whose
/// top two bits are zero.
fn crc11(bytes: &[u8; 13]) -> u16 {
    const GEN: u32 = 0x0F35;
    let mut fcs: u32 = 0x07FF;
    // Most-significant byte, skipping the 2 unused top bits.
    let mut data = (bytes[0] as u32) << 5;
    for _ in 2..8 {
        if (fcs ^ data) & 0x400 != 0 {
            fcs = (fcs << 1) ^ GEN;
        } else {
            fcs <<= 1;
        }
        fcs &= 0x7FF;
        data <<= 1;
    }
    // Remaining bytes.
    for &byte in &bytes[1..13] {
        let mut data = (byte as u32) << 3;
        for _ in 0..8 {
            if (fcs ^ data) & 0x400 != 0 {
                fcs = (fcs << 1) ^ GEN;
            } else {
                fcs <<= 1;
            }
            fcs &= 0x7FF;
            data <<= 1;
        }
    }
    fcs as u16
}

/// USPS Intelligent Mail Barcode encoder.
///
/// Input is the 20-digit tracking code optionally followed by `-` and a 0/5/9/11
/// digit routing code, e.g. `"01234567094987654321-01234567891"`.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::postal::imb::Imb;
///
/// let mut buf = [false; 3 * 129];
/// let Encoded::Matrix { width, height } =
///     Imb::encode_into("01234567094987654321-01234567891", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!((width, height), (129, 3));
/// ```
pub struct Imb;

impl BarcodeEncoder for Imb {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        // Split the tracking code from the optional routing (ZIP) code.
        let (tracker, zip) = match input.split_once('-') {
            Some((t, z)) => (t, z),
            None => (input, ""),
        };
        if tracker.len() != 20 || !tracker.bytes().all(|b| b.is_ascii_digit()) {
            return Err(EncodeError::InvalidInput(
                "IMb tracking code must be 20 digits",
            ));
        }
        if tracker.as_bytes()[1] > b'4' {
            return Err(EncodeError::InvalidInput(
                "IMb barcode identifier (2nd digit) must be 0-4",
            ));
        }
        if !matches!(zip.len(), 0 | 5 | 9 | 11) || !zip.bytes().all(|b| b.is_ascii_digit()) {
            return Err(EncodeError::InvalidInput(
                "IMb routing code must be 0, 5, 9 or 11 digits",
            ));
        }
        let tb = tracker.as_bytes();
        let d = |b: u8| (b - b'0') as u128;

        // Step 1: data fields → a single (up to 102-bit) integer.
        let mut accum: u128 = 0;
        for &b in zip.as_bytes() {
            accum = accum * 10 + d(b);
        }
        accum += match zip.len() {
            11 => 1_000_100_001,
            9 => 100_001,
            5 => 1,
            _ => 0,
        };
        accum = accum * 10 + d(tb[0]);
        accum = accum * 5 + d(tb[1]);
        for &b in &tb[2..20] {
            accum = accum * 10 + d(b);
        }

        // Step 2: 11-bit CRC over the 13-byte (104-bit) big-endian form.
        let reg = accum & !(1u128 << 102) & !(1u128 << 103);
        let mut byte_array = [0u8; 13];
        for (i, slot) in byte_array.iter_mut().enumerate() {
            *slot = (reg >> (8 * (12 - i))) as u8;
        }
        let crc = crc11(&byte_array);

        // Step 3: integer → codewords (base 636 then base 1365).
        let mut cw = [0u32; 10];
        cw[9] = (accum % 636) as u32;
        accum /= 636;
        for j in (1..=8).rev() {
            cw[j] = (accum % 1365) as u32;
            accum /= 1365;
        }
        cw[0] = accum as u32;

        // Step 4: fold in the CRC / orientation.
        cw[9] *= 2;
        if crc >= 1024 {
            cw[0] += 659;
        }

        // Step 5: codewords → 13-bit characters (with CRC bit inversion).
        let mut chars = [0u16; 10];
        for (i, c) in chars.iter_mut().enumerate() {
            let v = cw[i] as usize;
            *c = if v < 1287 {
                APPX_D_I[v]
            } else {
                APPX_D_II[v - 1287]
            };
            if crc & (1 << i) != 0 {
                *c = 0x1FFF - *c;
            }
        }

        // Step 6: characters → 65 four-state bars.
        let mut bar_map = [0u8; 130];
        for (i, &c) in chars.iter().enumerate() {
            for j in 0..13 {
                bar_map[(APPX_D_IV[13 * i + j] - 1) as usize] = ((c >> j) & 1) as u8;
            }
        }

        // Render into a 3-row matrix (bar every 2 columns).
        let width = 65 * 2 - 1;
        let cells = 3 * width;
        if buf.len() < cells {
            return Err(EncodeError::BufferTooSmall);
        }
        for slot in buf[..cells].iter_mut() {
            *slot = false;
        }
        for i in 0..65 {
            // state: 0 = full, 1 = ascender, 2 = descender, 3 = tracker.
            let mut state = 0;
            if bar_map[i] == 0 {
                state += 1;
            }
            if bar_map[i + 65] == 0 {
                state += 2;
            }
            let col = i * 2;
            if state == 0 || state == 1 {
                buf[col] = true; // ascender (top row)
            }
            buf[width + col] = true; // tracker (middle row)
            if state == 0 || state == 2 {
                buf[2 * width + col] = true; // descender (bottom row)
            }
        }

        Ok(Encoded::Matrix { width, height: 3 })
    }

    fn symbology_name() -> &'static str {
        "USPS IMb"
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode the 3-row matrix back to the DAFT state string (F/A/D/T).
    fn daft(buf: &[bool], width: usize) -> [u8; 65] {
        let mut out = [b'?'; 65];
        for (i, o) in out.iter_mut().enumerate() {
            let col = i * 2;
            let top = buf[col];
            let bot = buf[2 * width + col];
            *o = match (top, bot) {
                (true, true) => b'F',
                (true, false) => b'A',
                (false, true) => b'D',
                (false, false) => b'T',
            };
        }
        out
    }

    fn encode(input: &str) -> ([u8; 65], usize) {
        let mut buf = [false; 3 * 129];
        match Imb::encode_into(input, &mut buf).unwrap() {
            Encoded::Matrix { width, .. } => (daft(&buf, width), width),
            _ => panic!("expected matrix"),
        }
    }

    /// Canonical USPS-B-3200 example: this input produces this exact DAFT string.
    #[test]
    fn test_daft_reference_vector() {
        let (states, width) = encode("01234567094987654321-01234567891");
        assert_eq!(width, 129);
        assert_eq!(
            &states,
            b"AADTFFDFTDADTAADAATFDTDDAAADDTDTTDAFADADDDTFFFDDTTTADFAAADFTDAADA"
        );
    }

    #[test]
    fn test_no_zip() {
        let mut buf = [false; 3 * 129];
        assert!(Imb::encode_into("01234567094987654321", &mut buf).is_ok());
    }

    #[test]
    fn test_invalid_length() {
        let mut buf = [false; 3 * 129];
        assert!(Imb::encode_into("12345", &mut buf).is_err());
    }

    #[test]
    fn test_invalid_zip() {
        let mut buf = [false; 3 * 129];
        assert!(Imb::encode_into("01234567094987654321-123", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(Imb::symbology_name(), "USPS IMb");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = Imb::encode("01234567094987654321").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
