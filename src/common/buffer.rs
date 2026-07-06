//! Zero-allocation writer for filling a caller-provided module buffer.
#![forbid(unsafe_code)]

use super::errors::EncodeError;

/// A bounds-checked cursor that appends modules into a borrowed `&mut [bool]`.
///
/// Every push validates remaining capacity and returns
/// [`EncodeError::BufferTooSmall`] instead of panicking or allocating, so
/// encoders can stream their output into fixed stack buffers.
pub struct SliceWriter<'a> {
    buf: &'a mut [bool],
    pos: usize,
}

impl<'a> SliceWriter<'a> {
    /// Wrap a caller-provided buffer.
    #[inline]
    pub fn new(buf: &'a mut [bool]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Number of modules written so far.
    #[inline]
    pub fn len(&self) -> usize {
        self.pos
    }

    /// Whether nothing has been written yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pos == 0
    }

    /// Append a single module.
    #[inline]
    pub fn push(&mut self, value: bool) -> Result<(), EncodeError> {
        let slot = self
            .buf
            .get_mut(self.pos)
            .ok_or(EncodeError::BufferTooSmall)?;
        *slot = value;
        self.pos += 1;
        Ok(())
    }

    /// Append `count` copies of `value` (e.g. a wide bar or space).
    #[inline]
    pub fn push_run(&mut self, value: bool, count: usize) -> Result<(), EncodeError> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(EncodeError::BufferTooSmall)?;
        let slice = self
            .buf
            .get_mut(self.pos..end)
            .ok_or(EncodeError::BufferTooSmall)?;
        slice.fill(value);
        self.pos = end;
        Ok(())
    }

    /// Append every module yielded by an iterator.
    #[inline]
    pub fn extend<I: IntoIterator<Item = bool>>(&mut self, iter: I) -> Result<(), EncodeError> {
        for value in iter {
            self.push(value)?;
        }
        Ok(())
    }
}
