//! EAN/UPC family of barcode encoders.
//!
//! - [`ean13`] — EAN-13 (13-digit retail standard)
//! - [`ean8`]  — EAN-8  (compact 8-digit retail)
//! - [`upca`]  — UPC-A  (12-digit North American retail)
//! - [`upce`]  — UPC-E  (compressed 8-digit UPC)
#![forbid(unsafe_code)]

pub mod ean13;
pub mod ean8;
pub mod upca;
pub mod upce;
