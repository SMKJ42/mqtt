use crate::{
    err::{DecodeError, DecodeErrorKind},
    v3::{FixedHeader, PacketType},
};

use bytes::{BufMut, Bytes, BytesMut};

/*
 * A PINGRESP Packet is sent by the Server to the Client in response to a PINGREQ Packet. It indicates that the Server is alive.
 * This Packet is used in Keep Alive processing, see Section 3.1.2.10 for more details.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct PingRespPacket;

impl PingRespPacket {
    pub fn new() -> Self {
        return Self;
    }

    pub fn decode(f_header: FixedHeader) -> Result<Self, DecodeError> {
        if f_header.rest_len != 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedLength,
                String::from("PINGRESP packets can only contain a fixed header."),
            ));
        } else {
            return Ok(Self);
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        bytes.put_u8(PacketType::PINGRESP as u8);
        bytes.put_u8(0);

        return bytes.into();
    }
}

#[cfg(test)]
mod packet {
    use super::PingRespPacket;
    use crate::{
        v3::{FixedHeader, MqttPacket},
        Decode,
    };

    #[test]
    fn serialize_deserialize() {
        let packet = PingRespPacket::new();
        let mut buf = packet.encode();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::PingResp(packet));
    }
}
