//! Core trait that every barcode symbology encoder must implement.
#![forbid(unsafe_code)]

use crate::common::types::BarcodeOutput;

/// A trait implemented by every barcode symbology encoder.
///
/// # Type Parameters
///
/// - `Input` — the type of data being encoded (e.g. `str`, `[u8]`, a newtype).
/// - `Error` — the error type returned when encoding fails.
///
/// # Example
///
/// ```rust
/// use barcode::common::traits::BarcodeEncoder;
/// use barcode::common::types::BarcodeOutput;
/// use barcode::ean_upc::ean13::Ean13;
///
/// let output = Ean13::encode("5901234123457").unwrap();
/// assert!(matches!(output, BarcodeOutput::Linear(_)));
/// ```
pub trait BarcodeEncoder {
    /// The input type accepted by this encoder.
    type Input: ?Sized;
    /// The error type produced when encoding fails.
    type Error: core::fmt::Display + core::fmt::Debug;

    /// Encode `input` into a [`BarcodeOutput`].
    ///
    /// # Errors
    ///
    /// Returns `Err(Self::Error)` when the input is invalid or cannot be encoded.
    fn encode(input: &Self::Input) -> Result<BarcodeOutput, Self::Error>;

    /// Return the human-readable name of this symbology (e.g. `"EAN-13"`).
    fn symbology_name() -> &'static str;
}
