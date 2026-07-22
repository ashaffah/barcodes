//! Allocation-free SVG rendering.
//!
//! These writers stream SVG markup into any [`core::fmt::Write`] sink, so a
//! symbol produced by [`encode_into`](crate::common::traits::BarcodeEncoder::encode_into)
//! can be rendered without touching the heap.  With the `alloc` feature,
//! [`BarcodeOutput::to_svg_string`](crate::common::types::BarcodeOutput) offers
//! a `String`-returning convenience on top of these.
#![forbid(unsafe_code)]

use core::fmt::{self, Write};

use super::types::Encoded;

const BAR_WIDTH: u32 = 2;
const LINEAR_QUIET: u32 = 10;
const MODULE_SIZE: usize = 4;
const MATRIX_QUIET: usize = 4 * MODULE_SIZE;

/// Render the symbol described by `encoded` (whose modules live in `buf`) as SVG.
pub fn write_svg<W: Write>(encoded: Encoded, buf: &[bool], out: &mut W) -> fmt::Result {
    match encoded {
        Encoded::Linear { len, height } => write_linear(&buf[..len], height, out),
        Encoded::Matrix { width, height } => write_matrix(&buf[..width * height], width, out),
    }
}

/// Render a linear barcode (`bars`, one module per entry) as SVG.
pub fn write_linear<W: Write>(bars: &[bool], height: u32, out: &mut W) -> fmt::Result {
    let total_width = bars.len() as u32 * BAR_WIDTH + 2 * LINEAR_QUIET;
    let total_height = height + 2 * LINEAR_QUIET;

    write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="{total_height}" viewBox="0 0 {total_width} {total_height}" style="max-width:100%;height:auto"><rect width="{total_width}" height="{total_height}" fill="white"/>"#,
    )?;
    for (i, &dark) in bars.iter().enumerate() {
        if dark {
            let x = LINEAR_QUIET + i as u32 * BAR_WIDTH;
            write!(
                out,
                r#"<rect x="{x}" y="{LINEAR_QUIET}" width="{BAR_WIDTH}" height="{height}" fill="black"/>"#,
            )?;
        }
    }
    out.write_str("</svg>")
}

/// Render a 2D barcode (`modules`, row-major with `width` columns) as SVG.
pub fn write_matrix<W: Write>(modules: &[bool], width: usize, out: &mut W) -> fmt::Result {
    let rows = modules.len().checked_div(width).unwrap_or(0);
    let px_width = width * MODULE_SIZE + 2 * MATRIX_QUIET;
    let px_height = rows * MODULE_SIZE + 2 * MATRIX_QUIET;

    write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px_width}" height="{px_height}" viewBox="0 0 {px_width} {px_height}" style="max-width:100%;height:auto"><rect width="{px_width}" height="{px_height}" fill="white"/>"#,
    )?;
    for (idx, &dark) in modules.iter().enumerate() {
        if dark {
            let x = MATRIX_QUIET + (idx % width) * MODULE_SIZE;
            let y = MATRIX_QUIET + (idx / width) * MODULE_SIZE;
            write!(
                out,
                r#"<rect x="{x}" y="{y}" width="{MODULE_SIZE}" height="{MODULE_SIZE}" fill="black"/>"#,
            )?;
        }
    }
    out.write_str("</svg>")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `core::fmt::Write` sink backed by a fixed stack buffer — no heap.
    struct FixedWriter {
        buf: [u8; 4096],
        len: usize,
    }
    impl FixedWriter {
        fn new() -> Self {
            Self {
                buf: [0; 4096],
                len: 0,
            }
        }
        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buf[..self.len]).unwrap()
        }
    }
    impl Write for FixedWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let bytes = s.as_bytes();
            let end = self.len + bytes.len();
            if end > self.buf.len() {
                return Err(fmt::Error);
            }
            self.buf[self.len..end].copy_from_slice(bytes);
            self.len = end;
            Ok(())
        }
    }

    #[test]
    fn linear_svg_no_alloc() {
        let bars = [true, false, true, true, false];
        let mut w = FixedWriter::new();
        write_linear(&bars, 50, &mut w).unwrap();
        let svg = w.as_str();
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
        // Intrinsic size (width/height) for a sensible default, plus viewBox and
        // max-width:100% so it also scales down to fit a narrower container.
        assert!(svg.contains(r#"style="max-width:100%;height:auto""#));
        assert!(svg.contains("viewBox="));
        let open_tag = &svg[..svg.find('>').unwrap()];
        assert!(open_tag.contains("width="), "svg tag must set width");
        assert!(open_tag.contains("height="), "svg tag must set height");
    }

    #[test]
    fn matrix_svg_no_alloc() {
        // 2x2: dark on the diagonal.
        let modules = [true, false, false, true];
        let mut w = FixedWriter::new();
        write_matrix(&modules, 2, &mut w).unwrap();
        let svg = w.as_str();
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains(r#"style="max-width:100%;height:auto""#));
    }
}
