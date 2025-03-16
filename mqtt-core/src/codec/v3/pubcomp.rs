use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    err::{DecodeError, DecodeErrorKind},
    v3::{FixedHeader, PacketType},
};
/*
 * The PUBCOMP Packet is the response to a PUBREL Packet.
 * It is the fourth and final packet of the QoS 2 protocol exchange.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct PubCompPacket {
    id: u16,
}

impl PubCompPacket {
    pub fn new(id: u16) -> Self {
        return Self { id };
    }

    pub fn decode(f_header: FixedHeader, bytes: &mut Bytes) -> Result<Self, DecodeError> {
        if f_header.rest_len != 2 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedLength,
                String::from("PUBCOMP packets can only contain a packet id."),
            ));
        } else {
            let id = bytes.get_u16();
            return Ok(Self { id });
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        bytes.put_u8(PacketType::PUBCOMP as u8);
        bytes.put_u8(2);
        bytes.put_u16(self.id);
        return bytes.into();
    }

    pub fn id(&self) -> u16 {
        return self.id;
    }
}

#[cfg(test)]
mod packet {
    use super::PubCompPacket;
    use crate::{
        v3::{FixedHeader, MqttPacket},
        Decode,
    };
    use bytes::Buf;

    #[test]
    fn serialize_deserialize() {
        let packet = PubCompPacket::new(1234);
        let mut buf = packet.encode();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::PubComp(packet));
    }
}
