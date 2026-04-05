//! Linear (one-dimensional) barcode encoders.
//!
//! - [`code128`] — Code 128 (full ASCII, all three subsets)
//! - [`code39`]  — Code 39 (alphanumeric + special chars)
//! - [`itf`]     — ITF / Interleaved 2 of 5 (numeric pairs)
#![forbid(unsafe_code)]

pub mod code128;
pub mod code39;
pub mod itf;
