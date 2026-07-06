//! Output types shared by all barcode symbologies.
//!
//! [`Encoded`] is the allocation-free result of
//! [`encode_into`](crate::common::traits::BarcodeEncoder::encode_into): it
//! describes the shape of the symbol written into the caller's buffer.  The
//! owned [`BarcodeOutput`] family is only available with the `alloc` feature.
#![forbid(unsafe_code)]

/// The shape of a barcode written into a caller-provided module buffer.
///
/// The module data itself lives in the caller's `&mut [bool]`; this value only
/// reports how to interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoded {
    /// A one-dimensional barcode occupying `buf[..len]`, one module per entry.
    Linear {
        /// Number of modules written (`true` = dark, `false` = light).
        len: usize,
        /// Recommended render height in modules (display hint only).
        height: u32,
    },
    /// A two-dimensional barcode written row-major into `buf[..width * height]`.
    Matrix {
        /// Number of columns.
        width: usize,
        /// Number of rows.
        height: usize,
    },
}

/// The encoded representation of any barcode.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarcodeOutput {
    /// A one-dimensional (linear) barcode.
    Linear(LinearBarcode),
    /// A two-dimensional (matrix) barcode.
    Matrix(MatrixBarcode),
}

/// An encoded one-dimensional barcode.
///
/// `bars` is a `Vec<bool>` where each element represents one module:
/// `true` = dark bar, `false` = light space.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearBarcode {
    /// Module sequence: `true` = dark, `false` = light.
    pub bars: alloc::vec::Vec<bool>,
    /// Recommended render height in modules (display hint only).
    pub height: u32,
    /// Optional human-readable text shown beneath the barcode.
    pub text: Option<alloc::string::String>,
}

/// An encoded two-dimensional barcode.
///
/// `modules` is row-major: `modules[row][col]` is `true` when the module is dark.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixBarcode {
    /// Row-major grid of modules: `true` = dark, `false` = light.
    pub modules: alloc::vec::Vec<alloc::vec::Vec<bool>>,
    /// Number of columns.
    pub width: usize,
    /// Number of rows.
    pub height: usize,
}

/// Metadata describing a barcode output.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Human-readable symbology name (e.g. `"EAN-13"`).
    pub symbology: alloc::string::String,
    /// Optional version / variant identifier.
    pub version: Option<alloc::string::String>,
}
