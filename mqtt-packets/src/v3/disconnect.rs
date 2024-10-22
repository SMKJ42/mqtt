use bytes::{BufMut, Bytes, BytesMut};

use crate::{
    err::PacketError,
    v3::{FixedHeader, PacketType},
};

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
                crate::err::PacketErrorKind::MalformedLength,
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
    use crate::v3::MqttPacket;

    use super::DisconnectPacket;

    #[test]
    fn disconnect_serialize_deserialize() {
        let packet = DisconnectPacket::new();

        let packet_en = packet.encode();

        let packet_de = MqttPacket::decode(packet_en).expect("Could not decode packet");
        assert_eq!(MqttPacket::Disconnect(packet), packet_de);
    }
}
