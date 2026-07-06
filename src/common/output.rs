//! SVG rendering convenience for the owned [`BarcodeOutput`] (requires `alloc`).
#![forbid(unsafe_code)]

use alloc::string::String;
use core::fmt::Write;

use super::svg;
use super::types::BarcodeOutput;

impl BarcodeOutput {
    /// Render this barcode as an SVG string.
    ///
    /// For linear barcodes the default bar width is 2 px and the height is
    /// determined by [`LinearBarcode::height`](super::types::LinearBarcode).
    /// For matrix barcodes each module is rendered as a 4 × 4 px square.  A
    /// quiet zone is added on every side.  Linear barcodes with a `text` label
    /// render it centered beneath the bars.
    ///
    /// This is a thin `alloc` wrapper over the allocation-free writers in
    /// [`crate::common::svg`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use barcodes::ean_upc::ean13::Ean13;
    /// use barcodes::common::traits::BarcodeEncoder;
    ///
    /// let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
    /// assert!(svg.starts_with("<svg "));
    /// ```
    pub fn to_svg_string(&self) -> String {
        let mut out = String::new();
        // Writing into a String is infallible.
        match self {
            BarcodeOutput::Linear(lb) => {
                let _ = svg::write_linear(&lb.bars, lb.height, &mut out);
                if let Some(ref text) = lb.text {
                    let _ = write_caption(&mut out, &lb.bars, lb.height, text);
                }
            }
            BarcodeOutput::Matrix(mb) => {
                for row in &mb.modules {
                    debug_assert_eq!(row.len(), mb.width);
                }
                let _ = write_matrix(&mut out, mb);
            }
        }
        out
    }
}

/// Append a centered caption to a linear SVG (before the closing `</svg>`).
fn write_caption(out: &mut String, bars: &[bool], height: u32, text: &str) -> core::fmt::Result {
    const BAR_WIDTH: u32 = 2;
    const QUIET: u32 = 10;
    let total_width = bars.len() as u32 * BAR_WIDTH + 2 * QUIET;
    let total_height = height + 2 * QUIET;
    let text_y = total_height - 2;
    // Re-open by trimming the closing tag written by `write_linear`.
    let closing = "</svg>";
    if out.ends_with(closing) {
        out.truncate(out.len() - closing.len());
    }
    write!(
        out,
        r#"<text x="{}" y="{text_y}" font-family="monospace" font-size="12" text-anchor="middle" fill="black">{text}</text></svg>"#,
        total_width / 2,
    )
}

fn write_matrix(out: &mut String, mb: &super::types::MatrixBarcode) -> core::fmt::Result {
    // Flatten the row-major grid into a contiguous slice-free walk.
    const MODULE_SIZE: usize = 4;
    const QUIET: usize = 4 * MODULE_SIZE;
    let px_width = mb.width * MODULE_SIZE + 2 * QUIET;
    let px_height = mb.height * MODULE_SIZE + 2 * QUIET;

    write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px_width}" height="{px_height}" viewBox="0 0 {px_width} {px_height}"><rect width="{px_width}" height="{px_height}" fill="white"/>"#,
    )?;
    for (row_idx, row) in mb.modules.iter().enumerate() {
        for (col_idx, &dark) in row.iter().enumerate() {
            if dark {
                let x = QUIET + col_idx * MODULE_SIZE;
                let y = QUIET + row_idx * MODULE_SIZE;
                write!(
                    out,
                    r#"<rect x="{x}" y="{y}" width="{MODULE_SIZE}" height="{MODULE_SIZE}" fill="black"/>"#,
                )?;
            }
        }
    }
    out.write_str("</svg>")
}
