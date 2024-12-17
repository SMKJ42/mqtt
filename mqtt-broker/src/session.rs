use bytes::{BufMut, BytesMut};
use mqtt_core::err::server::ServerError;
use mqtt_core::id::{IdGenType, IdGenerator};
use mqtt_core::qos::QosLevel;
use mqtt_core::topic::TopicFilter;
use r2d2_sqlite::SqliteConnectionManager;
use sheesh::harness::sqlite::user::SqliteHarnessUser;
use sheesh::harness::stateless::{StatelessSession, StatelessToken};
use sheesh::harness::DbHarness;
use sheesh::id::DefaultIdGenerator;
use sheesh::session::{SessionManager, SessionManagerConfig};
use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use mqtt_core::msg_assurance::{AtLeastOnceList, ExactlyOnceList, RetryDuration};
use mqtt_core::v3::{
    ConnectPacket, MqttPacket, PubAckPacket, PubRecPacket, PubRelPacket, PublishPacket, Will,
};

pub type AtLeastOnceListType = AtLeastOnceList<Arc<PublishPacket>, Instant, RetryDuration>;
pub type ExactlyOnceListType = ExactlyOnceList<Arc<PublishPacket>, Instant, RetryDuration>;

#[derive(Debug, Clone)]
pub struct ActiveSession {
    client_id: String,
    will: Option<Will>,
    keep_alive: u64,
    last_read: Instant,
    topic_filters: Vec<TopicFilter>,
    qos1_packets: AtLeastOnceListType,
    qos2_packets: ExactlyOnceListType,
    id_gen: IdGenerator,
}

impl ActiveSession {
    pub fn from_packet(packet: ConnectPacket) -> Self {
        Self {
            client_id: packet.client_id().to_string(),
            will: packet.will,
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: vec![],
            qos1_packets: AtLeastOnceList::new(),
            qos2_packets: ExactlyOnceList::new(),
            id_gen: IdGenerator::new(IdGenType::Broker),
        }
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
        self.qos2_packets.relay(packet_id)
    }

    pub fn comp(&mut self, packet_id: u16) {
        self.qos2_packets.complete(packet_id);
    }

    // pub fn comp(&mut self, packet_id: u16)
}

impl From<(DisconnectedSession, ConnectPacket)> for ActiveSession {
    fn from((dc_session, packet): (DisconnectedSession, ConnectPacket)) -> Self {
        let mut id_gen = IdGenerator::new(IdGenType::Broker);

        for packet in dc_session.qos1_packets.iter() {
            id_gen.set_id(packet.id());
        }

        for packet in dc_session.qos2_packets.iter() {
            id_gen.set_id(packet.id());
        }

        Self {
            client_id: packet.client_id.to_owned(),
            will: packet.will.to_owned(),
            keep_alive: packet.keep_alive.into(),
            last_read: Instant::now(),
            topic_filters: dc_session.topic_filters.clone(),
            id_gen,
            qos1_packets: dc_session.qos1_packets,
            qos2_packets: dc_session.qos2_packets,
        }
    }
}

impl From<ActiveSession> for DisconnectedSession {
    fn from(value: ActiveSession) -> Self {
        Self {
            client_id: value.client_id,
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
    // these values represent packets that were sent after a disconnect.
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

    pub fn into_active(self, packet: ConnectPacket) -> ActiveSession {
        return ActiveSession::from((self, packet));
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

/*
 *
 *  ---------- USER AUTHENTICATION ----------
 *
 */

use std::fmt::Display;

use sheesh::user::{
    PrivateUserMeta, PublicUserMeta, Role, UserManager, UserManagerConfig, UserManagerError,
    UserManagerErrorKind, UserMeta,
};

// the following trait impls create type safety for you across the application.
pub enum Roles {
    Admin,
}

impl Roles {
    pub fn to_string(&self) -> String {
        match self {
            Self::Admin => return String::from("admin"),
        }
    }

    pub fn as_role(&self) -> Role {
        return Role::from_string(self.to_string());
    }
}

impl Display for Roles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => {
                write!(f, "admin")
            }
        }
    }
}
#[derive(Clone)]
pub struct MyPublicUserMetadata;
impl PublicUserMeta for MyPublicUserMetadata {
    // fn from_values(values: &mut slice::Iter<'_, String>) -> Option<Self> {
    //     None
    // }
    // fn into_values(&self) -> Vec<String> {
    //     vec![]
    // }
}

#[derive(Clone)]
pub struct MyPrivateUserMetadata;
impl PrivateUserMeta for MyPrivateUserMetadata {
    // fn from_values(values: &mut slice::Iter<'_, String>) -> Option<Self> {
    //     None
    // }
    // fn into_values(&self) -> Vec<String> {
    //     vec![]
    // }
}

pub struct AuthManager {
    user: UserManager<DefaultIdGenerator, SqliteHarnessUser>,
    session: SessionManager<DefaultIdGenerator, StatelessSession, StatelessToken>,
}

impl AuthManager {
    pub fn new(path: PathBuf) -> Self {
        let conn_manager = SqliteConnectionManager::file(path);
        let pool = r2d2::Pool::new(conn_manager).unwrap();
        let harness = DbHarness::new_stateless_sqlite(pool);

        harness.init_tables().unwrap();

        let user = UserManagerConfig::default().init(harness.user);
        let session = SessionManagerConfig::default().init(harness.session, harness.token);

        return Self { user, session };
    }

    pub fn verify_credentials(&self, username: &str, pwd: &str) -> Result<UserMeta, ServerError> {
        match self.user.login(&self.session, username, pwd) {
            Ok((user, _, _)) => return Ok(user),
            Err(err) => {
                todo!();
            }
        }
    }
}
