//! Common foundational types, traits, and utilities shared by all symbology modules.
//!
//! - [`traits`] — the [`BarcodeEncoder`](traits::BarcodeEncoder) trait
//! - [`types`] — shared output types (`BarcodeOutput`, `LinearBarcode`, `MatrixBarcode`, `Metadata`)
//! - [`errors`] — shared error type (`EncodeError`)
//! - [`output`] — SVG rendering helpers
//! - [`image_output`] — image rendering helpers (requires `image` feature)
#![forbid(unsafe_code)]

extern crate alloc;

pub mod errors;
#[cfg(feature = "image")]
pub mod image_output;
pub mod output;
pub mod traits;
pub mod types;
