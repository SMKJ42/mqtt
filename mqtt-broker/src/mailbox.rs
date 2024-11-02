use std::sync::Arc;

use mqtt_core::topics::{TopicFilter, TopicName};
use mqtt_packets::v3::publish::PublishPacket;
use tokio::sync::broadcast::{error::TryRecvError, Receiver};

use crate::error::server::{self, ServerError};

pub struct Mailbox(Vec<Mail>);

impl Mailbox {
    pub fn new() -> Self {
        return Self(vec![]);
    }

    pub fn combine(&mut self, mailbox: &mut Mailbox) {
        self.0.append(mailbox.mail_mut());
    }

    pub fn queue(&mut self, mail: Mail) {
        self.0.push(mail);
    }

    pub fn mail_mut(&mut self) -> &mut Vec<Mail> {
        return &mut self.0;
    }

    pub fn remove(&mut self, filter: &TopicFilter) {
        while let Some(idx) = self
            .mail_mut()
            .iter()
            .position(|mail| &mail.topic == filter)
        {
            self.mail_mut().remove(idx);
        }
    }
}

pub struct Mail {
    topic: TopicName,
    receiver: Receiver<Arc<PublishPacket>>,
}

impl Mail {
    pub fn new(topic: TopicName, receiver: Receiver<Arc<PublishPacket>>) -> Self {
        return Self { topic, receiver };
    }

    pub fn recv(&mut self) -> Result<Option<Arc<PublishPacket>>, ServerError> {
        match self.receiver.try_recv() {
            Ok(packet) => return Ok(Some(packet)),
            Err(err) => match err {
                TryRecvError::Closed => Err(ServerError::new(
                    server::ErrorKind::BroadcastError,
                    String::from("Server Broadcast closed. Oops, youve encountered a bug. Please report this issue."),
                )),
                TryRecvError::Empty => {
                    return Ok(None);
                }
                TryRecvError::Lagged(val) => {
                    return Err(ServerError::new(server::ErrorKind::FullMailbox,
                    format!("Server could not forward broadcast messages in time, dropping {val} messages.")))
                }
            },
        }
    }

    pub fn topic(&self) -> &TopicName {
        return &self.topic;
    }
}
