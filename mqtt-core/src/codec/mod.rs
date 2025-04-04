use std::sync::Arc;

use bytes::Bytes;
use v4::FixedHeader;

use crate::err::EncodeError;

pub mod v4;

pub trait Encode {
    fn encode(&self) -> Result<Bytes, EncodeError>;
}

impl<T> Encode for Arc<T>
where
    T: Encode,
{
    fn encode(&self) -> Result<Bytes, EncodeError> {
        (**self).encode()
    }
}

pub trait Decode<T, E> {
    fn decode(fixed_header: FixedHeader, bytes: &mut Bytes) -> Result<T, E>;
}
