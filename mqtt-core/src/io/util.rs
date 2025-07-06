use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::err::{DecodeError, DecodeErrorKind, EncodeError, EncodeErrorKind};
/*
 * MQTT v3.1.1 standard, Remaining length field on the fixed header can be at
 * most 4 bytes.
 */

pub const MAX_ENCODED_PACKET_LEN: usize = (128 as u64).pow(4) as usize - 1;

pub fn encode_packet_length(bytes: &mut BytesMut, mut len: usize) -> Result<usize, EncodeError> {
    if len > MAX_ENCODED_PACKET_LEN {
        return Err(EncodeError::new(
            EncodeErrorKind::OversizedPayload,
            format!(
                "Packet payload exceeded max length of 127^4, found length {}",
                len
            ),
        ));
    }

    let mut num_bytes = 0;

    loop {
        let mut d: u8 = (len % 128) as u8;

        len /= 128;

        if len > 0 {
            d |= 128;
        }

        bytes.put_u8(d);

        num_bytes += 1;

        if len == 0 {
            break;
        }
    }

    return Ok(num_bytes);
}

pub fn encode_utf8(bytes: &mut BytesMut, val: &str) -> Result<(), EncodeError> {
    let new_val = val.as_bytes();
    return encode_bytes(bytes, new_val);
}

pub fn encode_bytes(bytes: &mut BytesMut, val: &[u8]) -> Result<(), EncodeError> {
    let len = val.len() as u16;

    bytes.put_slice(&len.to_be_bytes());
    bytes.put_slice(val);

    return Ok(());
}

pub fn decode_utf8(bytes: &mut Bytes) -> Result<String, DecodeError> {
    let len: u16;
    len = decode_u16_len(bytes)?;

    if len as usize > bytes.len() {
        // return Err(DecodeError::MalformedLength);
        return Err(DecodeError::new(DecodeErrorKind::MalformedLength,format!("Attempted invalid memory access, packet remaining length: {}, encoded length: {len}", bytes.len())));
    }

    let string = String::from_utf8(bytes.slice(0..len as usize).to_vec());

    bytes.advance(len as usize);

    match string {
        Ok(string) => return Ok(string),
        Err(e) => {
            return Err(DecodeError::new(
                DecodeErrorKind::Utf8ParseError,
                String::from(e.to_string()),
            ))
        }
    }
}

pub fn decode_bytes(bytes: &mut Bytes) -> Result<Bytes, DecodeError> {
    let len: u16;
    len = decode_u16_len(bytes)?;

    let slice = bytes.slice(0..len as usize);
    bytes.advance(len as usize);
    return Ok(slice);
}

pub fn decode_u16_len(bytes: &mut Bytes) -> Result<u16, DecodeError> {
    let len = bytes.get_u16();

    if len as usize > bytes.len() {
        return Err(DecodeError::new(
            DecodeErrorKind::MalformedLength,
            format!("Attempted invalid memory access, packet remaining length: {}, encoded length: {len}", bytes.len())
        ));
    }

    return Ok(len);
}

#[cfg(test)]
mod header_length {
    use bytes::{Bytes, BytesMut};

    use crate::v4::FixedHeader;

    use super::{encode_packet_length, MAX_ENCODED_PACKET_LEN};

    #[test]
    fn encode_length() {
        let buf: &[u8] = &[0, 0, 0, 0];
        let mut bytes = BytesMut::from(buf);
        let len = MAX_ENCODED_PACKET_LEN;
        let size = encode_packet_length(&mut bytes.clone(), len);

        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 4);

        let len = MAX_ENCODED_PACKET_LEN as usize + 1;
        let size = encode_packet_length(&mut bytes, len);

        assert!(size.is_err())
    }

    #[test]
    fn decode_length() {
        let buf: &[u8] = &[255, 255, 255, 127];
        let mut_bytes = BytesMut::from(buf);
        let mut bytes = Bytes::from(mut_bytes);

        let (encode_len, rest_len) =
            FixedHeader::decode_length(&mut bytes).expect("Error decoding valid length");

        assert_eq!(encode_len, 4);
        assert_eq!(rest_len, MAX_ENCODED_PACKET_LEN);
    }
    #[test]
    fn check_max_len_err() {
        let buf: &[u8] = &[255, 255, 255, 128];
        let mut bytes = Bytes::from(buf);

        let out = FixedHeader::decode_length(&mut bytes);
        assert!(out.is_err());
    }

    #[test]
    fn check_does_not_peek() {
        let buf: &[u8] = &[127, 128, 128];
        let mut bytes = Bytes::from(buf);

        let (encode_len, rest_len) =
            FixedHeader::decode_length(&mut bytes).expect("Error decoding valid length");

        assert_eq!(encode_len, 1);
        assert_eq!(rest_len, 127);
    }
}
