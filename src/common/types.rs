//! Output types shared by all barcode symbologies.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{string::String, vec::Vec};

/// The encoded representation of any barcode.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearBarcode {
    /// Module sequence: `true` = dark, `false` = light.
    pub bars: Vec<bool>,
    /// Recommended render height in modules (display hint only).
    pub height: u32,
    /// Optional human-readable text shown beneath the barcode.
    pub text: Option<String>,
}

/// An encoded two-dimensional barcode.
///
/// `modules` is row-major: `modules[row][col]` is `true` when the module is dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixBarcode {
    /// Row-major grid of modules: `true` = dark, `false` = light.
    pub modules: Vec<Vec<bool>>,
    /// Number of columns.
    pub width: usize,
    /// Number of rows.
    pub height: usize,
}

/// Metadata describing a barcode output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Human-readable symbology name (e.g. `"EAN-13"`).
    pub symbology: String,
    /// Optional version / variant identifier.
    pub version: Option<String>,
}
