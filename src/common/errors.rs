//! Common error types for barcode encoding.
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::string::String;
use core::fmt;

/// A generic encoding error returned when barcode encoding fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// The input data is invalid for this symbology (e.g., wrong length, unsupported characters).
    InvalidInput(String),
    /// The input data is too long to be encoded.
    DataTooLong,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            EncodeError::DataTooLong => write!(f, "data too long to encode"),
        }
    }
}
