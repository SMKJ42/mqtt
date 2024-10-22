use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    err::{PacketError, PacketErrorKind},
    v3::MqttPacket,
};

/*
 * MQTT v3.1.1 standard, Remaining length field on the fixed header can be at
 * most 4 bytes.
 */

const MAX_LEN_BYTES: usize = 4;

const MAX_LEN: usize = (128 as u64).pow(4) as usize;

pub fn encode_packet_length(bytes: &mut BytesMut, mut len: usize) -> Result<usize, PacketError> {
    if len >= MAX_LEN {
        return Err(PacketError::new(
            PacketErrorKind::MalformedLength,
            format!(
                "Cannot encode packet, exceeded max length of 128.pow(4) - 1 {}",
                len
            ),
        ));
    }

    let mut num_bytes = 0;

    loop {
        // if bytes + 1 > MAX_LEN_BYTES {
        //     return Ok(bytes);
        // }

        // find the length of the last packet.
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

pub fn encode_utf8(bytes: &mut BytesMut, val: &str) -> Result<(), PacketError> {
    let new_val = val.as_bytes();
    return encode_bytes(bytes, new_val);
}

pub fn encode_bytes(bytes: &mut BytesMut, val: &[u8]) -> Result<(), PacketError> {
    // TODO:
    // casting to 16 will require a check at the end of the packet encoding function to
    // ensure that the payload length matches the input length.
    let len = val.len() as u16;

    bytes.put_slice(&len.to_be_bytes());
    bytes.put_slice(val);

    return Ok(());
}

pub fn decode_utf8(mut bytes: Bytes) -> Result<(String, Bytes), PacketError> {
    let len: u16;
    (len, bytes) = decode_u16_len(bytes)?;

    if len as usize > bytes.len() {
        return Err(PacketError::new(
            PacketErrorKind::MalformedLength,
            format!(
                "u16 length: {len} overflows buf length: {} when parsing packet",
                bytes.len()
            ),
        ));
    }

    let string = String::from_utf8(bytes.slice(0..len as usize).to_vec());

    bytes.advance(len as usize);

    match string {
        Ok(string) => return Ok((string, bytes)),
        Err(_) => {
            return Err(PacketError::new(
                PacketErrorKind::Utf8ParseError,
                "Could not decode UTF-8 string.".to_string(),
            ))
        }
    }
}

pub fn decode_bytes(mut bytes: Bytes) -> Result<(Bytes, Bytes), PacketError> {
    let len: u16;
    (len, bytes) = decode_u16_len(bytes)?;

    if len as usize > bytes.len() {
        return Err(PacketError::new(
            PacketErrorKind::MalformedLength,
            format!(
                "Encoded length: {len} overflows buf length: {} when parsing packet",
                bytes.len()
            ),
        ));
    }

    let slice = bytes.slice(0..len as usize);
    bytes.advance(len as usize);
    return Ok((slice, bytes));
}

pub fn decode_u16_len(mut bytes: Bytes) -> Result<(u16, Bytes), PacketError> {
    let len = bytes.get_u16();

    if len as usize > bytes.len() {
        return Err(PacketError::new(
            PacketErrorKind::MalformedLength,
            format!(
                "Encoded length: {len} overflows buf length: {} when parsing packet",
                bytes.len()
            ),
        ));
    }

    return Ok((len, bytes));
}

pub fn decode_packet_length<'a>(mut bytes: Bytes) -> Result<(usize, Bytes), PacketError> {
    let mut c: u8;
    let mut mult = 1;
    let mut len: usize = 0;

    // We could handle the overflow edge case here, by specifying 0..4
    // However, we would lose context of the error.
    for i in 0..MAX_LEN_BYTES {
        c = bytes[i];

        len += (c as usize & 127) * mult; // Add the 7 least significant bits of c to value
        mult *= 128;

        // check most significant bit (0b1000_0000) for a set flag, if set to zero - break loop, else continue.
        if (c & 128) == 0 {
            bytes.advance(i + 1);
            return Ok((len, bytes));
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
        return Err(PacketError::new(
            PacketErrorKind::MalformedLength,
            "Cannot deserialize packet. Packet length exceeded".to_string(),
        ));
    } else {
        len += (c as usize & 127) * mult;
        bytes.advance(4);

        return Ok((len, bytes));
    }
}

pub fn decode_packet(buf: &[u8]) -> Result<MqttPacket, PacketError> {
    MqttPacket::decode(Bytes::copy_from_slice(buf))
}

// pub fn serialize_packet(packet: MqttPacket, int: u32) -> Vec<u8> {
//     unimplemented!()
// }

#[cfg(test)]
mod io_test {
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
    fn decode_length() {
        let buf: &[u8] = &[255, 255, 255, 127];
        let mut_bytes = BytesMut::from(buf);

        let bytes = Bytes::from(mut_bytes);

        let res = decode_packet_length(bytes);

        assert!(res.is_ok());

        let (len, _bytes) = res.unwrap();
        //2097151
        assert_eq!(len, (128 as usize).pow(4) - 1);

        let buf: &[u8] = &[128, 128, 128, 128];
        let bytes = Bytes::from(buf);

        let out = decode_packet_length(bytes);

        assert!(out.is_err());
    }
}
