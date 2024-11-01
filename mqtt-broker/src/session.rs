use std::{collections::HashMap, sync::Arc, time::Instant};

use bytes::{Bytes, BytesMut};
use mqtt_core::topics::TopicFilter;
use mqtt_packets::{
    decode_packet,
    v3::{
        connect::{ConnectPacket, Will},
        publish::PublishPacket,
        pubrec::PubRecPacket,
        pubrel::PubRelPacket,
        FixedHeader, MqttPacket,
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::client::ClientError;

// end state is PUBACK, but we do not need to maintain that state, only send the packet

#[derive(Clone)]
enum PacketExactlyOnce {
    Packet(Arc<PublishPacket>),
    Rec { id: u16, packet: Arc<PublishPacket> },
    Rel { id: u16, packet: Arc<PublishPacket> },
    // end state is PUBCOMP, but we do not need to maintain that state, only send the packet
}

impl PacketExactlyOnce {
    fn packet(self) -> Arc<PublishPacket> {
        match self {
            Self::Packet(packet) => packet,
            Self::Rec { id: _, packet } => packet,
            Self::Rel { id: _, packet } => packet,
        }
    }
}

impl TryFrom<&PacketExactlyOnce> for Bytes {
    type Error = ClientError;

    fn try_from(value: &PacketExactlyOnce) -> Result<Self, Self::Error> {
        match value {
            PacketExactlyOnce::Packet(packet) => Ok(packet.encode()?),
            PacketExactlyOnce::Rec { id, packet: _ } => Ok(PubRecPacket::new(id.clone()).encode()),
            PacketExactlyOnce::Rel { id, packet: _ } => Ok(PubRelPacket::new(id.clone()).encode()),
        }
    }
}

#[derive(Clone)]
struct PacketAtMostOnce(Arc<PublishPacket>);

impl PacketAtMostOnce {
    fn packet(self) -> Arc<PublishPacket> {
        return self.0;
    }
}

impl TryFrom<&PacketAtMostOnce> for Bytes {
    type Error = ClientError;

    fn try_from(value: &PacketAtMostOnce) -> Result<Self, Self::Error> {
        return Ok(value.0.encode()?);
    }
}

#[derive(Clone)]
pub struct ActiveSession {
    client_id: String,
    will_retain: bool,
    will: Option<Will>,
    keep_alive: u64,
    last_read: Instant,
    topic_filters: Vec<TopicFilter>,
    qos1_packets: Vec<PacketAtMostOnce>,
    qos2_packets: Vec<PacketExactlyOnce>,
}

impl ActiveSession {
    pub fn from_packet(packet: ConnectPacket) -> Self {
        Self {
            will_retain: packet.will_retain(),
            client_id: packet.client_id().to_string(),
            will: packet.will,
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: vec![],
            qos1_packets: vec![],
            qos2_packets: vec![],
        }
    }

    pub fn will(self) -> Option<Will> {
        return self.will;
    }

    pub fn update_last_read(&mut self) {
        self.last_read = Instant::now();
    }

    pub fn timed_out(&self) -> bool {
        return self.last_read.elapsed().as_secs() > self.keep_alive;
    }

    pub async fn retry_packets(&self, stream: &mut TcpStream) -> Result<(), ClientError> {
        for packet in &self.qos1_packets {
            let buf: Bytes = packet.try_into()?; // Uses TryFrom<PacketExactlyOnce> for Bytes
            stream.write_all(&buf).await?;
        }

        for packet in &self.qos2_packets {
            let buf: Bytes = packet.try_into()?;
            stream.write_all(&buf).await?
        }

        return Ok(());
    }

    pub fn will_retain(&self) -> bool {
        return self.will_retain;
    }
}

impl From<(&DisconnectedSession, &ConnectPacket)> for ActiveSession {
    fn from((dc_session, packet): (&DisconnectedSession, &ConnectPacket)) -> Self {
        Self {
            client_id: packet.client_id.to_owned(),
            will_retain: packet.will_retain(),
            will: packet.will.to_owned(),
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: dc_session.topic_filters.clone(),
            qos1_packets: dc_session
                .qos1_packets
                .iter()
                .map(|x| PacketAtMostOnce(x.clone()))
                .collect(),
            qos2_packets: dc_session
                .qos2_packets
                .iter()
                .map(|x| PacketExactlyOnce::Packet(x.clone()))
                .collect(),
        }
    }
}

impl From<ActiveSession> for DisconnectedSession {
    fn from(value: ActiveSession) -> Self {
        Self {
            client_id: value.client_id,
            qos1_packets: value.qos1_packets.into_iter().map(|x| x.packet()).collect(),
            qos2_packets: value.qos2_packets.into_iter().map(|x| x.packet()).collect(),
            topic_filters: value.topic_filters,
        }
    }
}

/// This structure is used to save connection state after a disconnection where the connection is established
/// with the clean_session bitflag set to false.
#[derive(Clone)]
pub struct DisconnectedSession {
    client_id: String,
    // these values represent packets that were sent after a disconnect.
    qos1_packets: Vec<Arc<PublishPacket>>,
    qos2_packets: Vec<Arc<PublishPacket>>,
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
    pub fn new(client_id: String) -> Self {
        return Self {
            client_id: client_id,

            qos1_packets: vec![],
            qos2_packets: vec![],
            topic_filters: vec![],
        };
    }

    pub fn client_id<'a>(&'a self) -> &'a str {
        return self.client_id.as_str();
    }

    pub fn into_active(&self, packet: &ConnectPacket) -> ActiveSession {
        return ActiveSession::from((self, packet));
    }

    // pub fn push_topic_filter(&mut self, filter: TopicFilter) {
    //     self.topic_filters.push(filter);
    // }

    // pub async fn publish_will(&self, subs: Subscribers) -> Result<(), MqttServerError> {
    //     match &self.will {
    //         Some(will) => {
    //             let mut will_packet = PublishPacket::new(
    //                 will.will_topic().clone(),
    //                 Bytes::from(will.will_topic().clone().to_string()),
    //             );
    //             match will.will_qos() {
    //                 QosLevel::AtMostOnce => will_packet.set_qos_atmostonce(),
    //                 QosLevel::AtLeastOnce => {
    //                     todo!("Need to create a packet_id generator")
    //                     // will_packet.set_qos_atleastonce(packet_id);
    //                 }
    //                 QosLevel::ExactlyOnce => {
    //                     todo!("Need to create a packet_id generator")

    //                     // will_packet.set_qos_exactlyonce(packet_id);
    //                 }
    //             };

    //             // todo!("Need to create a collection for subscribers.");
    //             for subscriber in subs {
    //                 match subscriber.try_lock() {
    //                     Ok(mut subscriber) => {
    //                         subscriber.write_packet(MqttPacket::Publish(will_packet.clone()))?;
    //                     }
    //                     Err(err) => {
    //                         todo!()
    //                     }
    //                 }
    //             }
    //         }
    //         None => {}
    //     }

    //     return Ok(());
    // }

    // pub async fn read_packet(&mut self) -> Result<MqttPacket, MqttServerError> {
    //     return read_raw_packet(&mut self.stream);
    // }

    // Subscribers are provided in the case of a failed connection,
    // that way the connection's will can be published.
    // pub fn write_packet(&mut self, packet: MqttPacket) -> Result<(), MqttServerError> {
    //     match packet {
    //         MqttPacket::Publish(pub_packet) => {
    //             let mut bytes = pub_packet.encode()?;
    //             match pub_packet.qos() {
    //                 QosLevel::AtMostOnce => match self.stream.write_all(&mut bytes) {
    //                     Ok(_) => return Ok(()),
    //                     Err(err) => match err.kind() {
    //                         _ => return Err(err.into()),
    //                     },
    //                 },
    //                 QosLevel::AtLeastOnce => {
    //                     todo!()
    //                 }
    //                 QosLevel::ExactlyOnce => {
    //                     todo!()
    //                 }
    //             }
    //         }
    //         _ => match self.stream.write_all(&mut MqttPacket::encode(&packet)?) {
    //             Ok(_) => return Ok(()),
    //             Err(err) => match err.kind() {
    //                 _ => return Err(err.into()),
    //             },
    //         },
    //     }
    // }
}

pub struct DisconnectedSessions {
    dc_session: HashMap<String, DisconnectedSession>,
}

impl DisconnectedSessions {
    pub fn new() -> Self {
        return Self {
            dc_session: HashMap::new(),
        };
    }

    pub fn len(&self) -> usize {
        return self.dc_session.len();
    }

    pub fn add_session(&mut self, session: DisconnectedSession) {
        let client_id: String;

        {
            client_id = session.client_id().to_string();
        }

        let insert_res = self.dc_session.insert(client_id, session);

        match insert_res {
            Some(value) => {
                todo!("connection id is already in use")
            }
            None => {}
        }
    }

    pub fn remove_session(&mut self, id: &str) -> Option<DisconnectedSession> {
        return self.dc_session.remove(id);
    }

    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut DisconnectedSession> {
        if let Some(session) = self.dc_session.get_mut(id) {
            return Some(session);
        } else {
            return None;
        }
    }

    pub fn get_session(&self, id: &str) -> Option<&DisconnectedSession> {
        if let Some(session) = self.dc_session.get(id) {
            return Some(session);
        } else {
            return None;
        }
    }

    pub fn has_session(&self, session: &str) -> bool {
        todo!()
    }

    pub async fn flush(&self) {
        todo!()
    }
}

pub async fn read_raw_packet(stream: &mut TcpStream) -> Result<MqttPacket, ClientError> {
    let mut buf = [0; 5];
    stream.peek(&mut buf).await?;
    match FixedHeader::decode(Bytes::from_iter(buf)) {
        Ok((f_header, buf)) => {
            // size the buf for packet, then read the bytes into the buf.
            let mut buf = BytesMut::with_capacity(f_header.len() + buf.len());
            stream.read_exact(&mut buf).await?;
            match decode_packet(f_header, &buf) {
                Ok(packet) => return Ok(packet),
                Err(err) => return Err(err.into()),
            };
        }
        Err(err) => Err(err.into()),
    }
}
