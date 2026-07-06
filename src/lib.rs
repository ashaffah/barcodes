//! # barcodes
//!
//! A universal bar/QR code generation library supporting many symbologies.
//!
//! ## Zero-allocation core
//!
//! By default the crate is pure `no_std` and performs **no heap allocation**.
//! Encoders write their module data into a caller-provided `&mut [bool]` buffer
//! via [`BarcodeEncoder::encode_into`](common::traits::BarcodeEncoder::encode_into).
//!
//! Enable the optional `alloc` feature for the convenience
//! [`BarcodeEncoder::encode`](common::traits::BarcodeEncoder::encode) method
//! (returns an owned [`BarcodeOutput`](common::types::BarcodeOutput)) and SVG
//! string rendering.  The `image` feature (implies `std`) adds raster output.
//!
//! ## Modules
//!
//! - [`common`]  — shared traits, types, errors, and output helpers
//! - [`qrcode`]  — QR Code Model 2 encoder
//! - [`ean_upc`] — EAN-13, EAN-8, UPC-A, UPC-E encoders
//! - [`linear`]  — Code 128, Code 39, Code 93, Codabar, ITF encoders
//! - [`gs1`]     — GS1-128 and GS1 DataBar encoders
//! - [`twod`]    — PDF417, Data Matrix, Aztec Code encoders
//! - [`postal`]  — USPS IMb and Royal Mail RM4SCC encoders
#![forbid(unsafe_code)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod common;
pub mod ean_upc;
pub mod gs1;
pub mod linear;
pub mod postal;
pub mod qrcode;
pub mod twod;
