//! Core trait that every barcode symbology encoder must implement.
#![forbid(unsafe_code)]

use crate::common::errors::EncodeError;
use crate::common::types::Encoded;

/// A trait implemented by every barcode symbology encoder.
///
/// The required method [`encode_into`](Self::encode_into) is allocation-free:
/// it writes the symbol's modules into a caller-provided `&mut [bool]` buffer.
/// With the `alloc` feature the provided [`encode`](Self::encode) method offers
/// an owned-output convenience on top of it.
///
/// # Example (zero-allocation)
///
/// ```rust
/// use barcodes::common::traits::BarcodeEncoder;
/// use barcodes::common::types::Encoded;
/// use barcodes::ean_upc::ean13::Ean13;
///
/// let mut buf = [false; 128];
/// let Encoded::Linear { len, .. } = Ean13::encode_into("5901234123457", &mut buf).unwrap()
/// else { panic!("linear") };
/// let bars = &buf[..len];
/// assert_eq!(bars.len(), 95);
/// ```
pub trait BarcodeEncoder {
    /// The input type accepted by this encoder.
    type Input: ?Sized;

    /// Encode `input`, writing the modules into `buf` and returning an
    /// [`Encoded`] describing the written region.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError::BufferTooSmall`] if `buf` cannot hold the symbol,
    /// or another [`EncodeError`] variant when the input is invalid.
    fn encode_into(input: &Self::Input, buf: &mut [bool]) -> Result<Encoded, EncodeError>;

    /// Return the human-readable name of this symbology (e.g. `"EAN-13"`).
    fn symbology_name() -> &'static str;

    /// Encode `input` into an owned [`BarcodeOutput`](crate::common::types::BarcodeOutput).
    ///
    /// This is a convenience wrapper over [`encode_into`](Self::encode_into)
    /// that grows a heap buffer as needed; it requires the `alloc` feature.
    #[cfg(feature = "alloc")]
    fn encode(input: &Self::Input) -> Result<crate::common::types::BarcodeOutput, EncodeError> {
        use crate::common::types::{BarcodeOutput, LinearBarcode, MatrixBarcode};
        use alloc::{vec, vec::Vec};

        let mut buf: Vec<bool> = vec![false; 128];
        loop {
            match Self::encode_into(input, &mut buf) {
                Ok(Encoded::Linear { len, height }) => {
                    buf.truncate(len);
                    return Ok(BarcodeOutput::Linear(LinearBarcode {
                        bars: buf,
                        height,
                        text: None,
                    }));
                }
                Ok(Encoded::Matrix { width, height }) => {
                    let mut modules: Vec<Vec<bool>> = Vec::with_capacity(height);
                    for row in 0..height {
                        modules.push(buf[row * width..(row + 1) * width].to_vec());
                    }
                    return Ok(BarcodeOutput::Matrix(MatrixBarcode {
                        modules,
                        width,
                        height,
                    }));
                }
                Err(EncodeError::BufferTooSmall) => {
                    let bigger = buf.len().saturating_mul(2);
                    buf.clear();
                    buf.resize(bigger, false);
                }
                Err(e) => return Err(e),
            }
        }
    }
}
