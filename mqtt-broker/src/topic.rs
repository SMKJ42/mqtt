use mqtt_core::{topic::TopicName, v3::PublishPacket};
use std::{
    collections::{hash_map, HashMap},
    sync::Arc,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct ServerTopics {
    topics: HashMap<TopicName, ServerTopic>,
    max_queued_messages: usize,
}

impl ServerTopics {
    pub fn new(max_queued_messages: usize) -> Self {
        return Self {
            topics: HashMap::new(),
            max_queued_messages,
        };
    }

    pub fn create_topic(&mut self, topic_name: TopicName) {
        self.topics
            .insert(topic_name, ServerTopic::new(self.max_queued_messages));
    }

    pub fn retain_message(&mut self, packet: PublishPacket) {
        let topic_name = packet.topic().clone();
        match self.topic_mut(&topic_name) {
            Some(channel) => {
                channel.retain_message(packet);
            }
            None => {
                let mut topic = ServerTopic::new(self.max_queued_messages);
                topic.retain_message(packet);
                self.topics.insert(topic_name, topic);
            }
        }
    }

    pub fn topic_mut(&mut self, topic_name: &TopicName) -> Option<&mut ServerTopic> {
        return self.topics.get_mut(topic_name);
    }

    pub fn iter(&self) -> hash_map::Iter<'_, TopicName, ServerTopic> {
        return self.topics.iter();
    }
}

#[derive(Debug, Clone)]
pub struct ServerTopic {
    channel: broadcast::Sender<Arc<PublishPacket>>,
    retained_message: Option<PublishPacket>,
}

impl ServerTopic {
    pub fn new(size: usize) -> Self {
        return Self {
            channel: broadcast::Sender::new(size),
            retained_message: None,
        };
    }

    pub fn get_retained_message(&self) -> Option<&PublishPacket> {
        return self.retained_message.as_ref();
    }

    pub fn retain_message(&mut self, message: PublishPacket) {
        if message.payload().len() == 0 {
            self.retained_message = None;
        } else {
            self.retained_message = Some(message);
        }
    }

    pub fn channel(&self) -> &broadcast::Sender<Arc<PublishPacket>> {
        return &self.channel;
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Arc<PublishPacket>> {
        return self.channel.subscribe();
    }
}
