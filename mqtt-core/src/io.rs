use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::err::{DecodeError, DecodeErrorKind, EncodeError, EncodeErrorKind};

/*
 * MQTT v3.1.1 standard, Remaining length field on the fixed header can be at
 * most 4 bytes.
 */

const MAX_LEN_BYTES: usize = 5;

const MAX_LEN: usize = (128 as u64).pow(4) as usize;

pub fn encode_packet_length(bytes: &mut BytesMut, mut len: usize) -> Result<usize, EncodeError> {
    if len >= MAX_LEN {
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

/// Will NOT advance the internal buffer. To keep alignment with the buffer,
/// the user is responsible for advancing the buffer's pointer.
///
/// ## Returns (var_len, rest_len, buf)
/// where 'var_len' is the length of the encoding, 'rest_len' is the remaining length of the packet and 'buf' returns ownership of the buf
pub fn decode_packet_length<'a>(bytes: &Bytes) -> Result<(usize, usize), DecodeError> {
    let mut c: u8;
    let mut mult = 1;
    let mut len: usize = 0;

    // We could handle the overflow edge case here, by specifying 0..4
    // However, we would lose context of the error.
    for i in 1..MAX_LEN_BYTES {
        c = bytes[i];

        len += (c as usize & 127) * mult; // Add the 7 least significant bits of c to value
        mult *= 128;

        // check most significant bit (0b1000_0000) for a set flag, if set to zero - break loop, else continue.
        if (c & 128) == 0 {
            return Ok((i + 1, len));
        };
    }

    /*
     * https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718023
     * Section 2.2.3
     * Control Packets of size up to 268,435,455 (256 MB). The representation of this number on the wire is: 0xFF, 0xFF, 0xFF, 0x7F.
     *
     * If we hit the following branch of code, we are touching the 4th bit, and need to check the max value.
     */

    let c = bytes[3];

    // MSB of byte siginifies a continuation of encoded length, if MSB is set or we are at max value of bits (i.e. 128), we have exceeded max allowable packet length.
    if c >= 128 {
        return Err(DecodeError::new(
            DecodeErrorKind::MalformedLength,
            format!(
                "Packet payload exceeded max length of 127^4, found length {}",
                len
            ),
        ));
    } else {
        len += (c as usize & 127) * mult;
        return Ok((5, len));
    }
}

// pub fn serialize_packet(packet: MqttPacket, int: u32) -> Vec<u8> {
//     unimplemented!()
// }

#[cfg(test)]
mod header_length {
    use bytes::{Bytes, BytesMut};

    use crate::io::{decode_packet_length, encode_packet_length};

    #[test]
    fn encode_length() {
        let buf: &[u8] = &[0, 0, 0, 0];
        let mut bytes = BytesMut::from(buf);
        let len = (128 as u64).pow(4) as usize - 1;
        let size = encode_packet_length(&mut bytes.clone(), len);

        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 4);

        let len = (128 as u64).pow(4) as usize;
        let size = encode_packet_length(&mut bytes, len);

        assert!(size.is_err())
    }

    #[test]
    fn decode_length_max() {
        let buf: &[u8] = &[0, 255, 255, 255, 127];
        let mut_bytes = BytesMut::from(buf);
        let bytes = Bytes::from(mut_bytes);

        let (encode_len, rest_len) =
            decode_packet_length(&bytes).expect("Error decoding valid length");

        assert_eq!(encode_len, 5);
        assert_eq!(rest_len, (128 as usize).pow(4) - 1);
    }

    #[test]
    fn check_max_len_err() {
        let buf: &[u8] = &[0, 128, 128, 128, 128];
        let bytes = Bytes::from(buf);

        let out = decode_packet_length(&bytes);

        assert!(out.is_err());
    }

    #[test]
    fn check_does_not_peek() {
        let buf: &[u8] = &[0, 127, 128, 128];
        let bytes = Bytes::from(buf);

        let (encode_len, rest_len) =
            decode_packet_length(&bytes).expect("Error decoding valid length");

        assert_eq!(encode_len, 2);
        assert_eq!(rest_len, 127);
    }
}

use futures::FutureExt;
use std::time::Duration;
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::sleep,
};

use crate::{
    err,
    v3::{decode_packet, FixedHeader, MqttPacket},
};

pub async fn read_packet<
    S: AsyncReadExt + AsyncRead + AsyncWriteExt + Unpin,
    E: From<io::Error> + From<err::DecodeError>,
>(
    stream: &mut S,
) -> Result<Option<MqttPacket>, E> {
    let timeout_dur = Duration::from_micros(100);
    let fut1 = sleep(timeout_dur);

    // This is a little hackey, however it does allow us to escape the event loop without having a direct access to a poll function.
    // Primarily useful for TLS stream types where the stream does not have a poll function.
    futures::select! {
        _ = fut1.fuse() => {
            return Ok(None);
        }
        out = unfused_read_packet(stream).fuse() => {
            return out;
        }
    }
}

pub async fn unfused_read_packet<
    S: AsyncReadExt + AsyncWrite + Unpin,
    E: From<io::Error> + From<err::DecodeError>,
>(
    stream: &mut S,
) -> Result<Option<MqttPacket>, E> {
    // read in the packet type and the encoded length.
    let mut header_buf = [0; 5];
    let f_header: FixedHeader;

    // read in packet type.
    header_buf[0] = stream.read_u8().await?;

    // read in encoded packet length.
    let mut i = 1;
    while i < 6 {
        let byte = stream.read_u8().await?;
        header_buf[i] = byte;
        if byte < 128 {
            break;
        }

        i += 1;
    }

    let mut header_buf = Bytes::copy_from_slice(&header_buf[0..i + 1]);
    f_header = FixedHeader::decode(&mut header_buf)?;

    let mut buf = BytesMut::new();
    buf.resize(f_header.rest_len(), 0);

    // extract the variable header and payload then parse the variable header.
    stream.read_exact(&mut buf).await?;
    match decode_packet(f_header, &mut buf.into()) {
        Ok(packet) => {
            return Ok(Some(packet));
        }
        Err(err) => {
            return Err(err.into());
        }
    }
}
