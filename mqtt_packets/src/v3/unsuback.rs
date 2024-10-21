use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{err::PacketError, FixedHeader, PacketType};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct UnsubAckPacket {
    id: u16,
}

impl UnsubAckPacket {
    pub fn new(id: u16) -> Self {
        return Self { id };
    }

    pub fn decode(f_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketError> {
        if f_header.len != 2 {
            return Err(PacketError::new(
                crate::err::PacketErrorKind::MalformedLength,
                format!("UNSUBACK packet must be of length 2"),
            ));
        } else {
            let id = bytes.get_u16();
            return Ok(Self { id });
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        bytes.put_u8(PacketType::UNSUBACK as u8);
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
    use crate::MqttPacket;

    use super::UnsubAckPacket;

    #[test]
    fn unsuback_serialize_deserialize() {
        let packet = UnsubAckPacket::new(1234);

        let packet_en = packet.encode();

        let packet_de = MqttPacket::decode(packet_en).expect("Could not decode packt");
        assert_eq!(MqttPacket::UnsubAck(packet), packet_de);
    }
}
