mod error;
mod mailbox;
mod session;
mod topic;

use bytes::BytesMut;
use error::{
    client::{self, ClientError},
    server,
};
use mailbox::{Mail, Mailbox};
use mqtt_core::{
    qos::QosLevel,
    topics::{TopicFilter, TopicName},
};
use mqtt_packets::v3::{
    conack::{ConnAckPacket, ConnectReturnCode},
    pingresp::PingRespPacket,
    publish::PublishPacket,
    MqttPacket,
};
use session::{read_raw_packet, ActiveSession, DisconnectedSessions};
use std::{io, sync::Arc};
use tokio::sync::broadcast::error::SendError;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use topic::ServerTopics;

struct MqttServer {
    topics: Arc<Mutex<ServerTopics>>,
    dc_sessions: Arc<Mutex<DisconnectedSessions>>,
}

impl MqttServer {
    /// Creates a new MqttServer instance holding a mutex to topics and a mutex to disconnected sessions.
    fn new() -> Self {
        MqttServer {
            topics: Arc::new(Mutex::new(ServerTopics::new())),
            dc_sessions: Arc::new(Mutex::new(DisconnectedSessions::new())),
        }
    }

    /// returns an option to the channels receiver.
    async fn get_topic_channel(
        &self,
        topic_name: &TopicName,
    ) -> Option<broadcast::Sender<Arc<PublishPacket>>> {
        let topics = self.topics.lock().await;
        topics.get_channel(topic_name)
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
    async fn publish(
        &self,
        topic: &TopicName,
        message: Arc<PublishPacket>,
    ) -> Result<usize, SendError<Arc<PublishPacket>>> {
        // Send only fails when there are no receivers, and returns the message wrapped in an Err to allow for recovery.
        // We don't need to recover from this state, so just discard the error.

        match self.get_topic_channel(topic).await {
            Some(topic) => {
                return topic.send(message);
            }
            None => return Ok(0),
        }
    }

    /// Subscribe to a topic.
    ///
    /// This function forwards any retained messages that the server holds for that topic,
    /// and returns a mailbox with receivers to the subscriptions.
    ///
    /// ## Error
    ///
    /// It will error if the retained messages cannot be written to the stream
    async fn subscribe(
        &self,
        mut stream: &mut TcpStream,
        topic_filter: &TopicFilter,
    ) -> Result<Mailbox, ClientError> {
        let topics = self.topics.lock().await;

        let mut mailbox = Mailbox::new();

        for (topic_name, topic) in topics.iter() {
            if topic_name == topic_filter {
                let (receiver, retained_messages) = topic.subscribe();
                mailbox.queue(Mail::new(topic_name.clone(), receiver));
                retained_messages.publish(&mut stream).await?
            }
        }
        return Ok(mailbox);
    }
}

/// Handle a single TCP client connection event loop.
async fn handle_client(
    mut stream: TcpStream,
    mailbox: &mut Mailbox,
    server: Arc<MqttServer>,
) -> Result<(), ClientError> {
    let connect_packet = read_init_packet(&server, &mut stream, mailbox).await?;

    if let Some(mut session) = connect_packet {
        match handle_client_connection(stream, &server, &mut session, mailbox).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                let is_retained = session.will_retain();
                // handle unintended disconnections for packets with clean_session set to false.
                // Clean session set to false implies that the session is intended to continue after a disconnect,
                // and publish packets with a QoS greater than 0 should be stored for delivery later.
                if !is_retained {
                    let mut dc_sessions = server.dc_sessions.lock().await;

                    dc_sessions.add_session(session.clone().into());
                }

                match &session.will() {
                    Some(will) => {
                        let packet: PublishPacket = (will, 69).into();
                        let _ = server
                            .publish(&packet.topic().clone(), Arc::new(packet))
                            .await;
                    }
                    None => {}
                }

                return Err(err);
            }
        }
    } else {
        // we received a PINGREQ so close the connection.
        return Ok(());
    }
}

/// Handle a MQTT client connection event loop after CONNECT packet receipt.
async fn handle_client_connection(
    mut stream: TcpStream,
    server: &Arc<MqttServer>,
    session: &mut ActiveSession,
    mailbox: &mut Mailbox,
) -> Result<(), ClientError> {
    loop {
        let mut buf = BytesMut::new();

        match stream.try_read(&mut buf) {
            Ok(num_bytes) => {
                let packets = read_raw_packet(&mut buf.into()).await?;

                if session.timed_out() {
                    return Ok(());
                } else {
                    session.update_last_read();
                }
                handle_packet(server, &mut stream, mailbox, packets).await?
            }

            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        }

        // Example: treat all input as publish to the topic

        for mail in mailbox.mail_mut() {
            // read all mail in this receiver.
            forward_mail(mail, &mut stream).await?;
        }
    }
}

async fn handle_packet(
    server: &Arc<MqttServer>,
    stream: &mut TcpStream,
    mailbox: &mut Mailbox,
    packets: Vec<MqttPacket>,
) -> Result<(), ClientError> {
    for mqtt_packet in packets {
        match mqtt_packet {
            MqttPacket::Subscribe(packet) => {
                handle_subscribe(&server, stream, mailbox, packet.topic_filters()).await;
            }
            MqttPacket::Publish(packet) => {
                if packet.retain() {
                    let mut topics = server.topics.lock().await;
                    if let Some(topic) = topics.topic_mut(packet.topic()) {
                        topic.retain_message(Arc::new(packet.clone()));
                    }
                }

                let _ = server
                    .publish(&packet.topic().clone(), Arc::new(packet))
                    .await;
            }
            MqttPacket::PingReq(_packet) => {
                stream.write_all(&PingRespPacket::new().encode()).await?;
            }
            MqttPacket::PubAck(packet) => {
                // update session
                todo!()
            }
            MqttPacket::PubRec(packet) => {
                // update session
                //send pubrel
                todo!();
            }
            MqttPacket::PubRel(packet) => {
                // update session
                // send pubcomp
                todo!();
            }
            MqttPacket::PubComp(packet) => {
                // update session
                todo!();
            }
            MqttPacket::Unsubscribe(packet) => {
                for filter in packet.filters() {
                    mailbox.remove(filter);
                }
            }
            MqttPacket::Disconnect(packet) => {
                return Ok(());
            }
            _ => {
                // Received invalid packet, drop the
                return Err(ClientError::new(
                    client::ErrorKind::ProtocolError,
                    String::from("MQTT Broker does not support packet type."),
                ));
            }
        }
    }
    return Ok(());
}

pub async fn forward_mail(mail: &mut Mail, stream: &mut TcpStream) -> Result<(), ClientError> {
    loop {
        match mail.recv() {
            Ok(message) => match message {
                Some(message) => {
                    let packet = message.encode()?;
                    // forward received mail to the client.
                    if let Err(err) = stream.write_all(&packet).await {
                        return Err(ClientError::new(
                            err.into(),
                            String::from("Could not write to client's stream."),
                        ));
                    } else {
                        // read mail until it is empty.
                        continue;
                    }
                }
                // no more mail in the receiver, move on to the next receiver.
                None => return Ok(()),
            },
            Err(err) => {
                eprintln!("{:?}", err);
                match err.kind() {
                    // if we have a full mailbox, retry again. We will drop some packets,
                    // but if QoS > 0 the client will initiate a retry attempt.
                    server::ErrorKind::FullMailbox => continue,
                    server::ErrorKind::BroadcastError => {
                        todo!("create a logger");
                    }
                };
            }
        }
    }
}

/// Reads the initial packet sent from the client
///
/// The first packet from the client should be a CONNECT packet or a PINGREQ packet,
/// if it is neither the function will return an error.
///
/// if a PINGREQ packet is the first packet sent, the function will return an Ok(None) value.
///
/// If a CONNECT packet is the first packet sent, the function will return an Ok(Session) value.
///
/// This function is part of the main event loop of the client's connection instance.
///

async fn read_init_packet(
    server: &Arc<MqttServer>,
    stream: &mut TcpStream,
    mailbox: &mut Mailbox,
) -> Result<Option<ActiveSession>, ClientError> {
    let mut buf = BytesMut::new();

    stream.read_buf(&mut buf).await?;

    let mut packets = read_raw_packet(&mut buf.into()).await?.into_iter();

    let session: ActiveSession;

    if let Some(packet) = packets.next() {
        match packet {
            MqttPacket::PingReq(_) => {
                match stream.write_all(&mut PingRespPacket::new().encode()).await {
                    Ok(_) => return Ok(None),
                    Err(err) => {
                        return Err(ClientError::new(
                            err.into(),
                            String::from("Could not write to client's stream."),
                        ))
                    }
                }
            }

            MqttPacket::Connect(packet) => {
                let mut sessions = server.dc_sessions.lock().await;

                // clear the client's prior history and shift dc_session into active session.
                if let Some(dc_session) = sessions.remove_session(packet.client_id()) {
                    // Client did NOT disconnect gracefully, send any stored packets.
                    if packet.clean_session() {
                        stream
                            .write_all(
                                &ConnAckPacket::new(true, ConnectReturnCode::Accept).encode(),
                            )
                            .await?;
                        session = dc_session.into_active(&packet);
                    } else {
                        // It is the first time the client is connecting, or the client disconnected gracefully.
                        stream
                            .write_all(
                                &ConnAckPacket::new(false, ConnectReturnCode::Accept).encode(),
                            )
                            .await?;

                        session = ActiveSession::from_packet(packet);
                    }
                } else {
                    // It is the first time the client is connecting, or the client disconnected gracefully.
                    stream
                        .write_all(&ConnAckPacket::new(false, ConnectReturnCode::Accept).encode())
                        .await?;

                    session = ActiveSession::from_packet(packet);
                }
            }
            _ => {
                return Err(ClientError::new(
                    client::ErrorKind::ProtocolError,
                    String::from(
                        "Cannot initialize connection without first receiving a CONNECT packet",
                    ),
                ))
            }
        }
    } else {
        return Err(ClientError::new(
            client::ErrorKind::ProtocolError,
            String::from("Cannot initialize connection without first receiving a CONNECT packet"),
        ));
    }

    handle_packet(&server, stream, mailbox, packets.collect()).await?;

    return Ok(Some(session));
}

/// Handles updating the server state with new subscriptions and writing retained messages to the clients stream,
///
/// This function is part of the main event loop of the client's connection instance.
///
/// ## Error
///
/// It will error if the retained messages cannot be written to the stream
async fn handle_subscribe(
    server: &Arc<MqttServer>,
    stream: &mut TcpStream,
    mailbox: &mut Mailbox,
    filters: Vec<(TopicFilter, QosLevel)>,
) {
    for (filter, _qos) in filters {
        let new_topics_chunk = server.subscribe(stream, &filter).await;
        match new_topics_chunk {
            Ok(mut new_topics_chunk) => {
                mailbox.combine(&mut new_topics_chunk);
            }
            Err(err) => {
                todo!()
            }
        }
    }
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let server = Arc::new(MqttServer::new());

    // Start TCP listener
    let listener = TcpListener::bind("127.0.0.1:1883").await?;
    println!("Server listening on 127.0.0.1:1883");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("New connection: {}", addr);

        let server_clone = Arc::clone(&server);
        // Handle each client connection
        tokio::spawn(async move {
            let mut mailbox = Mailbox::new();

            if let Err(err) = handle_client(stream, &mut mailbox, server_clone).await {
                eprintln!(
                    "Server error handling client, closing connection:\n{:?}",
                    err
                );
            } else {
                println!("Gracefully closing connection.");
            }
        });
    }
}
