//! Two-dimensional barcode encoders.
//!
//! - [`pdf417`]    — PDF417 (stacked 2D barcode)
//! - [`datamatrix`] — Data Matrix ECC 200
//! - [`aztec`]     — Aztec Code
#![forbid(unsafe_code)]

pub mod aztec;
pub mod datamatrix;
pub mod pdf417;
