use std::{
    collections::{hash_map, HashMap},
    sync::Arc,
    vec,
};

use mqtt_core::{qos::QosLevel, topics::TopicName};
use mqtt_packets::v3::publish::PublishPacket;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::broadcast};

use crate::error::client::ClientError;

pub struct ServerTopics {
    topics: HashMap<TopicName, ServerTopic>,
}

impl ServerTopics {
    pub fn new() -> Self {
        return Self {
            topics: HashMap::new(),
        };
    }

    pub fn create_topic(&mut self, topic_name: TopicName) {
        let topic = self.topics.insert(topic_name, ServerTopic::new());
    }

    pub fn get_channel(
        &self,
        topic_name: &TopicName,
    ) -> Option<broadcast::Sender<Arc<PublishPacket>>> {
        match self.topics.get(topic_name) {
            Some(topic) => Some(topic.stream().clone()),
            None => None,
        }
    }

    pub fn get_retained(&self) {}

    pub fn iter(&self) -> hash_map::Iter<'_, TopicName, ServerTopic> {
        return self.topics.iter();
    }
}

pub struct ServerTopic {
    stream: broadcast::Sender<Arc<PublishPacket>>,
    retained_messages: RetainedMessages,
}

impl ServerTopic {
    pub fn new() -> Self {
        return Self {
            stream: broadcast::Sender::new(32),
            retained_messages: RetainedMessages::new(),
        };
    }
    pub fn retained_messages(&self) -> &RetainedMessages {
        return &self.retained_messages;
    }
    pub fn stream(&self) -> &broadcast::Sender<Arc<PublishPacket>> {
        return &self.stream;
    }

    pub fn subscribe(
        &self,
    ) -> (
        tokio::sync::broadcast::Receiver<Arc<PublishPacket>>,
        &RetainedMessages,
    ) {
        return (self.stream.subscribe(), &self.retained_messages);
    }
}

#[derive(Clone)]
pub struct RetainedMessages {
    qos_0_message: Option<Arc<PublishPacket>>,
    qos_gt0_messages: Vec<Arc<PublishPacket>>,
}

impl RetainedMessages {
    pub fn new() -> Self {
        return Self {
            qos_0_message: None,
            qos_gt0_messages: vec![],
        };
    }

    pub fn retain_message(&mut self, packet: Arc<PublishPacket>) {
        match packet.qos() {
            QosLevel::AtMostOnce => {
                self.qos_0_message = Some(packet);
            }
            _ => {
                self.qos_gt0_messages.push(packet);
            }
        }
    }

    pub async fn publish(&self, stream: &mut TcpStream) -> Result<(), ClientError> {
        if let Some(packet) = &self.qos_0_message {
            stream.write_all_buf(&mut packet.encode()?).await?;
        };

        for packet in &self.qos_gt0_messages {
            stream.write_all_buf(&mut packet.encode()?).await?;
        }

        return Ok(());
    }
}

// impl Iter<'a, Arc<PublishPacket>> for RetainedMessages {}
impl Iterator for RetainedMessages {
    type Item = Arc<PublishPacket>;
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
