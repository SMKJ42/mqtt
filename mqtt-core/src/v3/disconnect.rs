use crate::{
    err::{DecodeError, DecodeErrorKind},
    v3::{FixedHeader, PacketType},
};
use bytes::{BufMut, Bytes, BytesMut};

/*
 * The DISCONNECT Packet is the final Control Packet sent from the Client to the Server.
 * It indicates that the Client is disconnecting cleanly.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct DisconnectPacket;

impl DisconnectPacket {
    pub fn new() -> Self {
        return Self;
    }

    pub fn decode(f_header: FixedHeader) -> Result<Self, DecodeError> {
        if f_header.rest_len != 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedLength,
                String::from("DISCONNECT packets can only contain a fixed header."),
            ));
        } else {
            return Ok(Self);
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        bytes.put_u8(PacketType::DISCONNECT as u8);
        bytes.put_u8(0);

        return bytes.into();
    }
}

#[cfg(test)]
mod packet {
    use super::DisconnectPacket;
    use crate::v3::{FixedHeader, MqttPacket};
    use bytes::Buf;

    #[test]
    fn serialize_deserialize() {
        let packet = DisconnectPacket::new();
        let mut buf = packet.encode();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Disconnect(packet));
    }
}
