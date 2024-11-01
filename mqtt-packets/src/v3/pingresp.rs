use bytes::{BufMut, Bytes, BytesMut};
use mqtt_core::err::{PacketError, PacketErrorKind};

use crate::v3::{FixedHeader, PacketType};

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

    pub fn decode(f_header: FixedHeader) -> Result<Self, PacketError> {
        if f_header.len != 0 {
            return Err(PacketError::new(
                PacketErrorKind::MalformedLength,
                format!("RINGRESP packet must be of length 0"),
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
mod test {
    use crate::v3::{FixedHeader, MqttPacket};

    use super::PingRespPacket;

    #[test]
    fn pingresp_serialize_deserialize() {
        let packet = PingRespPacket::new();
        let buf = packet.encode();

        let (f_header, buf) = FixedHeader::decode(buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::PingResp(packet));
    }
}
