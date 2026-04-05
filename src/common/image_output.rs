//! Image rendering for [`BarcodeOutput`] and [`QrCode`].
//!
//! This module is only available when the `image` feature is enabled.
//!
//! It provides methods to render barcodes as [`image::GrayImage`] buffers,
//! which can then be encoded to PNG, GIF, or WebP using the [`image`] crate.
#![forbid(unsafe_code)]

use image::{GrayImage, Luma};

use super::types::{BarcodeOutput, LinearBarcode, MatrixBarcode};

const WHITE: Luma<u8> = Luma([255u8]);
const BLACK: Luma<u8> = Luma([0u8]);

impl BarcodeOutput {
    /// Render this barcode as a grayscale image buffer.
    ///
    /// For linear barcodes each bar module is `bar_width` pixels wide and the
    /// height is taken from [`LinearBarcode::height`].  For matrix barcodes each
    /// module is rendered as a `module_size` × `module_size` pixel square.
    /// A quiet zone is added on every side.
    ///
    /// # Example
    ///
    /// ```rust
    /// use barcode::ean_upc::ean13::Ean13;
    /// use barcode::common::traits::BarcodeEncoder;
    ///
    /// let output = Ean13::encode("5901234123457").unwrap();
    /// let img = output.to_image(2);
    /// assert!(img.width() > 0);
    /// assert!(img.height() > 0);
    /// ```
    pub fn to_image(&self, module_size: u32) -> GrayImage {
        assert!(module_size > 0, "module_size must be positive");
        match self {
            BarcodeOutput::Linear(lb) => render_linear_image(lb, module_size),
            BarcodeOutput::Matrix(mb) => render_matrix_image(mb, module_size),
        }
    }
}

fn render_linear_image(lb: &LinearBarcode, module_size: u32) -> GrayImage {
    let quiet = 10 * module_size;
    let width = lb.bars.len() as u32 * module_size + 2 * quiet;
    let height = lb.height * module_size + 2 * quiet;

    let mut img = GrayImage::from_pixel(width, height, WHITE);

    for (i, &dark) in lb.bars.iter().enumerate() {
        if dark {
            let x_start = quiet + i as u32 * module_size;
            for dx in 0..module_size {
                for dy in 0..(lb.height * module_size) {
                    img.put_pixel(x_start + dx, quiet + dy, BLACK);
                }
            }
        }
    }
    img
}

fn render_matrix_image(mb: &MatrixBarcode, module_size: u32) -> GrayImage {
    let quiet = 4 * module_size;
    let width = mb.width as u32 * module_size + 2 * quiet;
    let height = mb.height as u32 * module_size + 2 * quiet;

    let mut img = GrayImage::from_pixel(width, height, WHITE);

    for (row_idx, row) in mb.modules.iter().enumerate() {
        for (col_idx, &dark) in row.iter().enumerate() {
            if dark {
                let x_start = quiet + col_idx as u32 * module_size;
                let y_start = quiet + row_idx as u32 * module_size;
                for dx in 0..module_size {
                    for dy in 0..module_size {
                        img.put_pixel(x_start + dx, y_start + dy, BLACK);
                    }
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::vec;
    use image::Luma;

    use super::*;
    use crate::common::types::{LinearBarcode, MatrixBarcode};

    // ── Linear variant ──────────────────────────────────────────────

    #[test]
    fn linear_to_image_dimensions() {
        let lb = LinearBarcode {
            bars: vec![true, false, true],
            height: 10,
            text: None,
        };
        let output = BarcodeOutput::Linear(lb);
        let module_size: u32 = 2;
        let img = output.to_image(module_size);

        // quiet zone = 10 * module_size = 20
        let expected_width = 3 * module_size + 2 * 10 * module_size; // 6 + 40 = 46
        let expected_height = 10 * module_size + 2 * 10 * module_size; // 20 + 40 = 60
        assert_eq!(img.width(), expected_width);
        assert_eq!(img.height(), expected_height);
    }

    #[test]
    fn linear_to_image_quiet_zone_is_white() {
        let lb = LinearBarcode {
            bars: vec![true, false, true],
            height: 10,
            text: None,
        };
        let output = BarcodeOutput::Linear(lb);
        let img = output.to_image(1);

        // Top-left corner (0,0) is inside the quiet zone and must be white.
        assert_eq!(*img.get_pixel(0, 0), Luma([255u8]));
        // Bottom-right corner is also in the quiet zone.
        assert_eq!(
            *img.get_pixel(img.width() - 1, img.height() - 1),
            Luma([255u8])
        );
    }

    #[test]
    fn linear_to_image_first_dark_bar_is_black() {
        // bars[0] is dark → the pixel at (quiet, quiet) must be black.
        let lb = LinearBarcode {
            bars: vec![true, false, true],
            height: 10,
            text: None,
        };
        let output = BarcodeOutput::Linear(lb);
        let module_size: u32 = 1;
        let img = output.to_image(module_size);

        let quiet = 10 * module_size;
        assert_eq!(*img.get_pixel(quiet, quiet), Luma([0u8]));
    }

    #[test]
    fn linear_to_image_light_bar_stays_white() {
        // bars = [false, true] — the first module is light.
        let lb = LinearBarcode {
            bars: vec![false, true],
            height: 5,
            text: None,
        };
        let output = BarcodeOutput::Linear(lb);
        let module_size: u32 = 2;
        let img = output.to_image(module_size);

        let quiet = 10 * module_size;
        // First module (light) should be white at (quiet, quiet).
        assert_eq!(*img.get_pixel(quiet, quiet), Luma([255u8]));
        // Second module (dark) should be black.
        assert_eq!(*img.get_pixel(quiet + module_size, quiet), Luma([0u8]));
    }

    // ── Matrix variant ──────────────────────────────────────────────

    #[test]
    fn matrix_to_image_dimensions() {
        let mb = MatrixBarcode {
            modules: vec![vec![true, false], vec![false, true]],
            width: 2,
            height: 2,
        };
        let output = BarcodeOutput::Matrix(mb);
        let module_size: u32 = 3;
        let img = output.to_image(module_size);

        // quiet zone = 4 * module_size = 12
        let expected_width = 2 * module_size + 2 * 4 * module_size; // 6 + 24 = 30
        let expected_height = 2 * module_size + 2 * 4 * module_size; // 6 + 24 = 30
        assert_eq!(img.width(), expected_width);
        assert_eq!(img.height(), expected_height);
    }

    #[test]
    fn matrix_to_image_quiet_zone_is_white() {
        let mb = MatrixBarcode {
            modules: vec![vec![true, false], vec![false, true]],
            width: 2,
            height: 2,
        };
        let output = BarcodeOutput::Matrix(mb);
        let img = output.to_image(1);

        // (0,0) is inside the quiet zone.
        assert_eq!(*img.get_pixel(0, 0), Luma([255u8]));
        // Bottom-right corner is also in the quiet zone.
        assert_eq!(
            *img.get_pixel(img.width() - 1, img.height() - 1),
            Luma([255u8])
        );
    }

    #[test]
    fn matrix_to_image_first_dark_module_is_black() {
        // modules[0][0] is dark.
        let mb = MatrixBarcode {
            modules: vec![vec![true, false], vec![false, true]],
            width: 2,
            height: 2,
        };
        let output = BarcodeOutput::Matrix(mb);
        let module_size: u32 = 1;
        let img = output.to_image(module_size);

        let quiet = 4 * module_size;
        assert_eq!(*img.get_pixel(quiet, quiet), Luma([0u8]));
    }

    #[test]
    fn matrix_to_image_light_module_stays_white() {
        // modules[0][1] is light.
        let mb = MatrixBarcode {
            modules: vec![vec![true, false], vec![false, true]],
            width: 2,
            height: 2,
        };
        let output = BarcodeOutput::Matrix(mb);
        let module_size: u32 = 2;
        let img = output.to_image(module_size);

        let quiet = 4 * module_size;
        // Second column, first row → light module, should be white.
        assert_eq!(*img.get_pixel(quiet + module_size, quiet), Luma([255u8]));
    }

    #[test]
    fn matrix_to_image_module_fills_entire_block() {
        // modules[1][1] is dark. With module_size=3, all 9 pixels of that
        // block must be black.
        let mb = MatrixBarcode {
            modules: vec![vec![false, false], vec![false, true]],
            width: 2,
            height: 2,
        };
        let output = BarcodeOutput::Matrix(mb);
        let module_size: u32 = 3;
        let img = output.to_image(module_size);

        let quiet = 4 * module_size;
        let x_start = quiet + module_size;
        let y_start = quiet + module_size;
        for dx in 0..module_size {
            for dy in 0..module_size {
                assert_eq!(
                    *img.get_pixel(x_start + dx, y_start + dy),
                    Luma([0u8]),
                    "pixel ({}, {}) in dark block should be black",
                    x_start + dx,
                    y_start + dy,
                );
            }
        }
    }

    // ── Integration with real encoders ──────────────────────────────

    #[test]
    fn ean13_to_image_produces_valid_image() {
        use crate::common::traits::BarcodeEncoder;
        use crate::ean_upc::ean13::Ean13;

        let output = Ean13::encode("5901234123457").unwrap();
        let img = output.to_image(2);
        assert!(img.width() > 0);
        assert!(img.height() > 0);
        // Quiet zone at (0,0) must be white.
        assert_eq!(*img.get_pixel(0, 0), Luma([255u8]));
    }

    #[test]
    fn datamatrix_to_image_produces_valid_image() {
        use crate::common::traits::BarcodeEncoder;
        use crate::twod::datamatrix::DataMatrix;

        let output = DataMatrix::encode("HELLO").unwrap();
        let img = output.to_image(2);
        assert!(img.width() > 0);
        assert!(img.height() > 0);
        // Quiet zone at (0,0) must be white.
        assert_eq!(*img.get_pixel(0, 0), Luma([255u8]));
    }
}
