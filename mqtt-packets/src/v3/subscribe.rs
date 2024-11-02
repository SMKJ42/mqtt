use bytes::{Buf, BufMut, Bytes, BytesMut};
use mqtt_core::{
    err::PacketError,
    io::{encode_packet_length, encode_utf8},
    qos::QosLevel,
    topics::TopicFilter,
};

use crate::v3::PacketType;

/*
 * The SUBSCRIBE Packet is sent from the Client to the Server to create one or more Subscriptions.
 * Each Subscription registers a Client’s interest in one or more Topics.
 *
 * The Server sends PUBLISH Packets to the Client in order to forward Application Messages
 * that were published to Topics that match these Subscriptions.
 *
 * The SUBSCRIBE Packet also specifies (for each Subscription) the maximum QoS
 * with which the Server can send Application Messages to the Client.
 */

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct SubscribePacket {
    packet_id: u16,

    /*
     * The payload of a SUBSCRIBE Packet contains a list of Topic Filters indicating the Topics to which the Client wants to subscribe.
     *
     * The Topic Filters in a SUBSCRIBE packet payload MUST be UTF-8 encoded strings as defined in Section 1.5.3 [MQTT-3.8.3-1].
     *
     * A Server SHOULD support Topic filters that contain the wildcard characters defined in Section 4.7.1.
     *
     * If it chooses not to support topic filters that contain wildcard characters it MUST reject any Subscription
     * request whose filter contains them [MQTT-3.8.3-2]. Each filter is followed by a byte called the Requested QoS.
     *
     * This gives the maximum QoS level at which the Server can send Application Messages to the Client.
     */
    payload: Vec<(TopicFilter, QosLevel)>,
}

impl SubscribePacket {
    pub fn new(packet_id: u16, payload: Vec<(TopicFilter, QosLevel)>) -> Self {
        return Self { packet_id, payload };
    }

    pub fn decode(mut bytes: &mut Bytes) -> Result<Self, PacketError> {
        let packet_id = bytes.get_u16();

        let mut payload = Vec::new();

        // The requested maximum QoS field is encoded in the byte following each UTF-8 encoded topic name, and these Topic Filter / QoS pairs are packed contiguously.
        loop {
            let (topic_filter, new_bytes) = TopicFilter::decode(bytes)?;
            bytes = new_bytes;

            let qos: QosLevel = bytes.get_u8().try_into()?;

            // we add 1 to the length to accound to the QoS byte.

            payload.push((topic_filter, qos));

            if bytes.remaining() == 0 {
                break;
            }
        }

        return Ok(Self { packet_id, payload });
    }

    pub fn encode(&self) -> Result<Bytes, PacketError> {
        // 2 for packet_id
        let mut len = 2;

        for (filter, _qos) in &self.payload {
            // 2 for str length, 1 for QoS byte
            len += 2 + 1;
            len += filter.len();
        }

        let mut bytes = BytesMut::with_capacity(len);

        bytes.put_u8(PacketType::SUBSCRIBE as u8 | 0x02);

        encode_packet_length(&mut bytes, len)?;

        bytes.put_u16(self.packet_id);

        for (filter, qos) in &self.payload {
            encode_utf8(&mut bytes, &filter.clone().to_string())?;
            bytes.put_u8(*qos as u8);
        }

        return Ok(bytes.into());
    }

    pub fn topic_filters(&self) -> Vec<(TopicFilter, QosLevel)> {
        return self.payload.clone();
    }
}

#[cfg(test)]
mod test {
    use mqtt_core::{qos::QosLevel, topics::TopicFilter};

    use crate::v3::{FixedHeader, MqttPacket};

    use super::SubscribePacket;

    #[test]
    fn subscribe_serialize_deserialize() {
        let packet = SubscribePacket::new(
            1234,
            vec![(
                TopicFilter::from_str("test").unwrap(),
                QosLevel::AtLeastOnce,
            )],
        );
        let mut buf = packet.encode().unwrap();

        let (f_header, buf) = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Subscribe(packet));
    }
}
