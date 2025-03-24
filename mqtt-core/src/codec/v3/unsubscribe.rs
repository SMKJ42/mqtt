use crate::{
    err::{DecodeError, EncodeError},
    io::{encode_packet_length, encode_utf8},
    topic::TopicFilter,
    v3::PacketType,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};

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

    pub fn id(&self) -> u16 {
        return self.packet_id;
    }

    pub fn decode(bytes: &mut Bytes) -> Result<Self, DecodeError> {
        let packet_id = bytes.get_u16();

        let mut filters = Vec::new();

        loop {
            let topic_filter = TopicFilter::decode(bytes)?;

            filters.push(topic_filter);

            if bytes.remaining() == 0 {
                break;
            }
        }

        return Ok(Self { packet_id, filters });
    }

    pub fn encode(&self) -> Result<Bytes, EncodeError> {
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

    pub fn filters(&self) -> &Vec<TopicFilter> {
        return &self.filters;
    }
}

#[cfg(test)]
mod packet {
    use super::UnsubscribePacket;
    use crate::{
        topic::TopicFilter,
        v3::{FixedHeader, MqttPacket},
        Decode,
    };

    #[test]
    fn serialize_deserialize() {
        let packet = UnsubscribePacket::new(1234, vec![TopicFilter::from_str("test").unwrap()]);
        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Unsubscribe(packet));
    }
}
