//! SVG rendering for [`BarcodeOutput`].
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{format, string::String};

use super::types::BarcodeOutput;

impl BarcodeOutput {
    /// Render this barcode as an SVG string.
    ///
    /// For linear barcodes the default bar width is 2 px and the height is
    /// determined by [`LinearBarcode::height`].  For matrix barcodes each module
    /// is rendered as a 4 × 4 px square.  A 4-module quiet zone is added on
    /// every side.
    ///
    /// # Example
    ///
    /// ```rust
    /// use barcode::ean_upc::ean13::Ean13;
    /// use barcode::common::traits::BarcodeEncoder;
    ///
    /// let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
    /// assert!(svg.starts_with("<svg "));
    /// ```
    pub fn to_svg_string(&self) -> String {
        match self {
            BarcodeOutput::Linear(lb) => render_linear(lb),
            BarcodeOutput::Matrix(mb) => render_matrix(mb),
        }
    }
}

fn render_linear(lb: &super::types::LinearBarcode) -> String {
    const BAR_WIDTH: u32 = 2;
    const QUIET: u32 = 10;

    let total_width = lb.bars.len() as u32 * BAR_WIDTH + 2 * QUIET;
    let total_height = lb.height + 2 * QUIET;

    let mut rects = String::new();
    for (i, &dark) in lb.bars.iter().enumerate() {
        if dark {
            let x = QUIET + i as u32 * BAR_WIDTH;
            rects.push_str(&format!(
                r#"<rect x="{x}" y="{QUIET}" width="{BAR_WIDTH}" height="{}" fill="black"/>"#,
                lb.height,
            ));
        }
    }

    let text_elem = if let Some(ref t) = lb.text {
        let text_y = total_height - 2;
        format!(
            r#"<text x="{}" y="{text_y}" font-family="monospace" font-size="12" text-anchor="middle" fill="black">{t}</text>"#,
            total_width / 2,
        )
    } else {
        String::new()
    };

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="{total_height}" viewBox="0 0 {total_width} {total_height}"><rect width="{total_width}" height="{total_height}" fill="white"/>{rects}{text_elem}</svg>"#,
    )
}

fn render_matrix(mb: &super::types::MatrixBarcode) -> String {
    const MODULE_SIZE: usize = 4;
    const QUIET: usize = 4 * MODULE_SIZE;

    let px_width = mb.width * MODULE_SIZE + 2 * QUIET;
    let px_height = mb.height * MODULE_SIZE + 2 * QUIET;

    let mut rects = String::new();
    for (row_idx, row) in mb.modules.iter().enumerate() {
        for (col_idx, &dark) in row.iter().enumerate() {
            if dark {
                let x = QUIET + col_idx * MODULE_SIZE;
                let y = QUIET + row_idx * MODULE_SIZE;
                rects.push_str(&format!(
                    r#"<rect x="{x}" y="{y}" width="{MODULE_SIZE}" height="{MODULE_SIZE}" fill="black"/>"#,
                ));
            }
        }
    }

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px_width}" height="{px_height}" viewBox="0 0 {px_width} {px_height}"><rect width="{px_width}" height="{px_height}" fill="white"/>{rects}</svg>"#,
    )
}
