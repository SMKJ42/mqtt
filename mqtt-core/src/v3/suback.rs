use crate::{
    err::{DecodeError, EncodeError},
    io::encode_packet_length,
    qos::SubAckQoS,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};

use super::PacketType;

/*
 * A SUBACK Packet is sent by the Server to the Client to confirm receipt and processing of a SUBSCRIBE Packet.
 *
 * A SUBACK Packet contains a list of return codes, that specify the maximum QoS level that was granted
 * in each Subscription that was requested by the SUBSCRIBE.
 */
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct SubAckPacket {
    packet_id: u16,

    /*
     * The payload contains a list of return codes.
     *
     * Each return code corresponds to a Topic Filter in the SUBSCRIBE Packet being acknowledged.
     *
     * The order of return codes in the SUBACK Packet MUST match the order of Topic Filters in the SUBSCRIBE Packet [MQTT-3.9.3-1].
     */
    payload: Vec<SubAckQoS>,
}

impl SubAckPacket {
    pub fn new(packet_id: u16, payload: Vec<SubAckQoS>) -> Self {
        return Self { packet_id, payload };
    }

    pub fn decode(bytes: &mut Bytes) -> Result<Self, DecodeError> {
        let packet_id = bytes.get_u16();

        let mut payload: Vec<SubAckQoS> = Vec::new();

        while bytes.remaining() > 0 {
            payload.push(bytes.get_u8().try_into()?);
        }

        return Ok(Self { packet_id, payload });
    }

    pub fn encode(&self) -> Result<Bytes, EncodeError> {
        let len = 2 + self.payload.len();

        let mut bytes = BytesMut::with_capacity(len);

        bytes.put_u8(PacketType::SUBACK as u8);
        encode_packet_length(&mut bytes, len)?;

        bytes.put_u16(self.packet_id);

        for topic in &self.payload {
            // ew
            let topic = *topic;
            bytes.put_u8(topic.into());
        }

        return Ok(bytes.into());
    }

    pub fn id(&self) -> u16 {
        return self.packet_id;
    }

    pub fn filters(&self) -> &Vec<SubAckQoS> {
        return &self.payload;
    }
}

#[cfg(test)]
mod packet {
    use super::SubAckPacket;
    use crate::{
        qos::{QosLevel, SubAckQoS},
        v3::{FixedHeader, MqttPacket},
    };
    use bytes::Buf;

    #[test]
    fn serialize_deserialize() {
        let packet = SubAckPacket::new(1234, vec![SubAckQoS::QOS(QosLevel::AtLeastOnce)]);
        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::SubAck(packet));
    }
}
