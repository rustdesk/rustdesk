pub mod exact_size_buf;

use bytes::{Buf, Bytes, BytesMut};

use crate::error::{Error, Result};

pub trait MarshalSize {
    fn marshal_size(&self) -> usize;
}

pub trait Marshal: MarshalSize {
    fn marshal_to(&self, buf: &mut [u8]) -> Result<usize>;

    fn marshal(&self) -> Result<Bytes> {
        let l = self.marshal_size();
        let mut buf = BytesMut::with_capacity(l);
        buf.resize(l, 0);
        let n = self.marshal_to(&mut buf)?;
        if n != l {
            Err(Error::Other(format!(
                "marshal_to output size {n}, but expect {l}"
            )))
        } else {
            Ok(buf.freeze())
        }
    }
}

pub trait Unmarshal: MarshalSize {
    fn unmarshal<B>(buf: &mut B) -> Result<Self>
    where
        Self: Sized,
        B: Buf;
}
