use bytes::{Buf, Bytes};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

mod connack;
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

pub use connack::ConnAckPacket;
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
pub use subscribe::SubscribePacket;
pub use unsuback::UnsubAckPacket;
pub use unsubscribe::UnsubscribePacket;

use crate::err;
use crate::{
    err::{DecodeError, DecodeErrorKind, EncodeError},
    io::MAX_ENCODED_PACKET_LEN,
};

use super::{Decode, Encode};

const PACKET_TYPE_BITS: u8 = 0b1111_0000;
const PACKET_FLAG_BITS: u8 = 0b0000_1111;

pub async fn decode_mqtt_packet<S, E>(
    stream: &mut S,
) -> Result<Option<(FixedHeader, MqttPacket)>, E>
where
    S: AsyncBufRead + Unpin,
    E: From<err::DecodeError>,
{
    let n: &[u8] = stream
        .fill_buf()
        .await
        .map_err(|e| DecodeError::new(DecodeErrorKind::StreamRead, e.to_string()))?;

    // if there are no bytes to read, return none and consume no bytes.
    if n.len() == 0 {
        return Ok(None);
    }

    let mut bytes = Bytes::from(n.to_owned());

    let f_header = decode_header(&mut bytes).await?;
    // if the stream is still waiting for more bytes, return none and consume no bytes.
    if bytes.remaining() < f_header.rest_len() {
        return Ok(None);
    }

    let _ = bytes.split_off(f_header.rest_len());

    match MqttPacket::decode(f_header, &mut bytes) {
        Ok(packet) => {
            return Ok(Some((f_header, packet)));
        }
        Err(err) => {
            return Err(err.into());
        }
    }
}

pub async fn decode_header(bytes: &mut Bytes) -> Result<FixedHeader, DecodeError> {
    return FixedHeader::decode(bytes);
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
    pub fn encode(&self) -> Result<Bytes, EncodeError> {
        return match self {
            Self::ConnAck(packet) => Ok(packet.encode()),
            Self::Disconnect(packet) => Ok(packet.encode()),
            Self::PingReq(packet) => Ok(packet.encode()),
            Self::PingResp(packet) => Ok(packet.encode()),
            Self::PubAck(packet) => Ok(packet.encode()),
            Self::PubComp(packet) => Ok(packet.encode()),
            Self::PubRel(packet) => Ok(packet.encode()),
            Self::PubRec(packet) => Ok(packet.encode()),
            Self::UnsubAck(packet) => Ok(packet.encode()),
            Self::Unsubscribe(packet) => packet.encode(),
            Self::Connect(packet) => packet.encode(),
            Self::Subscribe(packet) => packet.encode(),
            Self::Publish(packet) => packet.encode(),
            Self::SubAck(packet) => packet.encode(),
        };
    }
}

impl Decode<MqttPacket, DecodeError> for MqttPacket {
    fn decode(f_header: FixedHeader, bytes: &mut Bytes) -> Result<Self, DecodeError> {
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

        let type_byte = bytes.get_u8();
        let type_ = PacketType::try_from(type_byte)?;
        let flags = HeaderFlags::try_from((type_, type_byte))?;
        let (len_len, rest_len) = Self::decode_length(bytes)?;

        return Ok(Self {
            type_,
            flags,
            // encoded length + 1 to account for the packet type byte.
            header_len: len_len + 1,
            rest_len,
        });
    }

    /*
     * https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718023
     * Section 2.2.3
     * Control Packets of size up to 268,435,455 (256 MB). The representation of this number on the wire is: 0xFF, 0xFF, 0xFF, 0x7F.
     */

    /// Will NOT advance the internal buffer. To keep alignment with the buffer,
    /// the user is responsible for advancing the buffer's pointer.
    ///
    /// ## Returns (length_of_encoded_length, rest_len)
    pub fn decode_length(bytes: &mut Bytes) -> Result<(usize, usize), DecodeError> {
        let mut mult = 1;
        let mut len: usize = 0;
        let mut i = 0;
        let mut last = &0;

        for c in bytes.iter().take(4) {
            len += (*c as usize & 127) * mult; // Add the 7 least significant bits of c to value
            mult *= 128;
            i += 1;
            last = c;
            // check most significant bit (0b1000_0000) for a set flag, if set to zero - break loop, else continue.

            if (c & 128) == 0 {
                break;
            };
        }

        if len > MAX_ENCODED_PACKET_LEN || *last > 127 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedLength,
                format!(
                    "Packet payload exceeded max length of 127^4, found length {}",
                    len
                ),
            ));
        };

        bytes.advance(i);

        return Ok((i, len));
    }

    pub fn set_flags(&mut self, flags: HeaderFlags) {
        self.flags = flags;
    }

    pub(crate) fn header_len(&self) -> usize {
        return self.header_len;
    }

    pub(crate) fn rest_len(&self) -> usize {
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

const CONNECT: u8 = 0b0001_0000;
const CONNACK: u8 = 0b0010_0000;
const PUBLISH: u8 = 0b0011_0000;
const PUBACK: u8 = 0b0100_0000;
const PUBREC: u8 = 0b0101_0000;
const PUBREL: u8 = 0b0110_0000;
const PUBCOMP: u8 = 0b0111_0000;
const SUBSCRIBE: u8 = 0b1000_0000;
const SUBACK: u8 = 0b1001_0000;
const UNSUBSCRIBE: u8 = 0b1010_0000;
const UNSUBACK: u8 = 0b1011_0000;
const PINGREQ: u8 = 0b1100_0000;
const PINGRESP: u8 = 0b1101_0000;
const DISCONNECT: u8 = 0b1110_0000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PacketType {
    CONNECT = CONNECT as isize,
    CONNACK = CONNACK as isize,
    PUBLISH = PUBLISH as isize,
    PUBACK = PUBACK as isize,
    PUBREC = PUBREC as isize,
    PUBREL = PUBREL as isize,
    PUBCOMP = PUBCOMP as isize,
    SUBSCRIBE = SUBSCRIBE as isize,
    SUBACK = SUBACK as isize,
    UNSUBSCRIBE = UNSUBSCRIBE as isize,
    UNSUBACK = UNSUBACK as isize,
    PINGREQ = PINGREQ as isize,
    PINGRESP = PINGRESP as isize,
    DISCONNECT = DISCONNECT as isize,
}

impl Into<u8> for PacketType {
    fn into(self) -> u8 {
        match self {
            Self::CONNECT => CONNECT,
            Self::CONNACK => CONNACK,
            Self::PUBLISH => PUBLISH,
            Self::PUBACK => PUBACK,
            Self::PUBREC => PUBREC,
            Self::PUBREL => PUBREL,
            Self::PUBCOMP => PUBCOMP,
            Self::SUBSCRIBE => SUBSCRIBE,
            Self::SUBACK => SUBACK,
            Self::UNSUBSCRIBE => UNSUBSCRIBE,
            Self::UNSUBACK => UNSUBACK,
            Self::PINGREQ => PINGREQ,
            Self::PINGRESP => PINGRESP,
            Self::DISCONNECT => DISCONNECT,
        }
    }
}

impl TryFrom<u8> for PacketType {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, DecodeError> {
        // we only want to match on the left four bits.

        let out = match value & PACKET_TYPE_BITS {
            CONNECT => Self::CONNECT,
            CONNACK => Self::CONNACK,
            PUBLISH => Self::PUBLISH,
            PUBACK => Self::PUBACK,
            PUBREC => Self::PUBREC,
            PUBREL => Self::PUBREL,
            PUBCOMP => Self::PUBCOMP,
            SUBSCRIBE => Self::SUBSCRIBE,
            SUBACK => Self::SUBACK,
            UNSUBSCRIBE => Self::UNSUBSCRIBE,
            UNSUBACK => Self::UNSUBACK,
            PINGREQ => Self::PINGREQ,
            PINGRESP => Self::PINGRESP,
            DISCONNECT => Self::DISCONNECT,
            _ => {
                return Err(DecodeError::new(
                    DecodeErrorKind::PacketType,
                    format!("{} does not map to a valid packet type.", value >> 4),
                ))
            }
        };
        return Ok(out);
    }
}

impl Display for PacketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CONNECT => write!(f, "CONNECT"),
            Self::CONNACK => write!(f, "CONNACK"),
            Self::PUBLISH => write!(f, "PUBLISH"),
            Self::PUBACK => write!(f, "PUBACK"),
            Self::PUBREC => write!(f, "PUBREQ"),
            Self::PUBREL => write!(f, "PUBREL"),
            Self::PUBCOMP => write!(f, "PUBCOMP"),
            Self::SUBSCRIBE => write!(f, "SUBSCRIBE"),
            Self::SUBACK => write!(f, "SUBACK"),
            Self::UNSUBSCRIBE => write!(f, "UNSUBSCRIBE"),
            Self::UNSUBACK => write!(f, "UNSUBACK"),
            Self::PINGREQ => write!(f, "PINGREQ"),
            Self::PINGRESP => write!(f, "PINGRESP"),
            Self::DISCONNECT => write!(f, "DISCONNECT"),
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
