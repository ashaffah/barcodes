//! # barcode
//!
//! A universal bar/QR code generation library supporting many symbologies.
//!
//! ## Modules
//!
//! - [`common`]  — shared traits, types, errors, and SVG output helpers
//! - [`qrcode`]  — QR Code Model 2 encoder
//! - [`ean_upc`] — EAN-13, EAN-8, UPC-A, UPC-E encoders
//! - [`linear`]  — Code 128, Code 39, ITF encoders
//! - [`gs1`]     — GS1-128 and GS1 DataBar encoders
//! - [`twod`]    — PDF417, Data Matrix, Aztec Code encoders
//! - [`postal`]  — USPS IMb and Royal Mail RM4SCC encoders
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod common;
pub mod ean_upc;
pub mod gs1;
pub mod linear;
pub mod postal;
pub mod qrcode;
pub mod twod;
