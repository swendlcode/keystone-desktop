//! A bounds-checked reader for the on-disk peak format.

/// A byte reader that answers `None` instead of panicking, so a truncated cache
/// file is a miss rather than an index-out-of-bounds in a worker thread.
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(super) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    pub(super) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    pub(super) fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    pub(super) fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    pub(super) fn f32s(&mut self, n: usize) -> Option<Vec<f32>> {
        let bytes = self.take(n.checked_mul(4)?)?;
        Some(
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect(),
        )
    }
}
