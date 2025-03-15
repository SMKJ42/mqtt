// end state is PUBACK, but we do not need to maintain that state, only send the packet

use crate::{
    err::EncodeError,
    v3::{MqttPacket, PubRecPacket, PubRelPacket, PublishPacket},
};
use bytes::Bytes;

use core::time::Duration;
use std::slice::{Iter, IterMut};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ExactlyOnceList<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    inner: Vec<ExactlyOncePacket<P, I, B>>,
}

impl<I, B> ExactlyOnceList<Arc<PublishPacket>, I, B>
where
    I: Instant,
    B: ExponentialBackoff,
{
    /// Takes in an Arc<PublishPacket> and moves the packet onto the stack and applies a new packet Id.
    ///
    /// The returned packet is safe to drop.
    pub fn origin(&mut self, packet: Arc<PublishPacket>, new_id: u16) -> PublishPacket {
        let mut deref_packet = (*packet).clone();
        let packet = ExactlyOncePacket::origin(packet, new_id);
        match self.inner.binary_search(&packet) {
            Ok(_idx) => {
                // TODO: packet already exists.
                todo!("Packet exists");
            }
            Err(idx) => {
                self.inner.insert(idx, packet);
            }
        }
        unsafe { deref_packet.set_id(Some(new_id)) };
        return deref_packet;
    }

    /// Inserts a PUBLISH packet into the ExactlyOnceList if packet is not already contained in the list.
    ///
    /// ## Returns a PUBREC packet if the publish packet has not yet been received by the broker.
    pub fn publish(&mut self, packet: PublishPacket, new_id: u16) -> Option<PubRecPacket> {
        let packet = Arc::new(packet);
        let is_duplicate = packet.dup();
        let packet = ExactlyOncePacket::publish(packet, new_id);
        // if it is a duplicate, check if we already received the packet
        if is_duplicate {
            match self.inner.binary_search(&packet) {
                // if we already received the packet, do nothing.
                Ok(_idx) => return None,
                Err(idx) => {
                    self.inner.insert(idx, packet);
                }
            }
        } else {
            self.inner.push(packet);
        }

        return Some(PubRecPacket::new(new_id));
    }

    /// Updates the state of the ExactlyOncelist to reflect that the PUBREL packet has been received.
    pub fn release(&mut self, packet_id: u16) -> Option<Arc<PublishPacket>> {
        for packet in self.inner.iter_mut() {
            if packet.new_id == packet_id {
                return packet.relay();
            }
        }

        return None;
    }

    /// Updates the state of the ExactlyOnceList to reflect that the PUBCOMP packet has been received.
    pub fn complete(&mut self, packet_id: u16) -> Option<PublishPacket> {
        for packet in self.inner.iter_mut() {
            if packet.new_id == packet_id {
                return packet.complete();
            }
        }
        return None;
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct ExactlyOncePacket<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    packet: P,
    new_id: u16,
    stage: QoS2Stage,
    last_received: I,
    retry_duration: B,
}

impl<P, I, B> ExactlyOnceList<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    pub fn new() -> Self {
        return Self { inner: vec![] };
    }

    /// All Complete and Relay packets are removed, and their Ids are returned.
    pub fn clean(&mut self) -> Vec<u16> {
        let mut id_idxs = vec![];
        for (idx, packet) in self.inner.iter().enumerate() {
            match packet.stage {
                QoS2Stage::Comp | QoS2Stage::Rel => {
                    id_idxs.push((packet.new_id, idx));
                }
                _ => {}
            }
        }

        let (ids, idxs): (Vec<u16>, Vec<usize>) = id_idxs.into_iter().unzip();

        for idx in idxs.iter().rev() {
            self.inner.remove(*idx);
        }

        return ids;
    }

    /// Updates the state of the ExactlyOnceList to reflect that the client has sent a PUBREC packet.
    pub fn receive(&mut self, packet_id: u16) -> Option<PubRelPacket> {
        for packet in self.inner.iter_mut() {
            if packet.new_id == packet_id {
                return packet.receive();
            }
        }
        return None;
    }

    pub fn len(&self) -> usize {
        return self.inner.len();
    }

    pub fn iter(&self) -> Iter<'_, ExactlyOncePacket<P, I, B>> {
        return self.inner.iter();
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, ExactlyOncePacket<P, I, B>> {
        return self.inner.iter_mut();
    }
}

impl<I, B> ExactlyOncePacket<Arc<PublishPacket>, I, B>
where
    I: Instant,
    B: ExponentialBackoff,
{
    /// Updates the persisted packet to indicate to the device that the PUBCOMP packet has been received
    pub fn complete(&mut self) -> Option<PublishPacket> {
        if self.stage != QoS2Stage::Rec {
            return None;
        }

        self.last_received = I::now();
        self.stage = QoS2Stage::Comp;

        let mut packet = (*self.packet).clone();
        unsafe { packet.set_id(Some(self.new_id)) };

        return Some(packet);
    }

    pub fn id(&self) -> u16 {
        return self.new_id;
    }

    /// Obtains the correct packet for the current state of a packet transmission.
    pub fn get_retry_packet(&self) -> Option<MqttPacket> {
        let out: MqttPacket;
        match self.stage {
            QoS2Stage::Origin => {
                let mut packet = (*self.packet).clone();
                unsafe { packet.set_id(Some(self.new_id)) };
                packet.set_dup(true);
                out = MqttPacket::Publish(packet);
            }

            QoS2Stage::Rec => {
                let packet = PubRelPacket::new(self.new_id);
                out = MqttPacket::PubRel(packet);
            }
            _ => return None,
        };

        return Some(out);
    }

    /// Updates the persisted packet to indicate to the device that the PUBREL packet has been received
    pub fn relay(&mut self) -> Option<Arc<PublishPacket>> {
        if self.stage != QoS2Stage::Publish {
            return None;
        }
        self.last_received = I::now();
        self.stage = QoS2Stage::Rel;
        return Some(self.inner().clone());
    }

    /// Returns the PUBLISH packet associate with the exactly once message.
    pub fn inner(&self) -> &Arc<PublishPacket> {
        return &self.packet;
    }
}

impl<P, I, B> ExactlyOncePacket<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    /// Used when publishing a QoS2 packet to persist the packet on the device.
    pub fn origin(packet: P, new_id: u16) -> Self {
        return Self {
            packet,
            new_id,
            stage: QoS2Stage::Origin,
            last_received: I::now(),
            retry_duration: B::default(),
        };
    }

    /// Used when a device has received a QoS2 PUBLISH packet to persist the packet on the device.
    pub fn publish(packet: P, new_id: u16) -> Self {
        return Self {
            packet,
            new_id,
            stage: QoS2Stage::Publish,
            last_received: I::now(),
            retry_duration: B::default(),
        };
    }

    /// Updates the persisted packet to indicate to the device that the PUBREC packet has been received
    ///
    /// Returns the response packet
    pub fn receive(&mut self) -> Option<PubRelPacket> {
        if self.stage != QoS2Stage::Origin {
            return None;
        }
        self.last_received = I::now();
        self.stage = QoS2Stage::Rec;
        return Some(PubRelPacket::new(self.new_id));
    }

    /// Updates the retry duration with an exponential backoff.
    pub fn update_retry_duration(&mut self) {
        let duration = self.retry_duration.exponential();
        self.retry_duration.set_duration(duration);
    }

    // Returns true if the duration since the packet was last advanced is greater thn the retry duration.
    pub fn should_retry(&self) -> bool {
        match self.stage() {
            QoS2Stage::Origin => self.is_timed_out(),
            _ => false,
        }
    }

    pub fn is_timed_out(&self) -> bool {
        I::now().duration_since(&self.last_received) > self.retry_duration.inner()
    }

    pub fn stage(&self) -> QoS2Stage {
        return self.stage;
    }
}

/// The value of the packet representing the last KNOWN state of the packet.
///
/// The last known state is the state the packet was received in.
///
/// For publishers the known state is Origin while subscribers the last known state is Publish.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum QoS2Stage {
    Origin,
    Publish,
    Rec,
    Rel,
    Comp,
}

/// Note that the functionality of the AtLeastOnceList is slightly different from the client crate.
///
/// The client crate handles the ACTION of sending packets,
/// while this struct handles the STATE of the last packet received in a chain of packets.
///
/// When choosing a function from this struct to use, choose the function with the name of the last known STATE
/// of the packet. For instance if you are sending a packet, the last known state is origin, while when receiving a publish packet
/// the last known state is publish.
#[derive(Clone, Debug)]
pub struct AtLeastOnceList<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    inner: Vec<AtLeastOncePacket<P, I, B>>,
}

impl<P, I, B> AtLeastOnceList<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    pub fn new() -> Self {
        Self { inner: vec![] }
    }

    pub fn iter(&self) -> Iter<'_, AtLeastOncePacket<P, I, B>> {
        return self.inner.iter();
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, AtLeastOncePacket<P, I, B>> {
        return self.inner.iter_mut();
    }

    pub fn len(&self) -> usize {
        return self.inner.len();
    }

    pub fn clean(&mut self) -> Vec<u16> {
        let mut id_idxs = vec![];
        for (idx, packet) in self.inner.iter().enumerate() {
            match packet.stage {
                QoS1Stage::Ack => {
                    id_idxs.push((packet.new_id, idx));
                }
                _ => {}
            }
        }

        let (ids, idxs): (Vec<u16>, Vec<usize>) = id_idxs.into_iter().unzip();

        for idx in idxs.iter().rev() {
            self.inner.remove(*idx);
        }

        return ids;
    }
}

impl<I, B> AtLeastOnceList<Arc<PublishPacket>, I, B>
where
    I: Instant,
    B: ExponentialBackoff,
{
    pub fn origin(&mut self, packet: Arc<PublishPacket>, new_id: u16) -> PublishPacket {
        let mut deref_packet = (*packet).clone();

        self.inner.push(AtLeastOncePacket::origin(packet, new_id));

        unsafe { deref_packet.set_id(Some(new_id)) };
        return deref_packet;
    }

    pub fn publish(&mut self, packet: Arc<PublishPacket>) {
        let deref_packet = (*packet).clone();
        let id = deref_packet.id().unwrap();

        self.inner.push(AtLeastOncePacket::publish(packet, id));
    }

    pub fn acknowledge(&mut self, packet_id: u16) {
        for packet in self.inner.iter_mut() {
            if packet.new_id == packet_id {
                packet.acknowledge();
            }
        }
    }
}

/// A struct representing a QoS1 packet.
///
/// Because it is At least once, the receiver of the packet does not need to store a PUBACK, and only the
/// originator of the packet needs to store the packet state.
#[derive(Clone, Debug)]
pub struct AtLeastOncePacket<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    packet: P,
    new_id: u16,
    stage: QoS1Stage,
    last_received: I,
    retry_duration: B,
}

impl<P, I, B> AtLeastOncePacket<P, I, B>
where
    P: PubPack,
    I: Instant,
    B: ExponentialBackoff,
{
    /// Used when publishing a QoS1 packet to persist the packet on the device.
    pub fn origin(packet: P, new_id: u16) -> Self {
        return Self {
            packet,
            new_id,
            stage: QoS1Stage::Origin,
            last_received: I::now(),
            retry_duration: B::default(),
        };
    }

    pub fn publish(packet: P, new_id: u16) -> Self {
        return Self {
            packet,
            new_id,
            stage: QoS1Stage::Publish,
            last_received: I::now(),
            retry_duration: B::default(),
        };
    }

    /// Updates the persisted packet to indicate to the device that the PUBACK packet has been received
    pub fn acknowledge(&mut self) {
        self.last_received = I::now();
        self.stage = QoS1Stage::Ack;
    }

    /// Updates the retry duration with an exponential backoff.
    pub fn update_retry_duration(&mut self) {
        let duration = self.retry_duration.exponential();
        self.retry_duration.set_duration(duration);
    }

    // Returns true if the duration since the packet was last advanced is greater thn the retry duration.
    pub fn should_retry(&self) -> bool {
        match self.stage {
            QoS1Stage::Origin => self.is_timed_out(),
            _ => false,
        }
    }

    pub fn id(&self) -> u16 {
        return self.new_id;
    }

    pub fn is_timed_out(&self) -> bool {
        I::now().duration_since(&self.last_received) > self.retry_duration.inner()
    }

    pub fn stage(&self) -> QoS1Stage {
        return self.stage;
    }
}

impl<I, B> AtLeastOncePacket<Arc<PublishPacket>, I, B>
where
    I: Instant,
    B: ExponentialBackoff,
{
    pub fn get_retry_packet(&self) -> Option<MqttPacket> {
        let out: MqttPacket;
        match self.stage {
            QoS1Stage::Origin => {
                let mut packet = (*self.packet).clone();
                packet.set_dup(true);
                out = MqttPacket::Publish(packet);
            }
            _ => return None,
        };

        return Some(out);
    }

    pub fn inner(&self) -> &Arc<PublishPacket> {
        return &self.packet;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QoS1Stage {
    Origin,
    Publish,
    Ack,
}

pub trait PubPack {
    fn as_bytes(&self) -> Result<Bytes, EncodeError>;
}

impl PubPack for Arc<PublishPacket> {
    fn as_bytes(&self) -> Result<Bytes, EncodeError> {
        self.encode()
    }
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
