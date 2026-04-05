//! GS1 barcode encoders.
//!
//! - [`gs1_128`] — GS1-128 (Code 128 with Application Identifiers)
//! - [`databar`] — GS1 DataBar Omnidirectional (GTIN encoding)
#![forbid(unsafe_code)]

pub mod databar;
pub mod gs1_128;
