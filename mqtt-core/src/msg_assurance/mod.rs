use std::time::Duration;

pub mod receiver;
pub mod sender;

// /// ## Important
// ///
// /// When utilizing TCP, or another protocol that handles retries automatically, this struct's retry mechanisms do not need to be utilized
// impl<I, B> ExactlyOncePacket<Arc<PublishPacket>, I, B>
// where
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     /// Updates the persisted packet to indicate to the device that the PUBCOMP packet has been received
//     pub fn complete(&mut self) -> Option<PublishPacket> {
//         if self.stage != QoS2Stage::Rec {
//             return None;
//         }

//         self.last_received = I::now();
//         self.stage = QoS2Stage::Comp;

//         let mut packet = (*self.packet).clone();
//         unsafe { packet.set_id(Some(self.new_id)) };

//         return Some(packet);
//     }

//     pub fn id(&self) -> u16 {
//         return self.new_id;
//     }

//     /// Obtains the correct packet for the current state of a packet transmission.
//     pub fn get_retry_packet(&self) -> Option<MqttPacket> {
//         let out: MqttPacket;
//         match self.stage {
//             QoS2Stage::Origin => {
//                 let mut packet = (*self.packet).clone();
//                 unsafe { packet.set_id(Some(self.new_id)) };
//                 packet.set_dup(true);
//                 out = MqttPacket::Publish(packet);
//             }

//             QoS2Stage::Rec => {
//                 let packet = PubRelPacket::new(self.new_id);
//                 out = MqttPacket::PubRel(packet);
//             }
//             _ => return None,
//         };

//         return Some(out);
//     }

//     /// Updates the persisted packet to indicate to the device that the PUBREL packet has been received
//     pub fn relay(&mut self) -> Option<Arc<PublishPacket>> {
//         if self.stage != QoS2Stage::Publish {
//             return None;
//         }
//         self.last_received = I::now();
//         self.stage = QoS2Stage::Rel;
//         return Some(self.inner().clone());
//     }

//     /// Returns the PUBLISH packet associate with the exactly once message.
//     pub fn inner(&self) -> &Arc<PublishPacket> {
//         return &self.packet;
//     }
// }

// impl<P, I, B> ExactlyOncePacket<P, I, B>
// where
//     P: Encode,
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     /// Used when publishing a QoS2 packet to persist the packet on the device.
//     pub fn origin(packet: P, new_id: u16) -> Self {
//         return Self {
//             packet,
//             new_id,
//             stage: QoS2Stage::Origin,
//             last_received: I::now(),
//             retry_duration: B::default(),
//         };
//     }

//     /// Used when a device has received a QoS2 PUBLISH packet to persist the packet on the device.
//     pub fn publish(packet: P, new_id: u16) -> Self {
//         return Self {
//             packet,
//             new_id,
//             stage: QoS2Stage::Publish,
//             last_received: I::now(),
//             retry_duration: B::default(),
//         };
//     }

//     /// Updates the persisted packet to indicate to the device that the PUBREC packet has been received
//     ///
//     /// Returns the response packet
//     pub fn receive(&mut self) -> Option<PubRelPacket> {
//         if self.stage != QoS2Stage::Origin {
//             return None;
//         }
//         self.last_received = I::now();
//         self.stage = QoS2Stage::Rec;
//         return Some(PubRelPacket::new(self.new_id));
//     }

//     /// Updates the retry duration with an exponential backoff.
//     pub fn update_retry_duration(&mut self) {
//         let duration = self.retry_duration.exponential();
//         self.retry_duration.set_duration(duration);
//     }

//     // Returns true if the duration since the packet was last advanced is greater thn the retry duration.
//     pub fn should_retry(&self) -> bool {
//         match self.stage() {
//             QoS2Stage::Origin => self.is_timed_out(),
//             _ => false,
//         }
//     }

//     pub fn is_timed_out(&self) -> bool {
//         I::now().duration_since(&self.last_received) > self.retry_duration.inner()
//     }

//     pub fn stage(&self) -> QoS2Stage {
//         return self.stage;
//     }
// }

// /// The value of the packet representing the last KNOWN state of the packet.
// ///
// /// The last known state is the state the packet was received in.
// ///
// /// For publishers the known state is Origin while subscribers the last known state is Publish.
// #[derive(Clone, Debug, Copy, PartialEq, Eq, Ord, PartialOrd)]
// pub enum QoS2Stage {
//     Origin,
//     Publish,
//     Rec,
//     Rel,
//     Comp,
// }

// ///
// /// ## Note
// /// The functionality of the AtLeastOnceList is slightly different from the client crate.
// ///
// /// The client crate handles the ACTION of sending packets,
// /// while this struct handles the STATE of the last packet received in a chain of packets.
// ///
// /// When choosing a function from this struct to use, choose the function with the name of the last known STATE
// /// of the packet. For instance if you are sending a packet, the last known state is origin, while when receiving a publish packet
// /// the last known state is publish.
// #[derive(Clone, Debug)]
// pub struct AtLeastOnceList<P, I, B>
// where
//     P: Encode,
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     inner: Vec<AtLeastOncePacket<P, I, B>>,
// }

// impl<P, I, B> AtLeastOnceList<P, I, B>
// where
//     P: Encode,
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     pub fn new() -> Self {
//         Self { inner: vec![] }
//     }

//     pub fn iter(&self) -> Iter<'_, AtLeastOncePacket<P, I, B>> {
//         return self.inner.iter();
//     }

//     pub fn iter_mut(&mut self) -> IterMut<'_, AtLeastOncePacket<P, I, B>> {
//         return self.inner.iter_mut();
//     }

//     pub fn len(&self) -> usize {
//         return self.inner.len();
//     }

//     /// Removes the packets that have been acknowledged inside the queue. Returns the id's of the packets.
//     pub fn clean(&mut self) -> Vec<u16> {
//         let mut id_idxs = vec![];
//         for (idx, packet) in self.iter().enumerate() {
//             match packet.stage {
//                 QoS1Stage::Ack => {
//                     id_idxs.push((packet.new_id, idx));
//                 }
//                 _ => {}
//             }
//         }

//         let (ids, idxs): (Vec<u16>, Vec<usize>) = id_idxs.into_iter().unzip();

//         for idx in idxs.iter().rev() {
//             self.inner.remove(*idx);
//         }

//         return ids;
//     }
// }

// /// A struct representing a QoS1 packet.
// ///
// //// Tracks sent packets that have not reached the end of their lifetime.
// ///
// /// This struct is only required to be implemented for senders
// impl<I, B> AtLeastOnceList<Arc<PublishPacket>, I, B>
// where
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     pub fn origin(&mut self, packet: Arc<PublishPacket>, new_id: u16) -> PublishPacket {
//         let mut deref_packet = (*packet).clone();

//         self.inner.push(AtLeastOncePacket::origin(packet, new_id));

//         unsafe { deref_packet.set_id(Some(new_id)) };
//         return deref_packet;
//     }

//     pub fn publish(&mut self, packet: Arc<PublishPacket>) {
//         let deref_packet = (*packet).clone();
//         let id = deref_packet.id().unwrap();

//         self.inner.push(AtLeastOncePacket::publish(packet, id));
//     }

//     pub fn acknowledge(&mut self, packet_id: u16) {
//         for packet in self.inner.iter_mut() {
//             if packet.new_id == packet_id {
//                 packet.acknowledge();
//             }
//         }
//     }
// }

// /// A struct representing a QoS1 packet.
// ///
// /// Because it is At least once, the receiver of the packet does not need to store a PUBACK, and only the
// /// originator of the packet needs to store the packet state.
// #[derive(Clone, Debug)]
// pub struct AtLeastOncePacket<P, I, B>
// where
//     P: Encode,
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     packet: P,
//     new_id: u16,
//     stage: QoS1Stage,
//     last_received: I,
//     retry_duration: B,
// }

// impl<P, I, B> AtLeastOncePacket<P, I, B>
// where
//     P: Encode,
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     /// Used when publishing a QoS1 packet to persist the packet on the device.
//     pub fn origin(packet: P, new_id: u16) -> Self {
//         return Self {
//             packet,
//             new_id,
//             stage: QoS1Stage::Origin,
//             last_received: I::now(),
//             retry_duration: B::default(),
//         };
//     }

//     pub fn publish(packet: P, new_id: u16) -> Self {
//         return Self {
//             packet,
//             new_id,
//             stage: QoS1Stage::Publish,
//             last_received: I::now(),
//             retry_duration: B::default(),
//         };
//     }

//     /// Updates the persisted packet to indicate to the device that the PUBACK packet has been received
//     pub fn acknowledge(&mut self) {
//         self.last_received = I::now();
//         self.stage = QoS1Stage::Ack;
//     }

//     /// Updates the retry duration with an exponential backoff.
//     pub fn update_retry_duration(&mut self) {
//         let duration = self.retry_duration.exponential();
//         self.retry_duration.set_duration(duration);
//     }

//     // Returns true if the duration since the packet was last advanced is greater thn the retry duration.
//     pub fn should_retry(&self) -> bool {
//         match self.stage {
//             QoS1Stage::Origin => self.is_timed_out(),
//             _ => false,
//         }
//     }

//     pub fn id(&self) -> u16 {
//         return self.new_id;
//     }

//     pub fn is_timed_out(&self) -> bool {
//         I::now().duration_since(&self.last_received) > self.retry_duration.inner()
//     }

//     pub fn stage(&self) -> QoS1Stage {
//         return self.stage;
//     }
// }

// impl<I, B> AtLeastOncePacket<Arc<PublishPacket>, I, B>
// where
//     I: Instant,
//     B: ExponentialBackoff,
// {
//     pub fn get_retry_packet(&self) -> Option<MqttPacket> {
//         let out: MqttPacket;
//         match self.stage {
//             QoS1Stage::Origin => {
//                 let mut packet = (*self.packet).clone();
//                 packet.set_dup(true);
//                 out = MqttPacket::Publish(packet);
//             }
//             _ => return None,
//         };

//         return Some(out);
//     }

//     pub fn inner(&self) -> &Arc<PublishPacket> {
//         return &self.packet;
//     }
// }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QoS1Stage {
    Origin,
    Publish,
    Ack,
}

impl Instant for std::time::Instant {
    fn duration_since(&self, instant: &std::time::Instant) -> std::time::Duration {
        return self.duration_since(*instant);
    }
    fn now() -> Self {
        return std::time::Instant::now();
    }
}

#[derive(Debug, Clone, Ord, PartialEq, Eq, PartialOrd)]
pub struct RetryDuration {
    dur: core::time::Duration,
}

impl Default for RetryDuration {
    fn default() -> Self {
        return Self {
            dur: Duration::from_millis(200),
        };
    }
}

impl ExponentialBackoff for RetryDuration {
    fn exponential(&self) -> core::time::Duration {
        return *self.dur.checked_mul(2).get_or_insert(Duration::MAX);
    }

    fn inner(&self) -> core::time::Duration {
        return self.dur;
    }

    fn set_duration(&mut self, dur: Duration) {
        self.dur = dur;
    }
}

pub trait Instant: Ord {
    fn now() -> Self;
    fn duration_since(&self, instant: &Self) -> core::time::Duration;
}

/// This is useful for sessions that utilize a stream where packet delivery is not garenteed.
///
/// For example, TCP guarantees message delivery while UDP does not. UDP is not an inherently supported protocol,
/// however, I do plan on supporting more messaging protocols.
///
/// ## Examples
///
/// ```
/// use mqtt_core::msg_assurance::ExponentialBackoff;
/// use core::time::Duration;
///
/// #[derive(PartialEq, Eq, PartialOrd, Ord)]
/// pub struct MyDuration {
///     dur: Duration,
/// }
///
/// impl Default for MyDuration {
///     fn default() -> Self {
///         return Self {
///             dur: Duration::from_millis(100),
///         };
///     }
/// }
///
/// impl ExponentialBackoff for MyDuration {
///     fn exponential(&self) -> core::time::Duration {
///         return *self.dur.checked_mul(2).get_or_insert(Duration::MAX);
///     }
///
///     fn inner(&self) -> core::time::Duration {
///         return self.dur;
///         }
///
///     fn set_duration(&mut self, dur: Duration) {
///         self.dur = dur;
///     }
/// }
/// ```
pub trait ExponentialBackoff: Ord + Default {
    /// Returns a new duration with the exponential backoff function applied.
    /// Returns None if an overflow occured.
    fn exponential(&self) -> core::time::Duration;

    /// Function that receives the inner Duration value.
    fn inner(&self) -> core::time::Duration;

    fn set_duration(&mut self, dur: Duration);
}
