use crate::v3::{FixedHeader, PacketType};
use bytes::{BufMut, Bytes, BytesMut};
use mqtt_core::err::{PacketError, PacketErrorKind};

/*
 * The PINGREQ Packet is sent from a Client to the Server. It can be used to:
 *  - Indicate to the Server that the Client is alive in the absence of any other Control Packets being sent from the Client to the Server.
 *  - Request that the Server responds to confirm that it is alive.
 *  - Exercise the network to indicate that the Network Connection is active.
 *
 * This Packet is used in Keep Alive processing, see Section 3.1.2.10 for more details.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct PingReqPacket;

impl PingReqPacket {
    pub fn new() -> Self {
        return Self;
    }

    pub fn decode(f_header: FixedHeader) -> Result<Self, PacketError> {
        if f_header.len != 0 {
            return Err(PacketError::new(
                PacketErrorKind::MalformedLength,
                format!("PINGREQ packet must be of length 0"),
            ));
        } else {
            return Ok(Self);
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        bytes.put_u8(PacketType::PINGREQ as u8);
        bytes.put_u8(0);

        return bytes.into();
    }
}

#[cfg(test)]
mod test {
    use crate::v3::{FixedHeader, MqttPacket};

    use super::PingReqPacket;

    #[test]
    fn pingreq_serialize_deserialize() {
        let packet = PingReqPacket::new();
        let buf = packet.encode();

        let (f_header, buf) = FixedHeader::decode(buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::PingReq(packet));
    }
}
