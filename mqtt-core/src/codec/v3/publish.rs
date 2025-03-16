use crate::{
    err::{DecodeError, EncodeError},
    io::{decode_utf8, encode_packet_length, encode_utf8},
    qos::QosLevel,
    topic::TopicName,
    v3::{FixedHeader, PacketType},
    Decode, Encode,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use core::fmt::Debug;
use std::sync::Arc;

/*
 * A PUBLISH Control Packet is sent from a Client to a Server
 * or from Server to a Client to transport an Application Message.
 *
 * The receiver of a PUBLISH Packet MUST respond according to Table 3.4 -
 * Expected Publish Packet response as determined by the QoS in the PUBLISH Packet [MQTT-3.3.4-1].
 *
 * The Client uses a PUBLISH Packet to send an Application Message to the Server,
 * for distribution to Clients with matching subscriptions.
 *
 * The Server uses a PUBLISH Packet to send an Application Message to each Client
 * which has a matching subscription.
 *
 * When Clients make subscriptions with Topic Filters that include wildcards,
 * it is possible for a Client’s subscriptions to overlap so that a published message
 * might match multiple filters. In this case the Server MUST deliver the message to
 * the Client respecting the maximum QoS of all the matching subscriptions [MQTT-3.3.5-1].
 * In addition, the Server MAY deliver further copies of the message, one for each
 * additional matching subscription and respecting the subscription’s QoS in each case.
 *
 * The action of the recipient when it receives a PUBLISH Packet depends on the QoS
 * level as described in Section 4.3.
 *
 * If a Server implementation does not authorize a PUBLISH to be performed by a Client;
 * it has no way of informing that Client. It MUST either make a positive acknowledgement,
 * according to the normal QoS rules, or close the Network Connection [MQTT-3.3.5-2].
 */

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct PublishPacket {
    flags: PublishFixedHeaderFlags,
    /*
     * The Topic Name MUST be present as the first field in the PUBLISH Packet Variable header.
     * It MUST be a UTF-8 encoded string [MQTT-3.3.2-1] as defined in section 1.5.3.
     *
     * The Topic Name in the PUBLISH Packet MUST NOT contain wildcard characters [MQTT-3.3.2-2].
     *
     * The Topic Name in a PUBLISH Packet sent by a Server to a subscribing Client MUST match the
     * Subscription’s Topic Filter according to the matching process defined in Section 4.7  [MQTT-3.3.2-3]. However, since the Server is permitted to override the Topic Name, it might not be the same as the Topic Name in the original PUBLISH Packet.
     */
    topic_name: TopicName,
    /*
     * The Packet Identifier field is only present in PUBLISH Packets where the QoS level is 1 or 2.
     *  Section 2.3.1 provides more information about Packet Identifiers.
     */
    packet_id: Option<u16>,
    payload: Bytes,
}

impl PublishPacket {
    pub fn new(topic_name: &TopicName, payload: Bytes) -> Self {
        return Self {
            packet_id: None,
            topic_name: topic_name.clone(),
            flags: PublishFixedHeaderFlags::zero(),
            payload,
        };
    }

    pub fn decode(f_header: FixedHeader, bytes: &mut Bytes) -> Result<Self, DecodeError> {
        let topic_name_in = decode_utf8(bytes)?;
        let topic_name = TopicName::from_str(topic_name_in.as_str())?;

        let flags = PublishFixedHeaderFlags::from_byte(f_header.flags.as_byte());

        let packet_id = if flags.qos() != QosLevel::AtMostOnce {
            Some(bytes.get_u16())
        } else {
            None
        };

        return Ok(Self {
            packet_id,
            flags,
            topic_name,
            payload: bytes.clone(),
        });
    }

    pub fn set_qos_atmostonce(&mut self) {
        self.flags.set_qos(QosLevel::AtMostOnce);
    }

    pub fn set_qos_atleastonce(&mut self, packet_id: u16) {
        self.flags.set_qos(QosLevel::AtLeastOnce);
        self.packet_id = Some(packet_id);
    }

    pub fn set_qos_exactlyonce(&mut self, packet_id: u16) {
        self.flags.set_qos(QosLevel::ExactlyOnce);
        self.packet_id = Some(packet_id);
    }

    pub fn topic(&self) -> &TopicName {
        return &self.topic_name;
    }

    pub fn qos(&self) -> QosLevel {
        return self.flags.qos();
    }

    pub fn retain(&self) -> bool {
        return self.flags.retain();
    }

    pub fn set_retain(&mut self, val: bool) {
        self.flags.set_retain(val);
    }

    pub fn dup(&self) -> bool {
        return self.flags.dup();
    }

    pub fn set_dup(&mut self, val: bool) {
        self.flags.set_dup(val);
    }

    pub fn id(&self) -> Option<u16> {
        return self.packet_id;
    }

    pub unsafe fn set_id(&mut self, id: Option<u16>) {
        self.packet_id = id;
    }

    pub fn payload(&self) -> &Bytes {
        return &self.payload;
    }
}

/// I know this is weird to extract into a trait, for justification of trait see [Message Assurance](crate::msg_assurance)
///
/// It will allow mutlitple different packet types to be used in the messaging queue, but the publish packet is the only packet that will
/// ever be inserted into this queue. Therefore no other packet types utilize this trait.
impl Encode for PublishPacket {
    fn encode(&self) -> Result<Bytes, EncodeError> {
        // add size for topic length.
        let mut len = 2 + self.topic_name.len();
        // add 2 for packet id

        if self.packet_id.is_some() {
            len += 2;
        }

        len += self.payload.len();

        let mut bytes = BytesMut::with_capacity(len);

        bytes.put_u8(PacketType::PUBLISH as u8 | self.flags.byte);

        encode_packet_length(&mut bytes, len)?;

        encode_utf8(&mut bytes, &self.topic_name.clone().to_string())?;

        if let Some(packet_id) = self.packet_id {
            bytes.put_u16(packet_id);
        }

        bytes.put_slice(&self.payload);

        return Ok(bytes.into());
    }
}

/*
 * If the RETAIN flag is set to 1, in a PUBLISH Packet sent by a Client to a Server,
 * the Server MUST store the Application Message and its QoS, so that it can be delivered
 * to future subscribers whose subscriptions match its topic name [MQTT-3.3.1-5].
 *
 * When a new subscription is established, the last retained message, if any,
 * on each matching topic name MUST be sent to the subscriber [MQTT-3.3.1-6].
 *
 * If the Server receives a QoS 0 message with the RETAIN flag set to 1 it MUST discard
 * any message previously retained for that topic. It SHOULD store the new QoS 0 message
 * as the new retained message for that topic, but MAY choose to discard it at any
 * time - if this happens there will be no retained message for that topic [MQTT-3.3.1-7].
 *
 * See Section 4.1 for more information on storing state
 *
 * When sending a PUBLISH Packet to a Client the Server MUST set the RETAIN flag to 1 if
 * a message is sent as a result of a new subscription being made by a Client [MQTT-3.3.1-8].
 *
 * It MUST set the RETAIN flag to 0 when a PUBLISH Packet is sent to a Client because it matches
 * an established subscription regardless of how the flag was set in the message it received [MQTT-3.3.1-9].
 *
 * A PUBLISH Packet with a RETAIN flag set to 1 and a payload containing zero bytes will
 * be processed as normal by the Server and sent to Clients with a subscription matching
 * the topic name. Additionally any existing retained message with the same topic name MUST
 * be removed and any future subscribers for the topic will not receive a retained message [MQTT-3.3.1-10].
 *
 * “As normal” means that the RETAIN flag is not set in the message received by existing Clients.
 *  A zero byte retained message MUST NOT be stored as a retained message on the Server [MQTT-3.3.1-11].
 *
 * If the RETAIN flag is 0, in a PUBLISH Packet sent by a Client to a Server,
 * the Server MUST NOT store the message and MUST NOT remove or replace any existing
 * retained message [MQTT-3.3.1-12].
 */
const RETAIN: u8 = 0b0000_0001;

/*
 * A PUBLISH Packet MUST NOT have both QoS bits set to 1.
 * If a Server or Client receives a PUBLISH Packet which has
 * both QoS bits set to 1 it MUST close the Network Connection [MQTT-3.3.1-4].
 */
const QOS_1: u8 = 0b0000_0010; // QoS 1 (bit 1 of QoS)
const QOS_2: u8 = 0b0000_0100; // QoS 2 (bit 2 of QoS)
const QOS_BITS: u8 = 0b0000_0110;

/*
 * If the DUP flag is set to 0, it indicates that this is the first occasion
 * that the Client or Server has attempted to send this MQTT PUBLISH Packet.
 *
 * If the DUP flag is set to 1, it indicates that this might be re-delivery of
 * an earlier attempt to send the Packet.
 *
 * The DUP flag MUST be set to 1 by the Client or Server when it attempts to re-deliver
 * a PUBLISH Packet [MQTT-3.3.1.-1]. The DUP flag MUST be set to 0 for all QoS 0 messages [MQTT-3.3.1-2].
 *
 * The value of the DUP flag from an incoming PUBLISH packet is not propagated
 * when the PUBLISH Packet is sent to subscribers by the Server. The DUP flag in
 * the outgoing PUBLISH packet is set independently to the incoming PUBLISH packet,
 * its value MUST be determined solely by whether the outgoing PUBLISH packet is
 * a retransmission [MQTT-3.3.1-3].
 */
const DUP: u8 = 0b0000_1000;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct PublishFixedHeaderFlags {
    byte: u8,
}

impl PublishFixedHeaderFlags {
    fn from_byte(byte: u8) -> Self {
        return Self {
            byte: byte & 0b0000_1111,
        };
    }

    fn zero() -> Self {
        return Self { byte: 0 };
    }

    fn qos(&self) -> QosLevel {
        match self.byte & (QOS_BITS) {
            QOS_1 => QosLevel::AtLeastOnce,
            QOS_2 => QosLevel::ExactlyOnce,
            _ => QosLevel::AtMostOnce,
        }
    }

    // Helper to set the QoS field (2 bits)
    fn set_qos(&mut self, val: QosLevel) {
        // Clear the current QoS bit
        self.byte = self.byte & !(QOS_BITS);
        // Get the QoS as numeric u8 value
        // Left shift the QoS to the proper placement and set the bits.
        self.byte = self.byte | ((val as u8) << 1);
    }

    fn retain(&self) -> bool {
        if self.byte & RETAIN == RETAIN {
            return true;
        } else {
            return false;
        }
    }

    fn set_retain(&mut self, val: bool) {
        if val {
            self.byte = self.byte | RETAIN;
        } else {
            self.byte = self.byte & !RETAIN;
        }
    }

    fn dup(&self) -> bool {
        if self.byte & DUP == DUP {
            return true;
        } else {
            return false;
        }
    }

    fn set_dup(&mut self, val: bool) {
        if val {
            self.byte = self.byte | DUP;
        } else {
            self.byte = self.byte & !DUP;
        }
    }
}

// impl Encode for Arc<PublishPacket> {
//     fn encode(&self) -> Result<Bytes, EncodeError> {
//         self.encode()
//     }
// }

// impl Decode for PublishPacket {
//     fn try_from_bytes(&self) -> Result<Self, T> {}
// }

#[cfg(test)]
mod packet {
    use super::PublishPacket;
    use crate::topic::TopicName;
    use crate::v3::{FixedHeader, MqttPacket};
    use crate::{Decode, Encode};
    use bytes::Buf;
    use bytes::Bytes;

    // Generic Packet test
    #[test]
    fn serialize_deserialize_generic() {
        let packet = PublishPacket::new(
            &TopicName::from_str("this/is/a/test").expect("Could not create topic name"),
            Bytes::from_iter([117]),
        );
        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet.payload.first().expect("No payload present"), &117);
        assert_eq!(packet_de, MqttPacket::Publish(packet));
    }

    // Test QoS encode and decode
    #[test]
    fn serialize_deserialize_qos() {
        let mut packet = PublishPacket::new(
            &TopicName::from_str("this/is/a/test").expect("Could not create topic name"),
            Bytes::from_iter([117]),
        );
        packet.set_qos_atleastonce(1234);

        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Publish(packet));
    }
}
