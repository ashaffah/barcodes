//! GS1 DataBar Omnidirectional (RSS-14) barcode encoder.
//!
//! Encodes a 13- or 14-digit GTIN into the 96-module GS1 DataBar
//! Omnidirectional linear pattern (ISO/IEC 24724).  The element-width
//! generation follows the standard combinatorial algorithm, so the symbol
//! decodes on conforming readers.
#![forbid(unsafe_code)]

use crate::common::{
    buffer::SliceWriter, errors::EncodeError, traits::BarcodeEncoder, types::Encoded,
};

// ---- Tables (ISO/IEC 24724, via zint) --------------------------------------

/// Combinations table `C(n, r)` for n = 0..17, r = 0..5.
const COMBINS: [[u16; 6]; 18] = [
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 2, 1, 1, 1, 1],
    [1, 3, 3, 1, 1, 1],
    [1, 4, 6, 4, 1, 1],
    [1, 5, 10, 10, 5, 1],
    [1, 6, 15, 20, 15, 6],
    [1, 7, 21, 35, 35, 21],
    [1, 8, 28, 56, 70, 56],
    [1, 9, 36, 84, 126, 126],
    [1, 10, 45, 120, 210, 252],
    [1, 11, 55, 165, 330, 462],
    [1, 12, 66, 220, 495, 792],
    [1, 13, 78, 286, 715, 1287],
    [1, 14, 91, 364, 1001, 2002],
    [1, 15, 105, 455, 1365, 3003],
    [1, 16, 120, 560, 1820, 4368],
    [1, 17, 136, 680, 2380, 6188],
];

/// Group value sums: outside groups 0..4, inside groups 5..8.
const G_SUM: [i32; 9] = [0, 161, 961, 2015, 2715, 0, 336, 1036, 1516];
/// t-values (even for outside, odd for inside).
const T_EVEN_ODD: [i32; 9] = [1, 10, 34, 70, 126, 4, 20, 48, 81];
/// Module counts: outside odd, inside odd, outside even, inside even.
const MODULES: [i32; 18] = [12, 10, 8, 6, 4, 5, 7, 9, 11, 4, 6, 8, 10, 12, 10, 8, 6, 4];
/// Widest odd element per group.
const WIDEST: [i32; 9] = [8, 6, 4, 3, 1, 2, 4, 6, 8];
/// Checksum weights.
const CHECKSUM_WEIGHT: [[i32; 8]; 4] = [
    [1, 3, 9, 27, 2, 6, 18, 54],
    [4, 12, 36, 29, 8, 24, 72, 58],
    [16, 48, 65, 37, 32, 17, 51, 74],
    [64, 34, 23, 69, 49, 68, 46, 59],
];
/// Finder patterns (5 elements each, 9 patterns).
const FINDER: [[i32; 5]; 9] = [
    [3, 8, 2, 1, 1],
    [3, 5, 5, 1, 1],
    [3, 3, 7, 1, 1],
    [3, 1, 9, 1, 1],
    [2, 7, 4, 1, 1],
    [2, 5, 6, 1, 1],
    [2, 3, 8, 1, 1],
    [1, 5, 7, 1, 1],
    [1, 3, 9, 1, 1],
];

// ---- Element-width generation ----------------------------------------------

#[inline]
fn combins(n: i32, r: i32) -> i32 {
    if !(0..18).contains(&n) || !(0..6).contains(&r) {
        return 0;
    }
    COMBINS[n as usize][r as usize] as i32
}

/// Generate 4 element widths for `val` (ISO/IEC 24724 Annex B).
fn get_widths(widths: &mut [i32; 4], mut val: i32, mut n: i32, max_width: i32, no_narrow: bool) {
    const ELEMENTS: i32 = 4;
    let mut narrow_mask = 0i32;
    let mut bar = 0;
    while bar < ELEMENTS - 1 {
        let mut elm_width = 1;
        narrow_mask |= 1 << bar;
        let mut sub_val;
        loop {
            sub_val = combins(n - elm_width - 1, ELEMENTS - bar - 2);
            if no_narrow
                && narrow_mask == 0
                && n - elm_width - (ELEMENTS - bar - 1) >= ELEMENTS - bar - 1
            {
                sub_val -= combins(n - elm_width - (ELEMENTS - bar), ELEMENTS - bar - 2);
            }
            if ELEMENTS - bar - 1 > 1 {
                let mut less_val = 0;
                let mut mxw = n - elm_width - (ELEMENTS - bar - 2);
                while mxw > max_width {
                    less_val += combins(n - elm_width - mxw - 1, ELEMENTS - bar - 3);
                    mxw -= 1;
                }
                sub_val -= less_val * (ELEMENTS - 1 - bar);
            } else if n - elm_width > max_width {
                sub_val -= 1;
            }
            val -= sub_val;
            if val < 0 {
                break;
            }
            elm_width += 1;
            narrow_mask &= !(1 << bar);
        }
        val += sub_val;
        n -= elm_width;
        widths[bar as usize] = elm_width;
        bar += 1;
    }
    widths[bar as usize] = n;
}

/// Interleave odd/even element widths into `ret` (8 elements).
fn interleave(
    ret: &mut [i32; 8],
    v_odd: i32,
    v_even: i32,
    n_odd: i32,
    n_even: i32,
    max_width: i32,
    no_narrow: bool,
) {
    let mut odd = [0i32; 4];
    let mut even = [0i32; 4];
    get_widths(&mut odd, v_odd, n_odd, max_width, no_narrow);
    get_widths(&mut even, v_even, n_even, 9 - max_width, !no_narrow);
    for i in 0..4 {
        ret[i << 1] = odd[i];
        ret[(i << 1) + 1] = even[i];
    }
}

/// Determine the group index for a data-character value.
fn group(val: i32, outside: bool) -> usize {
    let end = 8 >> (outside as i32);
    let mut i = if outside { 0 } else { 5 };
    while i < end {
        if val < G_SUM[(i + 1) as usize] {
            return i as usize;
        }
        i += 1;
    }
    i as usize
}

// ---- Public encoder --------------------------------------------------------

/// GS1 DataBar Omnidirectional (RSS-14) encoder.
///
/// # Example
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::gs1::databar::DataBar;
///
/// let mut buf = [false; 128];
/// let Encoded::Linear { len, .. } = DataBar::encode_into("2001234567890", &mut buf).unwrap()
/// else { unreachable!() };
/// assert_eq!(len, 96);
/// ```
pub struct DataBar;

impl BarcodeEncoder for DataBar {
    type Input = str;

    fn encode_into(input: &str, buf: &mut [bool]) -> Result<Encoded, EncodeError> {
        let val = parse(input)?;

        // Left/right pair and four data characters.
        let left_pair = (val / 4_537_077) as i32;
        let right_pair = (val % 4_537_077) as i32;
        let data_char = [
            left_pair / 1597,
            left_pair % 1597,
            right_pair / 1597,
            right_pair % 1597,
        ];

        // Element widths for each data character.
        let mut data_widths = [[0i32; 8]; 4];
        for i in 0..4 {
            let outside = i % 2 == 0;
            let g = group(data_char[i], outside);
            let v = data_char[i] - G_SUM[g];
            let v_div = v / T_EVEN_ODD[g];
            let v_mod = v % T_EVEN_ODD[g];
            let (v_odd, v_even) = if outside {
                (v_div, v_mod)
            } else {
                (v_mod, v_div)
            };
            interleave(
                &mut data_widths[i],
                v_odd,
                v_even,
                MODULES[g],
                MODULES[g + 9],
                WIDEST[g],
                !outside,
            );
        }

        // Checksum → two check characters selecting the finder patterns.
        let mut checksum = 0;
        for i in 0..4 {
            for j in 0..8 {
                checksum += CHECKSUM_WEIGHT[i][j] * data_widths[i][j];
            }
        }
        checksum %= 79;
        if checksum >= 8 {
            checksum += 1;
        }
        if checksum >= 72 {
            checksum += 1;
        }
        let c_left = (checksum / 9) as usize;
        let c_right = (checksum % 9) as usize;

        // Assemble the 46 element widths (guards, data, finders).
        let mut tw = [0i32; 46];
        tw[0] = 1;
        tw[1] = 1;
        tw[44] = 1;
        tw[45] = 1;
        for i in 0..8 {
            tw[i + 2] = data_widths[0][i];
            tw[i + 15] = data_widths[1][7 - i];
            tw[i + 23] = data_widths[3][i];
            tw[i + 36] = data_widths[2][7 - i];
        }
        for i in 0..5 {
            tw[i + 10] = FINDER[c_left][i];
            tw[i + 31] = FINDER[c_right][4 - i];
        }

        // Render: alternate light/dark starting with light (96 modules).
        let mut w = SliceWriter::new(buf);
        let mut dark = false;
        for &width in &tw {
            w.push_run(dark, width as usize)?;
            dark = !dark;
        }

        Ok(Encoded::Linear {
            len: w.len(),
            height: 33,
        })
    }

    fn symbology_name() -> &'static str {
        "GS1 DataBar"
    }
}

// ---- Helpers ---------------------------------------------------------------

/// GS1 mod-10 check digit over `digits` (weights 3,1,3,1,… from the right).
fn gs1_check_digit(digits: &[u8]) -> u8 {
    let mut sum = 0u32;
    for (i, &d) in digits.iter().rev().enumerate() {
        sum += d as u32 * if i % 2 == 0 { 3 } else { 1 };
    }
    ((10 - (sum % 10)) % 10) as u8
}

/// Parse the input into the 13-digit numeric value that RSS-14 encodes.
fn parse(input: &str) -> Result<u64, EncodeError> {
    let t = input.trim();
    if !t.chars().all(|c| c.is_ascii_digit()) {
        return Err(EncodeError::InvalidInput(
            "GS1 DataBar input must contain digits only",
        ));
    }
    let bytes = t.as_bytes();
    let digits13: &[u8] = match bytes.len() {
        13 => bytes,
        14 => {
            let d: [u8; 14] = core::array::from_fn(|i| bytes[i] - b'0');
            if gs1_check_digit(&d[..13]) != d[13] {
                return Err(EncodeError::InvalidInput(
                    "GS1 DataBar check digit mismatch",
                ));
            }
            &bytes[..13]
        }
        _ => {
            return Err(EncodeError::InvalidInput(
                "GS1 DataBar input must be 13 or 14 digits",
            ));
        }
    };
    let mut val = 0u64;
    for &b in digits13 {
        val = val * 10 + (b - b'0') as u64;
    }
    Ok(val)
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_len(input: &str) -> usize {
        let mut buf = [false; 128];
        match DataBar::encode_into(input, &mut buf).unwrap() {
            Encoded::Linear { len, .. } => len,
            _ => panic!("expected linear"),
        }
    }

    #[test]
    fn test_encode_13_digits() {
        assert_eq!(encode_len("2001234567890"), 96);
    }

    #[test]
    fn test_invalid_chars() {
        let mut buf = [false; 128];
        assert!(DataBar::encode_into("200123456789X", &mut buf).is_err());
    }

    #[test]
    fn test_wrong_length() {
        let mut buf = [false; 128];
        assert!(DataBar::encode_into("12345", &mut buf).is_err());
    }

    #[test]
    fn test_symbology_name() {
        assert_eq!(DataBar::symbology_name(), "GS1 DataBar");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_svg_output() {
        let svg = DataBar::encode("2001234567890").unwrap().to_svg_string();
        assert!(svg.starts_with("<svg "));
    }
}
