use mqtt_core::{
    err::server::ServerError,
    qos::QosLevel,
    topic::{TopicName, TopicSubscription},
    v3::PublishPacket,
    Encode,
};
use std::{
    collections::{hash_map, HashMap},
    sync::Arc,
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::broadcast,
};

use crate::{
    mailbox::{Mail, Mailbox},
    session::ActiveSession,
};

#[derive(Debug)]
pub struct ServerTopics {
    topics: HashMap<TopicName, ServerTopic>,
    max_qos: QosLevel,
    max_queued_messages: usize,
}

impl ServerTopics {
    /// max_queued_messages: maximum number of messages to be stored in a queue for a given topic.
    /// If the broker-client connection does not reach the message before the queue size is reached,
    /// the message is lost.
    pub fn new(max_queued_messages: usize, max_qos: QosLevel) -> Self {
        return Self {
            topics: HashMap::new(),
            max_queued_messages,
            max_qos,
        };
    }

    /// Creates a new topic from a given TopicName. String representation of topics is folder-like, i.e. "some/descriptor/for/topic"
    pub fn create_topic(&mut self, topic_name: TopicName) {
        self.topics.insert(
            topic_name,
            ServerTopic::new(self.max_queued_messages, self.max_qos),
        );
    }

    /// Reserves a new retained message for the topic contained in the packet.
    ///
    /// Retained messages are reserved to one per topic. A new retained message entry will replace the old message. Retained messages are published
    /// to currently subscribed clients immediately[*1], and will be retained for future subscribers to receive.
    ///
    /// *1 "immediately" is a misnomer. It still is dependant on the server load. The message will be queued as any other message in the broker.
    pub fn retain_message(&mut self, packet: PublishPacket) {
        let topic_name = packet.topic().clone();
        match self.topic_mut(&topic_name) {
            Some(channel) => {
                channel.retain_message(packet);
            }
            None => {
                unreachable!("You've encountered a bug. Please submit a issue.");
                // let mut topic = ServerTopic::new(self.max_queued_messages, max_qos);
                // topic.retain_message(packet);
                // self.topics.insert(topic_name, topic);
            }
        }
    }

    /// obtain a mutable topic from the broker's stored topics.
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
    max_qos: QosLevel,
}

impl ServerTopic {
    pub fn new(size: usize, max_qos: QosLevel) -> Self {
        return Self {
            channel: broadcast::Sender::new(size),
            retained_message: None,
            max_qos,
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

    pub fn max_qos(&self) -> QosLevel {
        return self.max_qos;
    }
}

/// Subscribe to a topic.
///
/// This function forwards any retained messages that the server holds for that topic,
///
/// ## Returns
///
/// True if the client was successfully subscribed, false otherwise.
/// 'success' is defined as the there is an associated topic for the filter,
/// AND the client has the proper priviledges to access a topic inside the filter.
///
/// Because a client will be returned a success if even one topic matches a wildcard filter,
/// care should be taken to ensure that whitelists are not configured to
/// returned success on such patterns.
///
/// ## Error
///
/// It will error if the retained messages cannot be written to the stream,

// TODO: authorization for topics.
// TODO: binary search for left most valid, then recurse through remaining slice with another binary search.
pub async fn subscribe_to_topic_filter<S: AsyncWrite + Unpin>(
    stream: &mut S,
    topics: hash_map::Iter<'_, TopicName, ServerTopic>,
    session: &mut ActiveSession,
    mailbox: &mut Mailbox,
    topic_sub: &TopicSubscription,
) -> Result<bool, ServerError> {
    let mut sub_allowed = false;

    for (topic_name, topic) in topics {
        if topic_name == topic_sub.filter() {
            sub_allowed = true;
            // We can upgrade the Client's QoS for the retained messages on subscribe, see MQTT V3.1.1 documentation.
            //
            // The QoS of Payload Messages sent in response to a Subscription MUST be the minimum of the QoS of the originally
            // published message and the maximum QoS granted by the Server. The server is permitted to send duplicate copies of
            // a message to a subscriber in the case where the original message was published with QoS 1 and the maximum QoS
            // granted was QoS 0 [MQTT-3.8.4-6].
            if let Some(retained_message) = topic.get_retained_message() {
                // Performance overhead (clone into an Arc). The message assurance might need some refractoring...
                let packet = session.origin(&Arc::new(retained_message.clone()));
                stream.write_all(&packet.encode()?).await?;
            }

            let receiver = topic.subscribe();
            let mail = Mail::new(
                topic_name.clone(),
                receiver,
                topic_sub.qos().min(topic.max_qos()),
            );
            mailbox.add_slot(mail);
        }
    }
    return Ok(sub_allowed);
}
