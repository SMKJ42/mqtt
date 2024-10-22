use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    err::PacketError,
    io::{encode_packet_length, encode_utf8},
    v3::PacketType,
};

use super::shared::TopicFilter;

/*
 * An UNSUBSCRIBE Packet is sent by the Client to the Server, to unsubscribe from topics.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct UnsubscribePacket {
    packet_id: u16,
    filters: Vec<TopicFilter>,
}

impl UnsubscribePacket {
    pub fn new(packet_id: u16, filters: Vec<TopicFilter>) -> Self {
        return Self { packet_id, filters };
    }

    pub fn decode(mut bytes: Bytes) -> Result<Self, PacketError> {
        let packet_id = bytes.get_u16();

        let mut filters = Vec::new();

        loop {
            let (topic_filter, new_bytes) = TopicFilter::decode(bytes)?;
            bytes = new_bytes;

            filters.push(topic_filter);

            if bytes.remaining() == 0 {
                break;
            }
        }

        return Ok(Self { packet_id, filters });
    }

    pub fn encode(&self) -> Result<Bytes, PacketError> {
        // 2 for packet_id;
        let mut len = 2;

        for filter in &self.filters {
            len += 2 + filter.len()
        }

        let mut bytes = BytesMut::with_capacity(len);

        bytes.put_u8(PacketType::UNSUBSCRIBE as u8 | 0x02);

        encode_packet_length(&mut bytes, len)?;

        bytes.put_u16(self.packet_id);

        for filter in &self.filters {
            encode_utf8(&mut bytes, &filter.clone().to_string())?;
        }

        return Ok(bytes.into());
    }
}

#[cfg(test)]
mod test {
    use crate::v3::{shared::TopicFilter, MqttPacket};

    use super::UnsubscribePacket;

    #[test]
    fn unsubscribe_serialize_deserialize() {
        let packet = UnsubscribePacket::new(1234, vec![TopicFilter::from_str("test").unwrap()]);

        let packet_en = packet.encode().expect("Could not encode packet");

        let packet_de = MqttPacket::decode(packet_en).expect("Could not decode packet");

        assert_eq!(MqttPacket::Unsubscribe(packet), packet_de);
    }
}
