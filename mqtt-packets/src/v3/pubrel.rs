use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    err::PacketError,
    v3::{FixedHeader, PacketType},
};

/*
 * A PUBREL Packet is the response to a PUBREC Packet.
 * It is the third packet of the QoS 2 protocol exchange.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct PubRelPacket {
    id: u16,
}

impl PubRelPacket {
    pub fn new(id: u16) -> Self {
        return Self { id };
    }

    pub fn decode(f_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketError> {
        if f_header.len != 2 {
            return Err(PacketError::new(
                crate::err::PacketErrorKind::MalformedLength,
                format!("PUBREL packet must be of length 2"),
            ));
        } else {
            let id = bytes.get_u16();
            return Ok(Self { id });
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        // set packet type as PUBREL, and set the flag bit to 2
        bytes.put_u8(PacketType::PUBREL as u8 | 2);
        bytes.put_u8(2);
        bytes.put_u16(self.id);
        return bytes.into();
    }

    pub fn id(&self) -> u16 {
        return self.id;
    }
}

#[cfg(test)]
mod test {
    use crate::v3::MqttPacket;

    use super::PubRelPacket;

    #[test]
    fn pubrel_serialize_deserialize() {
        let packet = PubRelPacket::new(1234);

        let packet_en = packet.encode();

        let packet_de = MqttPacket::decode(packet_en).expect("Could not decode packet");
        assert_eq!(MqttPacket::PubRel(packet), packet_de);
    }
}
