//! Common foundational types, traits, and utilities shared by all symbology modules.
//!
//! - [`traits`] — the [`BarcodeEncoder`](traits::BarcodeEncoder) trait
//! - [`buffer`] — the zero-allocation [`SliceWriter`](buffer::SliceWriter)
//! - [`types`] — output views (`Encoded`) and owned types (behind `alloc`)
//! - [`errors`] — shared error type (`EncodeError`)
//! - [`output`] — SVG rendering helpers
//! - [`image_output`] — image rendering helpers (requires `image` feature)
#![forbid(unsafe_code)]

pub mod buffer;
pub mod errors;
#[cfg(feature = "image")]
pub mod image_output;
#[cfg(feature = "alloc")]
pub mod output;
pub mod svg;
pub mod traits;
pub mod types;
