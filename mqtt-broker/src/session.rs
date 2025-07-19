use bytes::{BufMut, BytesMut};

use mqtt_core::err::server::{self, ServerError};
use mqtt_core::id::{IdGenType, IdGenerator};
use mqtt_core::msg_assurance::sender::{AtLeastOnceList, ExactlyOnceList};
use mqtt_core::msg_assurance::RetryDuration;
use mqtt_core::qos::QosLevel;
use mqtt_core::topic::TopicFilter;
use mqtt_core::v4::{
    ConnectPacket, MqttPacket, PubAckPacket, PubRecPacket, PubRelPacket, PublishPacket, Will,
};
use mqtt_core::{ConnectReturnCode, Encode, MqttVersion};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use std::net::IpAddr;
use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::io::{AsyncWrite, AsyncWriteExt};

pub type AtLeastOnceListType = AtLeastOnceList<Arc<PublishPacket>, Instant, RetryDuration>;
pub type ExactlyOnceListType = ExactlyOnceList<Arc<PublishPacket>, Instant, RetryDuration>;

#[derive(Debug, Clone)]
pub struct ActiveSession {
    client_id: String,
    conn_t: ConnectionType,
    protocol: Protocol,
    will: Option<Will>,
    keep_alive: u64,
    last_read: Instant,
    topic_filters: Vec<TopicFilter>,
    qos1_packets: AtLeastOnceListType,
    qos2_packets: ExactlyOnceListType,
    id_gen: IdGenerator,
}

impl ActiveSession {
    pub fn new_tcp(packet: ConnectPacket) -> Self {
        return Self {
            client_id: packet.client_id().to_string(),
            conn_t: ConnectionType::Tcp,
            protocol: Protocol::Mqtt(packet.version()),
            will: packet.will,
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: vec![],
            qos1_packets: AtLeastOnceList::new(),
            qos2_packets: ExactlyOnceList::new(),
            id_gen: IdGenerator::new(IdGenType::Broker),
        };
    }

    pub fn will(&self) -> &Option<Will> {
        return &self.will;
    }

    pub fn update_last_read(&mut self) {
        self.last_read = Instant::now();
    }

    pub fn timed_out(&self) -> bool {
        return self.last_read.elapsed().as_secs() > self.keep_alive;
    }

    /// Returns true if the connection type is a stateless connection where packet retries are not handled by the internal protocol
    pub fn should_retry_packets(&self) -> bool {
        match self.conn_t {
            ConnectionType::Tcp => false,
        }
    }

    pub async fn retry_packets<S: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
    ) -> Result<(), ServerError> {
        let mut buf = BytesMut::new();

        for packet in self.qos1_packets.iter_mut() {
            if packet.should_retry() {
                if let Some(retry_packet) = packet.get_retry_packet() {
                    match retry_packet {
                        MqttPacket::Publish(mut packet) => {
                            // indicate to the client that this is a re-transmission.
                            packet.set_dup(true);
                            buf.put_slice(&packet.encode()?);
                        }
                        _ => {
                            buf.put_slice(&retry_packet.encode()?);
                        }
                    }
                    packet.update_retry_duration();
                }
            }
        }

        for packet in self.qos2_packets.iter_mut() {
            if packet.should_retry() {
                if let Some(retry_packet) = packet.get_retry_packet() {
                    buf.put_slice(&retry_packet.encode()?);
                    packet.update_retry_duration();
                }
            }
        }

        if buf.len() > 0 {
            stream.write_all(&buf).await?;
        }

        return Ok(());
    }

    pub fn clean_session(&mut self) {
        const CLEAN_MAX: usize = u16::MAX as usize / (2 as i32).pow(4) as usize;

        if self.qos1_packets.len() > CLEAN_MAX {
            let ids = self.qos1_packets.clean();
            for id in ids {
                self.id_gen.free_id(id);
            }
        }
        if self.qos2_packets.len() > CLEAN_MAX {
            let ids = self.qos2_packets.clean();
            for id in ids {
                self.id_gen.free_id(id);
            }
        }
    }

    pub fn ack(&mut self, packet_id: u16) -> PubAckPacket {
        self.qos1_packets.acknowledge(packet_id);
        return PubAckPacket::new(packet_id);
    }

    pub fn next_id(&mut self) -> Option<u16> {
        return self.id_gen.next_id();
    }

    pub fn origin(&mut self, packet: &Arc<PublishPacket>) -> PublishPacket {
        let new_id = self.next_id();
        match packet.qos() {
            QosLevel::AtMostOnce => {
                let packet = (**packet).clone();
                return packet;
            }
            QosLevel::AtLeastOnce => {
                return self.qos1_packets.origin(packet.clone(), new_id.unwrap());
            }
            QosLevel::ExactlyOnce => {
                return self.qos2_packets.origin(packet.clone(), new_id.unwrap());
            }
        }
    }

    pub fn publish(&mut self, packet: PublishPacket) -> Option<PubRecPacket> {
        let id = packet.id().unwrap();
        return self.qos2_packets.publish(packet, id);
    }

    pub fn rec(&mut self, packet_id: u16) -> Option<PubRelPacket> {
        self.qos2_packets.receive(packet_id)
    }

    pub fn rel(&mut self, packet_id: u16) -> Option<Arc<PublishPacket>> {
        self.qos2_packets.release(packet_id)
    }

    pub fn comp(&mut self, packet_id: u16) {
        self.qos2_packets.complete(packet_id);
    }
}

// TODO:
// Okay... this type signature is disgusting...
impl TryFrom<(DisconnectedSession, ConnectPacket)> for ActiveSession {
    type Error = ServerError;
    fn try_from(
        (dc_session, packet): (DisconnectedSession, ConnectPacket),
    ) -> Result<Self, Self::Error> {
        let mut id_gen = IdGenerator::new(IdGenType::Broker);

        for packet in dc_session.qos1_packets.iter() {
            id_gen.set_id(packet.id());
        }

        for packet in dc_session.qos2_packets.iter() {
            id_gen.set_id(packet.id());
        }

        return Ok(Self {
            client_id: packet.client_id.to_owned(),
            protocol: dc_session.protocol,
            conn_t: dc_session.conn_t,
            will: packet.will.to_owned(),
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: dc_session.topic_filters.clone(),
            id_gen,
            qos1_packets: dc_session.qos1_packets,
            qos2_packets: dc_session.qos2_packets,
        });
    }
}

impl From<ActiveSession> for DisconnectedSession {
    fn from(value: ActiveSession) -> Self {
        Self {
            client_id: value.client_id,
            conn_t: value.conn_t,
            protocol: value.protocol,
            keep_alive: value.keep_alive,
            last_read: value.last_read,
            qos1_packets: value.qos1_packets,
            qos2_packets: value.qos2_packets,
            topic_filters: value.topic_filters,
        }
    }
}

/// This structure is used to save connection state after a disconnection where the connection is established
/// with the clean_session bitflag set to false.
#[derive(Clone)]
pub struct DisconnectedSession {
    client_id: String,
    protocol: Protocol,
    conn_t: ConnectionType,
    keep_alive: u64,
    last_read: Instant,
    qos1_packets: AtLeastOnceListType,
    qos2_packets: ExactlyOnceListType,
    topic_filters: Vec<TopicFilter>,
}

impl PartialEq for DisconnectedSession {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl DisconnectedSession {
    pub fn expired(&self) -> bool {
        // setting the keep_alive value to zero has the effect of disabling session expiry.
        if self.keep_alive == 0 {
            return false;
        }

        return self.last_read.elapsed().as_secs() > self.keep_alive;
    }

    pub fn client_id<'a>(&'a self) -> &'a str {
        return self.client_id.as_str();
    }

    pub fn into_active(self, packet: ConnectPacket) -> Result<ActiveSession, ServerError> {
        return ActiveSession::try_from((self, packet));
    }
}

pub struct DisconnectedSessions {
    dc_sessions: HashMap<String, DisconnectedSession>,
}

impl DisconnectedSessions {
    pub fn new() -> Self {
        return Self {
            dc_sessions: HashMap::new(),
        };
    }

    // pub fn len(&self) -> usize {
    //     return self.dc_sessions.len();
    // }

    pub fn clean_expired(&mut self) {
        let expired = self.find_sessions(|x| x.expired());

        for client_id in expired {
            self.dc_sessions.remove(&client_id);
        }
    }

    /// Returns the session keys where Fn() evaluates to true.
    pub fn find_sessions(&self, cb: impl Fn(&DisconnectedSession) -> bool) -> Vec<String> {
        let mut out = vec![];

        for (client_id, session) in self.dc_sessions.iter() {
            if cb(session) {
                out.push(client_id.clone());
            }
        }
        return out;
    }

    pub fn add_session(&mut self, session: DisconnectedSession) {
        let client_id = session.client_id().to_string();
        self.dc_sessions.insert(client_id, session);
    }

    pub fn remove_session(&mut self, id: &str) -> Option<DisconnectedSession> {
        return self.dc_sessions.remove(id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConnectionType {
    Tcp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Protocol {
    Mqtt(MqttVersion),
}
