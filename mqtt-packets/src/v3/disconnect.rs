use crate::v3::{FixedHeader, PacketType};
use bytes::{BufMut, Bytes, BytesMut};
use mqtt_core::err::{PacketError, PacketErrorKind};

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

    pub fn decode(f_header: FixedHeader) -> Result<Self, PacketError> {
        if f_header.len != 0 {
            return Err(PacketError::new(
                PacketErrorKind::MalformedLength,
                format!("DISCONNECT packet must be of length 0"),
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
mod test {
    use crate::v3::{FixedHeader, MqttPacket};

    use super::DisconnectPacket;

    #[test]
    fn disconnect_serialize_deserialize() {
        let packet = DisconnectPacket::new();
        let mut buf = packet.encode();

        let (f_header, mut buf) = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Disconnect(packet));
    }
}
