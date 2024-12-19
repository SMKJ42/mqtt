use crate::err::{DecodeError, DecodeErrorKind, EncodeError};
use crate::v3::PacketType;
use crate::{
    io::{decode_bytes, decode_utf8, encode_bytes, encode_packet_length, encode_utf8},
    qos::QosLevel,
    topic::TopicName,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use core::fmt::Debug;

/*
 * After a Network Connection is established by a Client to a Server,
 * the first Packet sent from the Client to the Server MUST be a CONNECT Packet [MQTT-3.1.0-1].
 *
 * A Client can only send the CONNECT Packet once over a Network Connection.
 * The Server MUST process a second CONNECT Packet sent from a Client as a protocol
 * violation and disconnect the Client [MQTT-3.1.0-2].
 * See section 4.8 for information about handling errors.
 *
 * The payload contains one or more encoded fields. They specify a unique Client identifier for the Client,
 * a Will topic, Will Message, User Name and Password. All but the Client identifier are optional and their
 * presence is determined based on flags in the variable header.
 */
#[derive(Clone, PartialEq, Debug)]
pub struct ConnectPacket {
    /*
     * The Protocol Name is a UTF-8 encoded string that represents the protocol name “MQTT”, capitalized.
     * The string, its offset and length will not be changed by future versions of the MQTT specification.
     *
     * If the protocol name is incorrect the Server MAY disconnect the Client, or it MAY continue processing
     * the CONNECT packet in accordance with some other specification. In the latter case, the Server MUST NOT
     * continue to process the CONNECT packet in line with this specification [MQTT-3.1.2-1].
     */
    protocol: Protocol,

    /*
     * The 8 bit unsigned value that represents the revision level of the protocol used by the Client.
     *
     * The value of the Protocol Level field for the version 3.1.1 of the protocol is 4 (0x04).
     *
     * The Server MUST respond to the CONNECT Packet with a CONNACK return code 0x01 (unacceptable protocol level)
     * and then disconnect the Client if the Protocol Level is not supported by the Server [MQTT-3.1.2-2].
     */
    level: u8,

    /*
     * The Connect Flags byte contains a number of parameters specifying the behavior of the MQTT connection.
     * It also indicates the presence or absence of fields in the payload.
     *
     * BITS
     * 0: Reserved
     *
     * - The Server MUST validate that the reserved flag in the CONNECT Control Packet is set to zero and disconnect the
     *   Client if it is not zero [MQTT-3.1.2-3].
     *
     *
     * 1: Clean Session
     *
     * - This bit specifies the handling of the Session state.
     *
     * - The Client and Server can store Session state to enable reliable messaging to continue across
     *   a sequence of Network Connections. This bit is used to control the lifetime of the Session state.
     *
     * - If CleanSession is set to 0, the Server MUST resume communications with the Client based on state
     *   from the current Session (as identified by the Client identifier).
     *      - If there is no Session associated with the Client identifier the Server MUST
     *        create a new Session.
     *      - The Client and Server MUST store the Session after the Client and Server are
     *        disconnected [MQTT-3.1.2-4].
     *      - After the disconnection of a Session that had CleanSession set to 0, the Server
     *        MUST store further QoS 1 and QoS 2 messages that match any subscriptions that the
     *        client had at the time of disconnection as part of the Session state [MQTT-3.1.2-5].
     *      - It MAY also store QoS 0 messages that meet the same criteria.
     *
     * - If CleanSession is set to 1, the Client and Server MUST discard any previous Session and start a new one.
     *   This Session lasts as long as the Network Connection. State data associated with this Session MUST NOT be
     *   reused in any subsequent Session [MQTT-3.1.2-6].
     *
     * - Retained messages do not form part of the Session state in the Server, they MUST NOT be deleted when the
     *   Session ends [MQTT-3.1.2.7].
     *
     *
     * 2: Will Flag
     *
     * - If the Will Flag is set to 1 this indicates that, if the Connect request is accepted,
     *   a Will Message MUST be stored on the Server and associated with the Network Connection.
     *
     * - The Will Message MUST be published when the Network Connection is subsequently closed unless the Will Message
     *   has been deleted by the Server on receipt of a DISCONNECT Packet [MQTT-3.1.2-8].
     *
     * - If the Will Flag is set to 1, the Will QoS and Will Retain fields in the Connect Flags will be used by the Server,
     *   and the Will Topic and Will Message fields MUST be present in the payload [MQTT-3.1.2-9].
     *
     * - The Will Message MUST be removed from the stored Session state in the Server once it has been published or the Server
     * has received a DISCONNECT packet from the Client [MQTT-3.1.2-10].
     *
     * - If the Will Flag is set to 0 the Will QoS and Will Retain fields in the Connect Flags MUST be set to zero and the
     *   Will Topic and Will Message fields MUST NOT be present in the payload [MQTT-3.1.2-11].
     *
     * - If the Will Flag is set to 0, a Will Message MUST NOT be published when this Network Connection ends [MQTT-3.1.2-12].
     *
     * The Server SHOULD publish Will Messages promptly. In the case of a Server shutdown or failure the
     * server MAY defer publication of Will Messages until a subsequent restart. If this happens there
     * might be a delay between the time the server experienced failure and a Will Message being published.
     *
     *
     * 3 & 4: Will QoS
     *
     * - These two bits specify the QoS level to be used when publishing the Will Message.
     *
     * - If the Will Flag is set to 0, then the Will QoS MUST be set to 0 (0x00) [MQTT-3.1.2-13].
     *
     * - If the Will Flag is set to 1, the value of Will QoS can be 0 (0x00), 1 (0x01), or 2 (0x02).
     *   It MUST NOT be 3 (0x03) [MQTT-3.1.2-14].
     *
     *
     * 5: Will Retain
     *
     * - If the Will Flag is set to 0, then the Will Retain Flag MUST be set to 0 [MQTT-3.1.2-15].
     *
     * - If the Will Flag is set to 1:
     *      - If Will Retain is set to 0, the Server MUST publish the Will Message as a non-retained message [MQTT-3.1.2-16].
     *      - If Will Retain is set to 1, the Server MUST publish the Will Message as a retained message [MQTT-3.1.2-17].
     *
     *
     * 6: Password
     *
     * - If the User Name Flag is set to 0, a user name MUST NOT be present in the payload [MQTT-3.1.2-18].
     *
     * - If the User Name Flag is set to 1, a user name MUST be present in the payload [MQTT-3.1.2-19].
     *
     *
     * 7: Username
     *
     * - If the Password Flag is set to 0, a password MUST NOT be present in the payload [MQTT-3.1.2-20].
     *
     * - If the Password Flag is set to 1, a password MUST be present in the payload [MQTT-3.1.2-21].
     *
     * - If the User Name Flag is set to 0, the Password Flag MUST be set to 0 [MQTT-3.1.2-22].
     */
    conn_flags: ConnectFlags,

    /*
     * The Keep Alive is a time interval measured in seconds.
     *
     * Expressed as a 16-bit word, it is the maximum time interval that is permitted to elapse between the
     * point at which the Client finishes transmitting one Control Packet and the point it starts sending the next.
     *
     * It is the responsibility of the Client to ensure that the interval between Control Packets being sent does
     * not exceed the Keep Alive value.
     *
     * In the absence of sending any other Control Packets, the Client MUST send a PINGREQ Packet [MQTT-3.1.2-23].
     *
     * The Client can send PINGREQ at any time, irrespective of the Keep Alive value, and use the PINGRESP to
     * determine that the network and the Server are working.
     *
     * If the Keep Alive value is non-zero and the Server does not receive a Control Packet from the Client
     * within one and a half times the Keep Alive time period, it MUST disconnect the Network Connection to
     * the Client as if the network had failed [MQTT-3.1.2-24].
     *
     * If a Client does not receive a PINGRESP Packet within a reasonable amount of time after it has sent a
     * PINGREQ, it SHOULD close the Network Connection to the Server.
     *
     * A Keep Alive value of zero (0) has the effect of turning off the keep alive mechanism. This means that,
     * in this case, the Server is not required to disconnect the Client on the grounds of inactivity.
     *
     * Note that a Server is permitted to disconnect a Client that it determines to be inactive or non-responsive
     * at any time, regardless of the Keep Alive value provided by that Client.
     */
    pub keep_alive: u16,

    /*
     * The Client Identifier (ClientId) identifies the Client to the Server.
     * Each Client connecting to the Server has a unique ClientId.
     * The ClientId MUST be used by Clients and by Servers to identify state that
     * they hold relating to this MQTT Session between the Client and the Server [MQTT-3.1.3-2].
     *
     * The Client Identifier (ClientId) MUST be present and MUST be the first field in the CONNECT packet payload [MQTT-3.1.3-3].
     *
     * The ClientId MUST be a UTF-8 encoded string as defined in Section 1.5.3 [MQTT-3.1.3-4].
     *
     * The Server MUST allow ClientIds which are between 1 and 23 UTF-8 encoded bytes in length,
     * and that contain only the characters:
     * "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ" [MQTT-3.1.3-5].
     *
     * The Server MAY allow ClientId’s that contain more than 23 encoded bytes.
     * The Server MAY allow ClientId’s that contain characters not included in the list given above.
     *
     * A Server MAY allow a Client to supply a ClientId that has a length of zero bytes,
     * however if it does so the Server MUST treat this as a special case and assign a unique ClientId to that Client.
     * It MUST then process the CONNECT packet as if the Client had provided that unique ClientId [MQTT-3.1.3-6].
     *
     * If the Client supplies a zero-byte ClientId, the Client MUST also set CleanSession to 1 [MQTT-3.1.3-7].
     *
     * If the Client supplies a zero-byte ClientId with CleanSession set to 0,
     * the Server MUST respond to the CONNECT Packet with a CONNACK return code 0x02 (Identifier rejected)
     * and then close the Network Connection [MQTT-3.1.3-8].
     *
     * If the Server rejects the ClientId it MUST respond to the CONNECT Packet with a CONNACK return code 0x02
     * (Identifier rejected) and then close the Network Connection [MQTT-3.1.3-9].
     */
    pub client_id: String,

    pub will: Option<Will>,

    /*
     * If the User Name Flag is set to 1, this is the next field in the payload.
     * The User Name MUST be a UTF-8 encoded string as defined in Section 1.5.3 [MQTT-3.1.3-11].
     *  It can be used by the Server for authentication and authorization.
     */
    username: Option<String>,

    /*
     *If the Password Flag is set to 1, this is the next field in the payload.
     * The Password field contains 0 to 65535 bytes of binary data prefixed with a two byte length field which indicates
     * the number of bytes used by the binary data (it does not include the two bytes taken up by the length field itself).
     */
    password: Option<Bytes>,
}

impl ConnectPacket {
    pub fn decode(mut bytes: &mut Bytes) -> Result<Self, DecodeError> {
        // first byte is used to obtain the packet type.
        let protocol: Protocol;
        (protocol, bytes) = Protocol::from_bytes(bytes)?;

        let level = bytes.get_u8();

        if level != 4 {
            return Err(DecodeError::new(
                DecodeErrorKind::InvalidProtocol,
                format!("Mqtt V3.1.1 Requires Protocol level to be 4, instead received: {level}"),
            ));
        }

        let conn_flags = ConnectFlags::from_byte(bytes.get_u8())?;

        let keep_alive = bytes.get_u16();

        let client_id: String;
        client_id = decode_utf8(bytes)?;

        let mut will = None;

        if conn_flags.will() {
            // ewwww....
            let topic: String;
            topic = decode_utf8(bytes)?;

            let message: String;
            message = decode_utf8(bytes)?;

            let qos = conn_flags.will_qos();
            let retain = conn_flags.will_retain();

            will = Some(Will::new(
                TopicName::from_str(topic.as_str())?,
                message,
                qos,
                retain,
            ))
        }

        let username: Option<String>;

        if conn_flags.user_name() {
            let string: String;
            string = decode_utf8(bytes)?;
            username = Some(string);
        } else {
            username = None;
        }

        let password: Option<Bytes>;

        if conn_flags.password() {
            let buf: Bytes;
            buf = decode_bytes(bytes)?;
            password = Some(buf);
        } else {
            password = None;
        }

        return Ok(Self {
            protocol,
            level,
            conn_flags,
            keep_alive,
            client_id,
            will,
            username,
            password,
        });
    }

    pub fn encode(&self) -> Result<Bytes, EncodeError> {
        // 2 for fixed header, 1 for protocol level, 1 for connect flags, two for the keep alive.
        let mut len = 1 + 1 + 2;
        // utf-8 decode is prefixed by two bytes to denote string length.
        len += 2 + self.protocol.len();

        len += 2 + self.client_id.len();

        if let Some(will) = &self.will {
            len += will.will_topic.len() + 2;
            len += will.will_message.len() + 2;
        }

        if let Some(username) = &self.username {
            len += username.len() + 2;
        }

        if let Some(password) = &self.password {
            len += password.len() + 2;
        }

        let mut bytes = BytesMut::with_capacity(len);

        bytes.put_u8(PacketType::CONNECT as u8);

        encode_packet_length(&mut bytes, len)?;

        encode_utf8(&mut bytes, self.protocol.as_str())?;

        bytes.put_u8(self.level);

        bytes.put_u8(self.conn_flags.as_byte());

        bytes.put_u16(self.keep_alive);

        encode_utf8(&mut bytes, &self.client_id)?;

        if let Some(will) = &self.will {
            encode_utf8(&mut bytes, &will.will_topic.clone().to_string())?;
            encode_utf8(&mut bytes, &will.will_message)?;
        }

        if let Some(username) = &self.username {
            encode_utf8(&mut bytes, &username)?;
        }

        if let Some(password) = &self.password {
            encode_bytes(&mut bytes, &password)?;
        }

        return Ok(bytes.into());
    }

    pub fn new(
        is_clean_session: bool,
        keep_alive: u16,
        client_id: String,
        will: Option<Will>,
        username: Option<String>,
        password: Option<Bytes>,
    ) -> Self {
        let mut conn_flags = ConnectFlags::default();

        if username.is_some() {
            conn_flags.set_user_name(true);
        }

        if password.is_some() {
            conn_flags.set_password(true);
        }

        match &will {
            Some(will) => {
                conn_flags.set_will_retain(will.will_retain);
                conn_flags.set_will_qos(will.will_qos.into());
                conn_flags.set_will(true);
            }
            None => {}
        }

        if is_clean_session {
            conn_flags.set_clean_session(true);
        }

        return Self {
            protocol: Protocol::MQTT,
            level: 4,
            conn_flags,
            keep_alive,
            client_id,
            will,
            username,
            password,
        };
    }

    pub fn client_id(&self) -> &'_ str {
        return &self.client_id;
    }

    pub fn will_retain(&self) -> bool {
        return self.conn_flags.will_retain();
    }

    pub fn clean_session(&self) -> bool {
        return self.conn_flags.clean_session();
    }

    pub fn username(&self) -> &Option<String> {
        return &self.username;
    }

    pub fn password(&self) -> &Option<Bytes> {
        return &self.password;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Will {
    // prefixing the fields with will may seem verbose, but it adds clarity when dealing with the struct at a higher level.
    // For instance the will_topic, may not be the same as the PUBLISH topic being sent on the connection

    /*
     * If the Will Flag is set to 1, the Will Topic is the next field in the payload.
     * The Will Topic MUST be a UTF-8 encoded string as defined in Section 1.5.3 [MQTT-3.1.3-10].
     */
    will_topic: TopicName,

    /*
     * If the Will Flag is set to 1 the Will Message is the next field in the payload.
     *
     * The Will Message defines the Application Message that is to be published to the Will Topic as
     * described in Section 3.1.2.5.
     *
     * This field consists of a two byte length followed by the payload for the Will Message expressed
     * as a sequence of zero or more bytes.
     *
     * The length gives the number of bytes in the data that follows and does not include the 2 bytes
     * taken up by the length itself.
     *
     * When the Will Message is published to the Will Topic its payload consists only of the data portion
     * of this field, not the first two length bytes.
     *  - This essentially means that when sending a packet notifying a failed connection,
     *    it does not include the two bytes length. The length is already built into the
     *    'payload' section of the PUBLISH packet.
     */
    will_message: String,

    /*
     * These two bits specify the QoS level to be used when publishing the Will Message.
     *
     * If the Will Flag is set to 0, then the Will QoS MUST be set to 0 (0x00) [MQTT-3.1.2-13].
     *
     * If the Will Flag is set to 1, the value of Will QoS can be 0 (0x00), 1 (0x01), or 2 (0x02).
     * It MUST NOT be 3 (0x03) [MQTT-3.1.2-14].
     */
    will_qos: QosLevel,

    /*
     * This bit specifies if the Will Message is to be Retained when it is published.
     * If the Will Flag is set to 0, then the Will Retain Flag MUST be set to 0 [MQTT-3.1.2-15].
     *
     * If the Will Flag is set to 1:
     *   - If Will Retain is set to 0, the Server MUST publish the Will Message as a non-retained message [MQTT-3.1.2-16].
     *   - If Will Retain is set to 1, the Server MUST publish the Will Message as a retained message [MQTT-3.1.2-17].
     */
    will_retain: bool,
}

impl Will {
    pub fn new(
        will_topic: TopicName,
        will_message: String,
        will_qos: QosLevel,
        will_retain: bool,
    ) -> Self {
        return Self {
            will_topic,
            will_message,
            will_qos,
            will_retain,
        };
    }

    pub fn will_topic(&self) -> &TopicName {
        return &self.will_topic;
    }

    pub fn will_message(&self) -> String {
        return self.will_message.clone();
    }

    pub fn will_qos(&self) -> QosLevel {
        return self.will_qos;
    }

    pub fn will_retain(&self) -> bool {
        return self.will_retain;
    }
}

const USERNAME: u8 = 0b1000_0000;
const PASSWORD: u8 = 0b0100_0000;
const WILL_RETAIN: u8 = 0b0010_0000;
const WILL_QOS_2: u8 = 0b0001_0000; // QoS 2 (2nd bit of QoS)
const WILL_QOS_1: u8 = 0b0000_1000; // QoS 1 (1st bit of QoS)
const WILL_QOS_BITS: u8 = 0b0001_1000;
const WILL: u8 = 0b0000_0100;
const CLEAN_SESSION: u8 = 0b0000_0010;
const RESERVED_BIT: u8 = 0b0000_0001;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ConnectFlags {
    byte: u8,
}

impl ConnectFlags {
    /// Handles Errors for malformed Connect Flags.
    pub fn from_byte(byte: u8) -> Result<Self, DecodeError> {
        if (byte & WILL_QOS_BITS >> 3) > 3 {
            return Err(DecodeError::new(
                DecodeErrorKind::WillQoS,
                format!("QoS cannot be set to 4"),
            ));
        }
        if byte & RESERVED_BIT == RESERVED_BIT {
            return Err(DecodeError::new(
                DecodeErrorKind::ProtocolError,
                format!("Connect packet cannot have reserved bit (index 0) set, received: {byte}"),
            ));
        }
        if byte & WILL == 0 {
            if byte & 0b0011_1000 != 0 {
                return Err(DecodeError::new(
                    DecodeErrorKind::Will,
                    format!(
                        "Optional connection Will bits were set, but the Will bit itself was unset, received: {byte}"
                    ),
                ));
            }
        }

        if byte & PASSWORD == PASSWORD {
            if byte & !USERNAME == USERNAME {
                return Err(DecodeError::new(
                    DecodeErrorKind::UsernamePassword,
                    format!("Password bit is set and Username bit is unset, received: {byte}"),
                ));
            }
        }

        return Ok(Self { byte });
    }

    pub fn as_byte(&self) -> u8 {
        return self.byte;
    }

    // Helper for handling QoS field
    pub fn will_qos(&self) -> QosLevel {
        match self.byte & (WILL_QOS_BITS) {
            WILL_QOS_1 => QosLevel::AtLeastOnce,
            WILL_QOS_2 => QosLevel::ExactlyOnce,
            _ => QosLevel::AtMostOnce,
        }
    }

    pub fn set_will_qos(&mut self, value: QosLevel) {
        // Clear the current QoS bit
        self.byte = self.byte & !(WILL_QOS_BITS);
        // Left shift the QoS to the proper placement and set the bits.
        self.byte = self.byte | ((value as u8) << 3);
    }

    pub fn user_name(&self) -> bool {
        if self.byte & USERNAME == USERNAME {
            return true;
        } else {
            return false;
        }
    }

    pub fn set_user_name(&mut self, val: bool) {
        if val {
            self.byte = self.byte | USERNAME;
        } else {
            self.byte = self.byte & !USERNAME;
        }
    }

    pub fn password(&self) -> bool {
        if self.byte & PASSWORD == PASSWORD {
            return true;
        } else {
            return false;
        }
    }

    pub fn set_password(&mut self, val: bool) {
        if val {
            self.byte = self.byte | PASSWORD;
        } else {
            self.byte = self.byte & !PASSWORD;
        }
    }

    pub fn will_retain(&self) -> bool {
        if self.byte & WILL_RETAIN == WILL_RETAIN {
            return true;
        } else {
            return false;
        }
    }

    pub fn set_will_retain(&mut self, val: bool) {
        if val {
            self.byte = self.byte | WILL_RETAIN;
        } else {
            self.byte = self.byte & !WILL_RETAIN;
        }
    }

    pub fn will(&self) -> bool {
        if self.byte & WILL == WILL {
            return true;
        } else {
            return false;
        }
    }

    pub fn set_will(&mut self, val: bool) {
        if val {
            self.byte = self.byte | WILL;
        } else {
            self.byte = self.byte & !WILL;
        }
    }

    pub fn clean_session(&self) -> bool {
        if self.byte & CLEAN_SESSION == CLEAN_SESSION {
            return false;
        } else {
            return true;
        }
    }

    pub fn set_clean_session(&mut self, val: bool) {
        if val {
            self.byte = self.byte | CLEAN_SESSION;
        } else {
            self.byte = self.byte & !CLEAN_SESSION;
        }
    }
}

impl Default for ConnectFlags {
    fn default() -> Self {
        return Self { byte: 0 };
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    MQTT,
}

impl Protocol {
    pub fn from_bytes(bytes: &mut Bytes) -> Result<(Self, &mut Bytes), DecodeError> {
        let protocol_name = decode_utf8(bytes)?;

        match &protocol_name.as_str() {
            &"MQTT" => return Ok((Self::MQTT, bytes)),
            _ => {
                return Err(DecodeError::new(
                    DecodeErrorKind::InvalidProtocol,
                    format!(
                        "Only MQTT packet types are allowed, instead received type: {protocol_name}"
                    ),
                ))
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::MQTT => return "MQTT",
        }
    }

    /// returns the number of bytes, not the number of chars.
    pub fn len(&self) -> usize {
        return self.as_str().len();
    }
}

#[cfg(test)]
mod packet {

    use crate::v3::{FixedHeader, MqttPacket};

    use super::ConnectPacket;
    use bytes::Buf;

    #[test]
    fn serialize_deserialize() {
        let packet = ConnectPacket::new(true, 100, "id_1".to_string(), None, None, None);
        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Connect(packet));

        let packet = ConnectPacket::new(true, 10, String::from("TestClientId"), None, None, None);
        let mut buf = packet.encode().unwrap();

        let f_header = FixedHeader::decode(&mut buf).unwrap();
        buf.advance(f_header.header_len);
        let packet_de = MqttPacket::decode(f_header, &mut buf).expect("Could not decode packet");

        assert_eq!(packet_de, MqttPacket::Connect(packet));
    }
}
