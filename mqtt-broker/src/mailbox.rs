use std::sync::Arc;

use mqtt_core::{
    qos::QosLevel,
    topic::{TopicFilter, TopicName},
    v4::PublishPacket,
};
use tokio::sync::broadcast::{error::TryRecvError, Receiver};

use mqtt_core::err::server::{self, ServerError};

#[derive(Debug)]
pub struct Mailbox(Vec<Mail>);

//TODO: do binary search ops on filters for easier access.
impl Mailbox {
    pub fn new() -> Self {
        return Self(vec![]);
    }

    pub fn add_slot(&mut self, mail: Mail) {
        if !self.0.contains(&mail) {
            self.0.push(mail);
        }
    }

    pub fn mail_mut(&mut self) -> &mut Vec<Mail> {
        return &mut self.0;
    }

    pub fn remove_slot(&mut self, filter: &TopicFilter) {
        while let Some(idx) = self
            .mail_mut()
            .iter()
            .position(|mail| mail.topic() == filter)
        {
            self.mail_mut().remove(idx);
        }
    }
}

#[derive(Debug)]
pub struct Mail {
    topic: TopicName,
    receiver: Receiver<Arc<PublishPacket>>,
    qos_level: QosLevel,
}

impl Mail {
    pub fn new(
        topic: TopicName,
        // TODO: Fthis currently loses packets if the client does not acknowledge the packet in time. This should not happen.
        receiver: Receiver<Arc<PublishPacket>>,
        qos_level: QosLevel,
    ) -> Self {
        return Self {
            topic,
            receiver,
            qos_level,
        };
    }

    pub fn recv(&mut self) -> Result<Option<Arc<PublishPacket>>, ServerError> {
        match self.receiver.try_recv() {
            Ok(packet) => return Ok(Some(packet)),
            Err(err) => match err {
                TryRecvError::Closed => Err(ServerError::new(
                    server::ErrorKind::BroadcastError,
                    String::from("Server Broadcast closed. You've encountered a bug. Please report this issue on GitHub."),
                )),
                TryRecvError::Empty => {
                    return Ok(None);
                }
                TryRecvError::Lagged(val) => {
                    return Err(ServerError::new(server::ErrorKind::FullMailbox(val),
                    format!("Server could not forward broadcast messages in time, dropping {val} messages.")))
                }
            },
        }
    }

    pub fn qos(&self) -> QosLevel {
        return self.qos_level;
    }

    pub fn topic(&self) -> &TopicName {
        return &self.topic;
    }
}

impl PartialEq for Mail {
    fn eq(&self, other: &Self) -> bool {
        self.topic == other.topic
    }
}
