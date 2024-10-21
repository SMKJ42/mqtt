use bytes::{Buf, Bytes};
use disconnect::DisconnectPacket;
use err::{PacketError, PacketErrorKind};
use pingreq::PingReqPacket;
use pingresp::PingRespPacket;
use puback::PubAckPacket;
use pubcomp::PubCompPacket;
use pubrec::PubRecPacket;
use pubrel::PubRelPacket;
use std::fmt::{Debug, Display};
use unsuback::UnsubAckPacket;

pub mod err;
pub mod io;
pub mod v3;

use io::decode_packet_length;
use v3::subscribe::SubscribePacket;
use v3::*;

use conack::ConnAckPacket;
use connect::ConnectPacket;
use publish::PublishPacket;
use suback::SubAckPacket;
use unsubscribe::UnsubscribePacket;

const PACKET_TYPE_BITS: u8 = 0b1111_0000;
const PACKET_FLAG_BITS: u8 = 0b0000_1111;

#[derive(PartialEq, Debug)]
pub enum MqttPacket {
    ConnAck(ConnAckPacket),
    Connect(ConnectPacket),
    Disconnect(DisconnectPacket),
    PinReq(PingReqPacket),
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
    fn decode(mut bytes: Bytes) -> Result<Self, PacketError> {
        let f_header: FixedHeader;

        (f_header, bytes) = FixedHeader::decode(bytes)?;

        return match f_header.type_ {
            PacketType::CONNACK => Ok(Self::ConnAck(ConnAckPacket::decode(bytes)?)),
            PacketType::CONNECT => Ok(Self::Connect(ConnectPacket::decode(bytes)?)),
            PacketType::DISCONNECT => Ok(Self::Disconnect(DisconnectPacket::decode(f_header)?)),
            PacketType::PINGREQ => Ok(Self::PinReq(PingReqPacket::decode(f_header)?)),
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
}

#[derive(Copy, Clone)]
pub struct FixedHeader {
    pub type_: PacketType,
    pub flags: HeaderFlags,
    len: usize,
}

impl FixedHeader {
    pub fn decode(mut bytes: Bytes) -> Result<(Self, Bytes), PacketError> {
        let byte = bytes.get_u8();
        let type_ = PacketType::try_from(byte)?;
        let flags = HeaderFlags::try_from((type_, byte))?;

        let (len, bytes) = decode_packet_length(bytes.clone())?;

        if bytes.remaining() > len {
            return Err(PacketError::new(
                PacketErrorKind::MalformedLength,
                format!(
                    "Actual packet length: {} OVERFLOWED encoded packet length: {len} ",
                    bytes.remaining()
                ),
            ));
        } else if bytes.remaining() < len {
            return Err(PacketError::new(
                PacketErrorKind::MalformedLength,
                format!(
                    "Actual packet length: {} UNDERFLOWED encoded packet length: {len} ",
                    bytes.remaining()
                ),
            ));
        }

        match type_ {
            // Check for packets with fixed length of 2.
            PacketType::CONNACK
            | PacketType::PUBACK
            | PacketType::PUBREC
            | PacketType::PUBREL
            | PacketType::PUBCOMP
            | PacketType::UNSUBACK => {
                if len != 2 {
                    return Err(PacketError::new(
                        PacketErrorKind::MalformedLength,
                        format!("Packet length for packet type {type_} must be 2, instead reveived length: {len}"),
                    ));
                }
            }
            // Check for packets with fixed length of 0.
            PacketType::PINGREQ | PacketType::PINGRESP | PacketType::DISCONNECT => {
                if len != 0 {
                    return Err(PacketError::new(
                        PacketErrorKind::MalformedLength,
                        format!("Packet length for packet type {type_} must be 0, instead received length: {len}"),
                    ));
                }
            }
            // Length for all other packets is variable.
            _ => {}
        }

        return Ok((Self { type_, flags, len }, bytes));
    }

    pub fn set_flags(&mut self, flags: HeaderFlags) {
        self.flags = flags;
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
    type Error = PacketError;
    fn try_from((type_, byte): (PacketType, u8)) -> Result<Self, PacketError> {
        // clean up the unused bits (most significant 4 bits)
        match type_ {
            PacketType::PUBLISH => {
                // all bit values are available to be written to, and no error handling needs to be enforced on the parse.
            }
            PacketType::PUBREL | PacketType::SUBSCRIBE | PacketType::UNSUBSCRIBE => {
                // these packet types all require the flag bits 4 least significant bits to be 0010.
                if byte & PACKET_FLAG_BITS != 2 {
                    return Err(PacketError::new(
                        PacketErrorKind::FlagBits,
                        format!(
                            "Invalid flag bits: {} for packet type: {}, bits must be 0x_2.",
                            byte, type_
                        ),
                    ));
                }
            }
            _ => {
                // all other packets must have flag bits that are equal to 0.
                if byte & PACKET_FLAG_BITS != 0 {
                    return Err(PacketError::new(
                        PacketErrorKind::FlagBits,
                        format!(
                            "Invalid flag bits: {} for packet type: {}, bits must be 0x_0.",
                            byte, type_
                        ),
                    ));
                }
            }
        }
        return Ok(Self { byte });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

impl TryFrom<u8> for PacketType {
    type Error = PacketError;
    fn try_from(value: u8) -> Result<Self, PacketError> {
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
                return Err(PacketError::new(
                    PacketErrorKind::PacketType,
                    format!("Packet type {} is not a valid packet.", value),
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
