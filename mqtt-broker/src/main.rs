mod config;
mod init;
mod logger;
mod mailbox;
mod session;
mod topic;

use core::str;
use std::{path::PathBuf, sync::Arc};

use bytes::Bytes;
use config::MqttConfig;
use init::MqttEnv;

use mqtt_core::{
    err::server::{self, ServerError},
    io::read_packet_with_timeout,
    qos::{QosLevel, SubAckQoS},
    topic::{TopicFilterResult, TopicName},
    v3::{
        ConnAckPacket, ConnectPacket, MqttPacket, PingRespPacket, PubAckPacket, PubCompPacket,
        PublishPacket, SubAckPacket, UnsubAckPacket,
    },
    ConnectReturnCode,
};

use sheesh::user::UserMeta;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    join,
    net::TcpListener,
    sync::{Mutex, RwLock},
};

use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tokio_rustls::TlsAcceptor;

use mailbox::Mailbox;
use session::{ActiveSession, AuthManager, DisconnectedSessions};
use topic::{subscribe_to_topic_filter, ServerTopics};

struct MqttServer {
    config: MqttConfig,
    // Mutex is only locked when first obtaining a broadcast receiver handle, or when a topic is added or removed.
    // Should this be a tokio mutex or a std mutex? I don't think contention will be terribly high...
    topics: Arc<RwLock<ServerTopics>>,
    // Mutex is only locked when a client disconnects or connects.
    // Should this be a tokio mutex or a std mutex? Contention will probably be higher than topics though...
    dc_sessions: Arc<Mutex<DisconnectedSessions>>,
    auth_manager: AuthManager,
}

impl MqttServer {
    /// Creates a new MqttServer instance holding a mutex to topics and a mutex to disconnected sessions.
    pub fn new(config: MqttConfig) -> Self {
        MqttServer {
            auth_manager: AuthManager::new(config.user_db()),
            topics: Arc::new(RwLock::new(ServerTopics::new(config.max_queued_messages()))),
            config: config,
            dc_sessions: Arc::new(Mutex::new(DisconnectedSessions::new())),
        }
    }

    pub async fn start(self) {
        let addr = self.config.addr();

        let listener = TcpListener::bind(&addr).await.unwrap();

        log::info!("Server listening at: {}", addr);

        if self.config.tls_enabled() {
            self.start_tls(listener).await;
        } else {
            self.start_plaintext(listener).await;
        }
    }

    async fn start_plaintext(self, listener: TcpListener) {
        let server = Arc::new(self);
        loop {
            server.clean_expired_sessions().await;
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    log::info!("New connection attempt: {addr}");

                    let server_clone = Arc::clone(&server);

                    tokio::spawn(async move {
                        if let Err(err) = handle_client(&server_clone, &mut stream).await {
                            log::warn!("Error handling client: {err}, Closing connection: {addr}")
                        } else {
                            log::info!("Gracefully closing connection: {addr}")
                        }
                    });
                }
                Err(err) => {
                    log::error!("Rejected TCP connection: {}", err);
                }
            }
        }
    }

    async fn start_tls(self, listener: TcpListener) {
        let server = Arc::new(self);

        let certs = CertificateDer::pem_file_iter("tls/cert.pem")
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        if certs.len() == 0 {
            log::warn!("No certificates were provided. Check the ./tls/cert.pem file")
        }

        let key = PrivateKeyDer::from_pem_file("tls/key.pem").unwrap();

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();

        let acceptor = TlsAcceptor::from(Arc::new(config));

        log::info!(
            "Initialized TLS on TCP listener at addr: {}",
            server.config.addr()
        );

        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();

            server.clean_expired_sessions().await;
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    log::info!("New connection attempt from: {addr}");

                    let server_clone = Arc::clone(&server);

                    let mut tls_stream = BufReader::new(tls_stream);

                    tokio::spawn(async move {
                        if let Err(err) = handle_client(&server_clone, &mut tls_stream).await {
                            log::error!("Error handling client: {err}");
                            log::warn!("Closing connection: {addr}")
                        } else {
                            if let Err(_) = tls_stream.shutdown().await {
                                log::error!("Did not gracefully close connection: {addr}")
                            } else {
                                log::info!("Gracefully closing connection: {addr}")
                            }
                        }
                    });
                }
                Err(err) => {
                    log::error!("{}", err);
                    log::warn!("Rejected TCP connection")
                }
            }
        }
    }

    /// Sends  to the broadcast channel for the given TopicName.
    ///
    /// ## Error result
    ///
    /// Attempts to send a value to all active [Receiver] handles, returning it back if it could not be sent.
    ///
    /// A successful send occurs when there is at least one active [Receiver] handle.
    /// An unsuccessful send would be one where all associated [Receiver] handles have already been dropped.
    /// If we encounter an error, it is intended that we ignore the error.
    async fn publish_to_topic(&self, topic: &TopicName, packet: Arc<PublishPacket>) {
        let mut topics = self.topics.write().await;

        match topics.topic_mut(topic) {
            Some(topic) => {
                // only fails when there are no receivers to be written to.
                // This is inteded behavior, so ignore the error and do not propogate.

                // TODO: We can use topic.channel().receivers() to find if there are no subs.
                // if there are none, we can persist to a db... this is similar to google's Pub/Sub service, but
                // it would also be cool to store all messages in a db and have them be queryable...

                let _ = topic.channel().send(packet);
            }
            None => {
                topics.create_topic(packet.topic().clone());
            }
        }
    }

    fn topics(&self) -> &Arc<RwLock<ServerTopics>> {
        return &self.topics;
    }

    async fn publish_will(&self, session: &mut ActiveSession) -> Result<(), ServerError> {
        if let Some(will) = session.will().clone() {
            let mut packet = PublishPacket::new(
                will.will_topic(),
                Bytes::copy_from_slice(will.will_message().as_bytes()),
            );

            match will.will_qos() {
                QosLevel::AtMostOnce => {}
                QosLevel::AtLeastOnce => {
                    let id = session.next_id();
                    if let Some(id) = id {
                        packet.set_qos_atleastonce(id);
                    } else {
                        session.clean_session();
                        packet.set_qos_atleastonce(
                            session.next_id().expect("No valid Id's available"),
                        );
                    }
                }
                QosLevel::ExactlyOnce => {
                    let id = session.next_id();
                    if let Some(id) = id {
                        packet.set_qos_exactlyonce(id);
                    } else {
                        session.clean_session();
                        packet.set_qos_exactlyonce(
                            session.next_id().expect("No valid Id's available"),
                        );
                    }
                }
            }

            if will.will_retain() {
                let retain_fut = self.retain_message(packet.clone());
                let pub_fut = self.publish_to_topic(packet.topic(), Arc::new(packet.clone()));

                join!(retain_fut, pub_fut);
            } else {
                self.publish_to_topic(packet.topic(), Arc::new(packet.clone()))
                    .await;
            }
        }

        return Ok(());
    }

    async fn retain_message(&self, packet: PublishPacket) {
        let mut topics = self.topics.write().await;
        topics.retain_message(packet);
    }

    async fn clean_expired_sessions(&self) {
        self.dc_sessions.lock().await.clean_expired();
    }
}

/// Handle a single TCP client connection event loop.
async fn handle_client<S: AsyncReadExt + AsyncWrite + Unpin>(
    server: &Arc<MqttServer>,
    stream: &mut S,
) -> Result<(), ServerError> {
    let active_session = handle_first_packet(&server, stream).await?;

    log::info!("connected");

    if let Some(mut session) = active_session {
        match handle_session(&server, stream, &mut session).await {
            Ok(()) => {
                return Ok(());
            }
            // If the session encounters an error, the error should close the connection.
            Err(err) => {
                let pub_to_server_fut = server.publish_will(&mut session);
                let dc_sessions_fut = server.dc_sessions.lock();

                let (_, mut dc_sessions) = join!(pub_to_server_fut, dc_sessions_fut);

                dc_sessions.add_session(session.into());

                return Err(err);
            }
        }
    } else {
        // we received a PINGREQ so close the connection.
        return Ok(());
    }
}

async fn handle_connect_packet<S: AsyncWriteExt + AsyncReadExt + Unpin>(
    server: &Arc<MqttServer>,
    packet: ConnectPacket,
    stream: &mut S,
) -> Result<ActiveSession, ServerError> {
    let mut connack = ConnAckPacket::new(false, ConnectReturnCode::Accept);

    // authenticate the request
    /*
     * TODO: the user is mut here becuase clippy isn't able to tell that the user can only be assigned once.
     * Maybe change the control flow for the function to safegaurd against inadvertant assignments to the user variable?
     */
    let mut user: Option<UserMeta> = None;

    if server.config.require_auth() {
        match (packet.username(), packet.password()) {
            (Some(username), Some(password)) => {
                let password = str::from_utf8(&password).unwrap();
                user = Some(server.auth_manager.verify_credentials(username, password)?);
            }
            _ => {
                return Err(ServerError::new(
                    server::ErrorKind::ConnectError(ConnectReturnCode::BadUsernameOrPassword),
                    String::from(
                        "Client attempted to connect without provided a username or password",
                    ),
                ))
            }
        }
    }

    let session: ActiveSession;

    let mut sessions = server.dc_sessions.lock().await;
    // Check if the server has a session history.
    if let Some(dc_session) = sessions.remove_session(packet.client_id()) {
        if packet.clean_session() {
            // The client requested a new session, drop the old session history and continue.
            session = ActiveSession::new(packet, user);
        } else {
            // The client requested to resume from a client's prior history.
            connack.set_session_present(true);
            session = dc_session.into_active(packet)?;
        }
    } else {
        // The server does not have any session history.
        session = ActiveSession::new(packet, user);
    }

    stream.write_all(&connack.encode()).await?;

    return Ok(session);
}

/// Reads the initial packet sent from the client
///
/// The first packet from the client should be a CONNECT packet or a PINGREQ packet,
/// if it is neither the function will return an error.
///
/// Any packets that were sent after the CONNECT packet, but before entry into the main event loop
/// will be handled as if the were sent in the main event loop
///
/// if a PINGREQ packet is the first packet sent, the function will return an Ok(None) value.
///
/// If a CONNECT packet is the first packet sent, the function will return an Ok(Session) value.
///
/// This function is NOT part of the main event loop of the client's connection instance.
///
async fn handle_first_packet<S: AsyncWriteExt + AsyncReadExt + Unpin>(
    server: &Arc<MqttServer>,
    stream: &mut S,
) -> Result<Option<ActiveSession>, ServerError> {
    loop {
        match read_packet_with_timeout::<_, ServerError>(stream).await {
            Ok(packet_opt) => {
                if let Some(packet) = packet_opt {
                    match packet {
                            MqttPacket::PingReq(_) => {
                                stream
                                    .write_all(&mut PingRespPacket::new().encode())
                                    .await?;
                                return Ok(None);
                            }

                            MqttPacket::Connect(packet) => {
                                // return Ok(Some( handle_connect().await));
                                return handle_connect_packet(server, packet, stream).await.map(|x| Some(x));
                            }
                            _ => {
                                return Err(ServerError::new(
                                    server::ErrorKind::ProtocolError,
                                    String::from(
                                        "Cannot initialize connection without first receiving a CONNECT packet",
                                    ),
                                ))
                            }
                        };
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }
}

/// Handle a MQTT client connection event loop after CONNECT packet receipt.
async fn handle_session<S: AsyncRead + AsyncWrite + Unpin>(
    server: &Arc<MqttServer>,
    mut stream: &mut S,
    session: &mut ActiveSession,
) -> Result<(), ServerError> {
    let mut mailbox = Mailbox::new();

    loop {
        // read in all packets.
        while let Some(packet) = read_packet_with_timeout::<_, ServerError>(stream).await? {
            if session.timed_out() {
                // if session has timed out, exit the main event loop
                return Ok(());
            } else {
                // if session has NOT timed out, update the last_read value of the session and continue the main event loop.
                session.update_last_read();
            }

            let should_shutdown =
                handle_packet(server, &mut stream, session, &mut mailbox, packet).await?;

            if should_shutdown {
                return Ok(());
            };
        }

        // WRITE all newly received packets
        for mail in mailbox.mail_mut() {
            // read all mail in this receiver.
            loop {
                match mail.recv() {
                    Ok(message) => match message {
                        Some(packet) => {
                            if mail.qos() == packet.qos() {
                                let buf = session.origin(&packet).encode()?;
                                // forward received mail to the client.
                                stream.write_all(&buf).await?;
                            } else {
                                match mail.qos().min(packet.qos()) {
                                    QosLevel::AtMostOnce => {
                                        let mut packet = (*packet).clone();
                                        packet.set_qos_atmostonce();
                                        stream.write_all(&packet.encode()?).await?;
                                    }
                                    QosLevel::AtLeastOnce => {
                                        // this has extra memory overhead. The message assurance might need some refractoring...
                                        let mut packet = (*packet).clone();
                                        packet.set_qos_atleastonce(0);
                                        session.origin(&Arc::new(packet.clone())).encode()?;
                                        stream.write_all(&packet.encode()?).await?;
                                    }
                                    QosLevel::ExactlyOnce => {
                                        unreachable!();
                                    }
                                }
                            }
                        }
                        // no more mail in the receiver, move on to the next receiver.
                        None => break,
                    },
                    Err(err) => {
                        match err.kind() {
                            server::ErrorKind::FullMailbox(count) => {
                                log::warn!("Mailbox was filled, lost {count} messages. Continuing to read from oldest available message.");
                                continue;
                            }
                            _ => {
                                log::error!("{}", err);
                            }
                        };
                    }
                }
            }
        }

        session.clean_session();
        // RETRY already sent packets that have not received a response after the timeout period.
        session.retry_packets(stream).await?;
    }
}

/// If the client disconnects gracefully return Ok(true), else returns Ok(false).
async fn handle_packet<S: AsyncRead + AsyncWrite + Unpin>(
    server: &Arc<MqttServer>,
    stream: &mut S,
    session: &mut ActiveSession,
    mailbox: &mut Mailbox,
    packet: MqttPacket,
) -> Result<bool, ServerError> {
    match packet {
        MqttPacket::Subscribe(packet) => {
            let mut resp: Vec<SubAckQoS> = vec![];

            if packet.topic_filters().len() == 0 {
                return Err(ServerError::new(
                    server::ErrorKind::ProtocolError,
                    String::from("Received SUBACK packet with no topic filters."),
                ));
            }

            for topic in packet.topic_filters() {
                match topic {
                    TopicFilterResult::Ok(sub) => {
                        let topics = server.topics().read().await;
                        if subscribe_to_topic_filter(stream, topics.iter(), session, mailbox, &sub)
                            .await?
                        {
                            resp.push(sub.qos().into());
                        } else {
                            resp.push(SubAckQoS::Err)
                        }
                    }
                    TopicFilterResult::Err => {
                        resp.push(SubAckQoS::Err);
                    }
                }
            }

            let mut buf = SubAckPacket::new(packet.id(), resp).encode()?;
            stream.write_all(&mut buf).await?;
        }
        MqttPacket::Publish(mut packet) => {
            packet.set_dup(false);

            // The retain flag has different meanings in the context which side is receiving the packet.
            // Therefore, the retain flag should be reset after the effects are handled by the broker.
            if packet.retain() {
                server.retain_message(packet.clone()).await;
            }

            packet.set_retain(false);

            match packet.qos() {
                QosLevel::ExactlyOnce => {
                    let res_packet = session.publish(packet);
                    match res_packet {
                        Some(packet) => {
                            // We received a new packet, send the appropriate response.
                            stream.write_all(&packet.encode()).await?;
                        }
                        None => {
                            // Do nothing, we already received the packet earlier. Wait for the timeout to hit.
                        }
                    }
                }
                QosLevel::AtLeastOnce => {
                    let id = packet.id().expect("At Least Once packet had no packet id");
                    let topic = packet.topic().clone();
                    let arc_packet = Arc::new(packet);
                    let buf = PubAckPacket::new(id).encode();

                    // if the server fails to respond, we want to fail the rest of the function, this
                    // allows the client to force a retry attempt on successive PUBLISH packets.
                    stream.write_all(&buf).await?;
                    server.publish_to_topic(&topic, arc_packet).await;
                }
                _ => {
                    let arc_packet = Arc::new(packet);
                    let _ = server
                        .publish_to_topic(&arc_packet.topic().clone(), arc_packet)
                        .await;
                }
            }
        }
        MqttPacket::PingReq(_packet) => {
            stream.write_all(&PingRespPacket::new().encode()).await?;
        }
        MqttPacket::PubAck(in_packet) => {
            let buf = session.ack(in_packet.id()).encode();
            stream.write_all(&buf).await?
        }
        MqttPacket::PubRec(in_packet) => {
            if let Some(packet) = session.rec(in_packet.id()) {
                stream.write_all(&packet.encode()).await?;
            }
        }
        MqttPacket::PubRel(in_packet) => {
            if let Some(forw_packet) = session.rel(in_packet.id()) {
                // if the server fails to respond, we want to fail the rest of the function, this
                // allows the client to force a retry attempt on successive PUBREL packets.
                stream
                    .write_all(&PubCompPacket::new(in_packet.id()).encode())
                    .await?;
                server
                    .publish_to_topic(&forw_packet.topic().clone(), forw_packet)
                    .await;
            } else {
                // if there is no current history of that packet, do nothing.
            }
        }
        MqttPacket::PubComp(in_packet) => {
            session.comp(in_packet.id());
        }
        MqttPacket::Unsubscribe(in_packet) => {
            for filter in in_packet.filters() {
                mailbox.remove(filter);
            }
            stream
                .write_all(&UnsubAckPacket::new(in_packet.id()).encode())
                .await?;
        }
        MqttPacket::Disconnect(_packet) => {
            stream.shutdown().await?;
            return Ok(true);
        }
        _ => {
            // Received invalid packet, drop the
            return Err(ServerError::new(
                server::ErrorKind::ProtocolError,
                String::from("MQTT Broker does not support packet type."),
            ));
        }
    }
    return Ok(false);
}

#[tokio::main]
async fn main() {
    let config_path = PathBuf::from("config.toml");
    let env = MqttEnv::new(&config_path).init();

    let server = MqttServer::new(env.config());
    server.start().await;
}
