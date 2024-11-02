use bytes::{Buf, BufMut, Bytes, BytesMut};
use mqtt_core::{err::PacketError, io::encode_packet_length, qos::QosLevel};

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
    payload: Vec<TopicFilterResponse>,
}

impl SubAckPacket {
    pub fn new(packet_id: u16, payload: Vec<TopicFilterResponse>) -> Self {
        return Self { packet_id, payload };
    }

    pub fn decode(bytes: &mut Bytes) -> Result<Self, PacketError> {
        let packet_id = bytes.get_u16();

        let mut payload: Vec<TopicFilterResponse> = Vec::new();

        while bytes.remaining() > 0 {
            payload.push(bytes.get_u8().try_into()?);
        }

        return Ok(Self { packet_id, payload });
    }

    pub fn encode(&self) -> Result<Bytes, PacketError> {
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
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum TopicFilterResponse {
    QOS(QosLevel),
    Err,
}

/*
* Allowed return codes:
* 0x00 - Success - Maximum QoS 0
* 0x01 - Success - Maximum QoS 1
* 0x02 - Success - Maximum QoS 2
* 0x80 - Failure
*/
impl Into<u8> for TopicFilterResponse {
    fn into(self) -> u8 {
        match self {
            Self::Err => return 0b1000_0000,
            Self::QOS(qos) => return qos as u8,
        }
    }
}

impl TryFrom<u8> for TopicFilterResponse {
    type Error = PacketError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 0b1000_0000 {
            return Ok(Self::Err);
        } else {
            return Ok(Self::QOS(QosLevel::try_from(value)?));
        }
    }
}

#[cfg(test)]
mod test {

    use mqtt_core::qos::QosLevel;

    use crate::v3::{suback::TopicFilterResponse, FixedHeader, MqttPacket};

    use super::SubAckPacket;

    #[test]
    fn suback_serialize_deserialize() {
        let packet = SubAckPacket::new(1234, vec![TopicFilterResponse::QOS(QosLevel::AtLeastOnce)]);
        let mut buf = packet.encode().unwrap();

        let (f_header, buf) = FixedHeader::decode(&mut buf).unwrap();
        let packet_de = MqttPacket::decode(f_header, buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::SubAck(packet));
    }
}
