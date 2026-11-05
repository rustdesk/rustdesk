// FIXME(regexident):
// Replace with `bytes::ExactSizeBuf` once merged:
// https://github.com/tokio-rs/bytes/pull/496

use bytes::buf::{Chain, Take};
use bytes::{Bytes, BytesMut};

/// A trait for buffers that know their exact length.
pub trait ExactSizeBuf {
    /// Returns the exact length of the buffer.
    fn len(&self) -> usize;

    /// Returns `true` if the buffer is empty.
    ///
    /// This method has a default implementation using `ExactSizeBuf::len()`,
    /// so you don't need to implement it yourself.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ExactSizeBuf for Bytes {
    #[inline]
    fn len(&self) -> usize {
        Bytes::len(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        Bytes::is_empty(self)
    }
}

impl ExactSizeBuf for BytesMut {
    #[inline]
    fn len(&self) -> usize {
        BytesMut::len(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        BytesMut::is_empty(self)
    }
}

impl ExactSizeBuf for [u8] {
    #[inline]
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        <[u8]>::is_empty(self)
    }
}

impl<T, U> ExactSizeBuf for Chain<T, U>
where
    T: ExactSizeBuf,
    U: ExactSizeBuf,
{
    fn len(&self) -> usize {
        let first_ref = self.first_ref();
        let last_ref = self.last_ref();

        first_ref.len() + last_ref.len()
    }

    fn is_empty(&self) -> bool {
        let first_ref = self.first_ref();
        let last_ref = self.last_ref();

        first_ref.is_empty() && last_ref.is_empty()
    }
}

impl<T> ExactSizeBuf for Take<T>
where
    T: ExactSizeBuf,
{
    fn len(&self) -> usize {
        let inner_ref = self.get_ref();
        let limit = self.limit();

        limit.min(inner_ref.len())
    }

    fn is_empty(&self) -> bool {
        let inner_ref = self.get_ref();
        let limit = self.limit();

        (limit == 0) || inner_ref.is_empty()
    }
}
