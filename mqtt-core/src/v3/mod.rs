use bytes::Bytes;

mod conack;
mod connect;
mod disconnect;
mod pingreq;
mod pingresp;
mod puback;
mod pubcomp;
mod publish;
mod pubrec;
mod pubrel;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

pub use conack::ConnAckPacket;
pub use connect::{ConnectPacket, Will};
pub use disconnect::DisconnectPacket;
pub use pingreq::PingReqPacket;
pub use pingresp::PingRespPacket;
pub use puback::PubAckPacket;
pub use pubcomp::PubCompPacket;
pub use publish::PublishPacket;
pub use pubrec::PubRecPacket;
pub use pubrel::PubRelPacket;
pub use std::fmt::{Debug, Display};
pub use suback::SubAckPacket;
pub use subscribe::{FilterResult, SubscribePacket};
pub use unsuback::UnsubAckPacket;
pub use unsubscribe::UnsubscribePacket;

use crate::{
    err::{DecodeError, DecodeErrorKind, EncodeError},
    io::decode_packet_length,
};

const PACKET_TYPE_BITS: u8 = 0b1111_0000;
const PACKET_FLAG_BITS: u8 = 0b0000_1111;

pub fn decode_packet(f_header: FixedHeader, buf: &mut Bytes) -> Result<MqttPacket, DecodeError> {
    MqttPacket::decode(f_header, buf)
}

#[derive(PartialEq, Debug, Clone)]
pub enum MqttPacket {
    ConnAck(ConnAckPacket),
    Connect(ConnectPacket),
    Disconnect(DisconnectPacket),
    PingReq(PingReqPacket),
    PingResp(PingRespPacket),
    PubAck(PubAckPacket),
    PubComp(PubCompPacket),
    Publish(PublishPacket),
    PubRec(PubRecPacket),
    PubRel(PubRelPacket),
    SubAck(SubAckPacket),
    Subscribe(SubscribePacket),
    UnsubAck(UnsubAckPacket),
    Unsubscribe(UnsubscribePacket),
}

impl MqttPacket {
    pub fn decode(f_header: FixedHeader, bytes: &mut Bytes) -> Result<Self, DecodeError> {
        return match f_header.type_ {
            PacketType::CONNACK => Ok(Self::ConnAck(ConnAckPacket::decode(bytes)?)),
            PacketType::CONNECT => Ok(Self::Connect(ConnectPacket::decode(bytes)?)),
            PacketType::DISCONNECT => Ok(Self::Disconnect(DisconnectPacket::decode(f_header)?)),
            PacketType::PINGREQ => Ok(Self::PingReq(PingReqPacket::decode(f_header)?)),
            PacketType::PINGRESP => Ok(Self::PingResp(PingRespPacket::decode(f_header)?)),
            PacketType::PUBACK => Ok(Self::PubAck(PubAckPacket::decode(f_header, bytes)?)),
            PacketType::PUBCOMP => Ok(Self::PubComp(PubCompPacket::decode(f_header, bytes)?)),
            PacketType::PUBLISH => Ok(Self::Publish(PublishPacket::decode(f_header, bytes)?)),
            PacketType::PUBREL => Ok(Self::PubRel(PubRelPacket::decode(f_header, bytes)?)),
            PacketType::PUBREC => Ok(Self::PubRec(PubRecPacket::decode(f_header, bytes)?)),
            PacketType::SUBACK => Ok(Self::SubAck(SubAckPacket::decode(bytes)?)),
            PacketType::SUBSCRIBE => Ok(Self::Subscribe(SubscribePacket::decode(bytes)?)),
            PacketType::UNSUBACK => Ok(Self::UnsubAck(UnsubAckPacket::decode(f_header, bytes)?)),
            PacketType::UNSUBSCRIBE => Ok(Self::Unsubscribe(UnsubscribePacket::decode(bytes)?)),
        };
    }

    pub fn encode(&self) -> Result<Bytes, EncodeError> {
        return match self {
            Self::ConnAck(packet) => Ok(packet.encode()),
            Self::Connect(packet) => packet.encode(),
            Self::Disconnect(packet) => Ok(packet.encode()),
            Self::PingReq(packet) => Ok(packet.encode()),
            Self::PingResp(packet) => Ok(packet.encode()),
            Self::PubAck(packet) => Ok(packet.encode()),
            Self::PubComp(packet) => Ok(packet.encode()),
            Self::Publish(packet) => packet.encode(),
            Self::PubRel(packet) => Ok(packet.encode()),
            Self::PubRec(packet) => Ok(packet.encode()),
            Self::SubAck(packet) => packet.encode(),
            Self::Subscribe(packet) => packet.encode(),
            Self::UnsubAck(packet) => Ok(packet.encode()),
            Self::Unsubscribe(packet) => packet.encode(),
        };
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FixedHeader {
    pub type_: PacketType,
    pub flags: HeaderFlags,
    rest_len: usize,
    header_len: usize,
}

impl FixedHeader {
    pub fn decode(bytes: &mut Bytes) -> Result<Self, DecodeError> {
        if bytes.len() == 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::ImproperDisconnect,
                String::from("Received packet of length zero."),
            ));
        }

        let byte = bytes[0];
        let type_ = PacketType::try_from(byte)?;
        let flags = HeaderFlags::try_from((type_, byte))?;

        let (header_len, rest_len) = decode_packet_length(bytes)?;

        return Ok(Self {
            type_,
            flags,
            header_len,
            rest_len,
        });
    }

    pub fn set_flags(&mut self, flags: HeaderFlags) {
        self.flags = flags;
    }

    pub fn header_len(&self) -> usize {
        return self.header_len;
    }

    pub fn rest_len(&self) -> usize {
        return self.rest_len;
    }
}

impl Into<u8> for FixedHeader {
    fn into(self) -> u8 {
        let flags: u8 = self.flags.as_byte();
        let type_: u8 = self.type_ as u8;
        return flags & type_;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct HeaderFlags {
    byte: u8,
}

impl HeaderFlags {
    pub fn as_byte(&self) -> u8 {
        return self.byte;
    }
}

// https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718022
impl TryFrom<(PacketType, u8)> for HeaderFlags {
    type Error = DecodeError;
    fn try_from((type_, byte): (PacketType, u8)) -> Result<Self, DecodeError> {
        // clean up the unused bits (most significant 4 bits)
        match type_ {
            PacketType::PUBLISH => {
                // all bit values are available to be written to, and no error handling needs to be enforced on the parse.
            }
            PacketType::PUBREL | PacketType::SUBSCRIBE | PacketType::UNSUBSCRIBE => {
                // these packet types all require the flag bits 4 least significant bits to be 0010.
                if byte & PACKET_FLAG_BITS != 2 {
                    return Err(DecodeError::new(
                        DecodeErrorKind::FlagBits,
                        format!(
                            "Invalid flag bits: {} for packet type: {}, byte must be == 2 for packet type {type_}.",
                            byte, type_
                        ),
                    ));
                }
            }
            _ => {
                // all other packets must have flag bits that are equal to 0.
                if byte & PACKET_FLAG_BITS != 0 {
                    return Err(DecodeError::new(
                        DecodeErrorKind::FlagBits,
                        format!(
                            "Invalid flag bits: {} for packet type: {}, bits must be == 0 for packet type {type_}.",
                            byte, type_
                        ),
                    ));
                }
            }
        }
        return Ok(Self { byte });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PacketType {
    CONNECT = 0b0001_0000,
    CONNACK = 0b0010_0000,
    PUBLISH = 0b0011_0000,
    PUBACK = 0b0100_0000,
    PUBREC = 0b0101_0000,
    PUBREL = 0b0110_0000,
    PUBCOMP = 0b0111_0000,
    SUBSCRIBE = 0b1000_0000,
    SUBACK = 0b1001_0000,
    UNSUBSCRIBE = 0b1010_0000,
    UNSUBACK = 0b1011_0000,
    PINGREQ = 0b1100_0000,
    PINGRESP = 0b1101_0000,
    DISCONNECT = 0b1110_0000,
}

impl Into<u8> for PacketType {
    fn into(self) -> u8 {
        match self {
            Self::CONNECT => 0b0001_0000,
            Self::CONNACK => 0b0010_0000,
            Self::PUBLISH => 0b0011_0000,
            Self::PUBACK => 0b0100_0000,
            Self::PUBREC => 0b0101_0000,
            Self::PUBREL => 0b0110_0000,
            Self::PUBCOMP => 0b0111_0000,
            Self::SUBSCRIBE => 0b1000_0000,
            Self::SUBACK => 0b1001_0000,
            Self::UNSUBSCRIBE => 0b1010_0000,
            Self::UNSUBACK => 0b1011_0000,
            Self::PINGREQ => 0b1100_0000,
            Self::PINGRESP => 0b1101_0000,
            Self::DISCONNECT => 0b1110_0000,
        }
    }
}

impl TryFrom<u8> for PacketType {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, DecodeError> {
        // we only want to match on the left four bits.

        let out = match value & PACKET_TYPE_BITS {
            0x10 => Self::CONNECT,
            0x20 => Self::CONNACK,
            0x30 => Self::PUBLISH,
            0x40 => Self::PUBACK,
            0x50 => Self::PUBREC,
            0x60 => Self::PUBREL,
            0x70 => Self::PUBCOMP,
            0x80 => Self::SUBSCRIBE,
            0x90 => Self::SUBACK,
            0xA0 => Self::UNSUBSCRIBE,
            0xB0 => Self::UNSUBACK,
            0xC0 => Self::PINGREQ,
            0xD0 => Self::PINGRESP,
            0xE0 => Self::DISCONNECT,
            _ => {
                return Err(DecodeError::new(
                    DecodeErrorKind::PacketType,
                    format!("Packet type {value} is not a valid packet."),
                ))
            }
        };
        return Ok(out);
    }
}

impl Display for PacketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CONNECT => write!(f, "PacketType::CONNECT"),
            Self::CONNACK => write!(f, "PacketType::CONNACK"),
            Self::PUBLISH => write!(f, "PacketType::PUBLISH"),
            Self::PUBACK => write!(f, "PacketType::PUBACK"),
            Self::PUBREC => write!(f, "PacketType::PUBREQ"),
            Self::PUBREL => write!(f, "PacketType::PUBREL"),
            Self::PUBCOMP => write!(f, "PacketType::PUBCOMP"),
            Self::SUBSCRIBE => write!(f, "PacketType::SUBSCRIBE"),
            Self::SUBACK => write!(f, "PacketType::SUBACK"),
            Self::UNSUBSCRIBE => write!(f, "PacketType::UNSUBSCRIBE"),
            Self::UNSUBACK => write!(f, "PacketType::UNSUBACK"),
            Self::PINGREQ => write!(f, "PacketType::PINGREQ"),
            Self::PINGRESP => write!(f, "PacketType::PINGRESP"),
            Self::DISCONNECT => write!(f, "PacketType::DISCONNECT"),
        }
    }
}

#[cfg(test)]
mod packet {
    use bytes::Bytes;

    use super::FixedHeader;

    #[test]
    fn deserialize() {
        let mut bytes = Bytes::from_iter([0b1001_0000, 100]);
        let header = FixedHeader::decode(&mut bytes).expect("Could not decode header.");

        assert_eq!(header.header_len, 2);
        assert_eq!(header.rest_len, 100);
    }
}
