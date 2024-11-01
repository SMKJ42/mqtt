use error::client::{self, ClientError};
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
use std::sync::Arc;
use tokio::sync::broadcast::{error::SendError, Receiver};
use tokio::sync::Mutex;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use topic::ServerTopics;

mod error;
mod session;
mod topic;

struct MqttServer {
    topics: Arc<Mutex<ServerTopics>>,
    dc_sessions: Arc<Mutex<DisconnectedSessions>>,
}

impl MqttServer {
    fn new() -> Self {
        MqttServer {
            topics: Arc::new(Mutex::new(ServerTopics::new())),
            dc_sessions: Arc::new(Mutex::new(DisconnectedSessions::new())),
        }
    }

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

    async fn subscribe(
        &self,
        mut stream: &mut TcpStream,
        topic_filter: &TopicFilter,
    ) -> Result<Vec<broadcast::Receiver<Arc<PublishPacket>>>, ClientError> {
        let topics = self.topics.lock().await;
        let mut channel_handles = vec![];

        for (topic_name, topic) in topics.iter() {
            if topic_name == topic_filter {
                let (channel, retained_messages) = topic.subscribe();
                channel_handles.push(channel);
                retained_messages.publish(&mut stream).await?
            }
        }
        return Ok(channel_handles);
    }
}

/// Handle a single TCP client connection event loop.
async fn handle_client(mut stream: TcpStream, server: Arc<MqttServer>) -> Result<(), ClientError> {
    let connect_packet = read_init_packet(&mut stream, &server.dc_sessions).await?;

    if let Some(mut session) = connect_packet {
        match handle_client_connection(stream, &server, &mut session).await {
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
) -> Result<(), ClientError> {
    let mut client_mailbox: Vec<Receiver<Arc<PublishPacket>>> = vec![];
    loop {
        let received = read_raw_packet(&mut stream).await?;

        if session.timed_out() {
            return Ok(());
        } else {
            session.update_last_read();
        }

        match received {
            MqttPacket::Subscribe(packet) => {
                handle_subscribe(
                    &server,
                    &mut stream,
                    &mut client_mailbox,
                    packet.topic_filters(),
                )
                .await;
            }
            MqttPacket::Publish(packet) => {
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
                // update mailbox
                todo!();
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
        // Example: treat all input as publish to the topic

        for receiver in client_mailbox.iter_mut() {
            while let Ok(message) = receiver.recv().await {
                let packet = message.encode()?;
                match stream.write_all(&packet).await {
                    Ok(_) => {}
                    Err(err) => {
                        return Err(ClientError::new(
                            err.into(),
                            String::from("Could not write to client's stream."),
                        ))
                    }
                }
            }
        }
    }
}

async fn read_init_packet(
    stream: &mut TcpStream,
    sessions: &Arc<Mutex<DisconnectedSessions>>,
) -> Result<Option<ActiveSession>, ClientError> {
    let packet = read_raw_packet(stream).await?;

    match packet {
        MqttPacket::PingReq(_) => {
            match stream.write_all(&mut PingRespPacket::new().encode()).await {
                Ok(_) => return Ok(None),
                Err(err) => Err(ClientError::new(
                    err.into(),
                    String::from("Could not write to client's stream."),
                )),
            }
        }

        MqttPacket::Connect(packet) => {
            let mut sessions = sessions.lock().await;

            // clear the client's prior history and shift dc_session into active session.
            if let Some(session) = sessions.remove_session(packet.client_id()) {
                // Client did NOT disconnect gracefully, send any stored packets.
                if packet.clean_session() {
                    stream
                        .write_all(&ConnAckPacket::new(true, ConnectReturnCode::Accept).encode())
                        .await?;
                    return Ok(Some(session.into_active(&packet)));
                }
            }
            // It is the first time the client is connecting, or the client disconnected gracefully.
            stream
                .write_all(&ConnAckPacket::new(false, ConnectReturnCode::Accept).encode())
                .await?;

            return Ok(Some(ActiveSession::from_packet(packet)));
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
}

async fn handle_subscribe(
    server: &Arc<MqttServer>,
    stream: &mut TcpStream,
    mailbox: &mut Vec<Receiver<Arc<PublishPacket>>>,
    filters: Vec<(TopicFilter, QosLevel)>,
) {
    for (filter, _qos) in filters {
        let new_topics_chunk = server.subscribe(stream, &filter).await;
        match new_topics_chunk {
            Ok(mut new_topics_chunk) => {
                mailbox.append(&mut new_topics_chunk);
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
        // Handle each client connection
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, server_clone).await {
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
