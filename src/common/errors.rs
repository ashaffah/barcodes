//! Common error types for barcode encoding.
//!
//! The error type is allocation-free: it carries only `Copy` payloads so the
//! zero-allocation core never touches the heap.
#![forbid(unsafe_code)]

use core::fmt;

/// A generic encoding error returned when barcode encoding fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The input data is invalid for this symbology (e.g. wrong length).
    InvalidInput(&'static str),
    /// The input contained a character that is not encodable in this symbology.
    InvalidCharacter(char),
    /// The input data is too long to be encoded.
    DataTooLong,
    /// The caller-provided output buffer is too small for the encoded symbol.
    BufferTooSmall,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            EncodeError::InvalidCharacter(ch) => write!(f, "invalid character: '{ch}'"),
            EncodeError::DataTooLong => write!(f, "data too long to encode"),
            EncodeError::BufferTooSmall => write!(f, "output buffer too small"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}
