//! Linear (one-dimensional) barcode encoders.
//!
//! - [`code128`] — Code 128 (full ASCII, all three subsets)
//! - [`code39`]  — Code 39 (alphanumeric + special chars)
//! - [`code93`]  — Code 93 (alphanumeric + special chars, dual check chars)
//! - [`codabar`] — Codabar / NW-7 (digits + `-$:/.+`)
//! - [`itf`]     — ITF / Interleaved 2 of 5 (numeric pairs)
#![forbid(unsafe_code)]

pub mod codabar;
pub mod code128;
pub mod code39;
pub mod code93;
pub mod itf;
