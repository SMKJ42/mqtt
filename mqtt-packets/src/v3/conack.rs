use crate::v3::{PacketError, PacketErrorKind, PacketType};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/*
 * The CONNACK Packet is the packet sent by the Server in response to a CONNECT Packet received from a Client.
 *  The first packet sent from the Server to the Client MUST be a CONNACK Packet [MQTT-3.2.0-1].
 */
#[derive(PartialEq, Debug, Clone)]
pub struct ConnAckPacket {
    /*
     * Byte 1 is the "Connect Acknowledge Flags". Bits 7-1 are reserved and MUST be set to 0.
     * Bit 0 (SP1) is the Session Present Flag.
     *
     * Position: bit 0 of the Connect Acknowledge Flags.
     *
     * If the Server accepts a connection with CleanSession set to 1, the Server MUST set Session Present to 0
     *  in the CONNACK packet in addition to setting a zero return code in the CONNACK packet [MQTT-3.2.2-1].
     *
     * If the Server accepts a connection with CleanSession set to 0, the value set in Session Present depends
     * on whether the Server already has stored Session state for the supplied client ID. If the Server has
     * stored Session state, it MUST set Session Present to 1 in the CONNACK packet [MQTT-3.2.2-2].
     *
     * If the Server does not have stored Session state, it MUST set Session Present to 0 in the CONNACK packet.
     * This is in addition to setting a zero return code in the CONNACK packet [MQTT-3.2.2-3].
     *
     * The Session Present flag enables a Client to establish whether the Client and Server have a consistent
     * view about whether there is already stored Session state.
     *
     * Once the initial setup of a Session is complete, a Client with stored Session state will expect the
     * Server to maintain its stored Session state. In the event that the value of Session Present received
     * by the Client from the Server is not as expected, the Client can choose whether to proceed with the
     * Session or to disconnect.
     *
     * The Client can discard the Session state on both Client and Server by disconnecting,
     * connecting with Clean Session set to 1 and then disconnecting again.
     *
     * If a server sends a CONNACK packet containing a non-zero return code it MUST set Session Present to 0 [MQTT-3.2.2-4].
     */
    session_present: bool,
    return_code: ConnectReturnCode,
}

impl ConnAckPacket {
    pub fn new(session_present: bool, return_code: ConnectReturnCode) -> Self {
        return Self {
            session_present,
            return_code,
        };
    }

    pub fn decode(bytes: &mut Bytes) -> Result<Self, PacketError> {
        let session_present_byte = bytes.get_u8();

        if (session_present_byte & 0b1111_1110) != 0 {
            return Err(PacketError::new(
                PacketErrorKind::AccessToReservedBit,
                String::from(
                    "Connection acknowledge bytes 1-7 are reserved, only the LSB (the CleanSession flag) can be set.",
                ),
            ));
        }

        let return_code = bytes.get_u8().try_into()?;

        return Ok(Self {
            session_present: session_present_byte != 0,
            return_code,
        });
    }

    pub fn encode(&self) -> Bytes {
        let mut bytes = BytesMut::with_capacity(4);
        // the packet type byte

        bytes.put_u8(PacketType::CONNACK as u8);

        // CONNACK packets have a fixed remaining length of 2.
        bytes.put_u8(2);

        // Encode value for 'session present'.
        if self.session_present {
            bytes.put_u8(1);
        } else {
            bytes.put_u8(0);
        }

        // Encode CONNACK packet's return code.
        bytes.put_u8(self.return_code as u8);

        return bytes.into();
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConnectReturnCode {
    Accept = 0,
    InvalidProtocol = 1,
    IdentifierRejected = 2,
    ServerUnavailable = 3,
    BadUsernameOrPassword = 4,
    NotAuthorized = 5,
}

impl TryFrom<u8> for ConnectReturnCode {
    type Error = PacketError;

    fn try_from(value: u8) -> Result<Self, PacketError> {
        match value {
            0 => return Ok(Self::Accept),
            1 => return Ok(Self::InvalidProtocol),
            2 => return Ok(Self::IdentifierRejected),
            3 => return Ok(Self::ServerUnavailable),
            4 => return Ok(Self::BadUsernameOrPassword),
            5 => return Ok(Self::NotAuthorized),
            _ => {
                return Err(PacketError::new(
                    PacketErrorKind::InvalidReturnCode,
                    format!("Return code: {} is not a valid value.", value),
                ));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::v3::{FixedHeader, MqttPacket};

    use super::ConnAckPacket;

    #[test]
    fn connack_serialize_deserialize() {
        let packet = ConnAckPacket::new(true, super::ConnectReturnCode::Accept);
        let mut buf = packet.encode();

        let (f_header, mut buf) = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::ConnAck(packet));
    }
}
