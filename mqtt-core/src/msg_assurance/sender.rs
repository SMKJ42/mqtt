use crate::{
    v4::{MqttPacket, PubRecPacket, PubRelPacket, PublishPacket},
    Encode,
};

use std::slice::{Iter, IterMut};
use std::sync::Arc;

#[derive(Clone, Debug)]
/// # Use
/// Method names denote the state the packet is received in,
/// starting with 'origin' for a new packet to be sent.
pub struct ExactlyOnceList<P, I, B>
where
    P: Encode,
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

//// Tracks sent and received packets that have not reached the end of their lifetime.
impl<P, I, B> ExactlyOnceList<P, I, B>
where
    P: Encode,
    I: Instant,
    B: ExponentialBackoff,
{
    pub fn new() -> Self {
        return Self { inner: vec![] };
    }

    /// Removes the packets that have been released or completed inside the queue. Returns the id's of the packets.
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

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct ExactlyOncePacket<P, I, B>
where
    P: Encode,
    I: Instant,
    B: ExponentialBackoff,
{
    packet: P,
    new_id: u16,
    stage: QoS2Stage,
    last_received: I,
    retry_duration: B,
}

/// ## Important
///
/// When utilizing TCP, or another protocol that handles retries automatically, this struct's retry mechanisms do not need to be utilized
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

    /// Updates the persisted packet to indicate to the device that the PUBREL packet has been received
    pub fn relay(&mut self) -> Option<Arc<PublishPacket>> {
        if self.stage != QoS2Stage::Publish {
            return None;
        }
        self.last_received = I::now();
        self.stage = QoS2Stage::Rel;
        return Some(self.inner().clone());
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

    /// Returns the PUBLISH packet associate with the exactly once message.
    pub fn inner(&self) -> &Arc<PublishPacket> {
        return &self.packet;
    }
}

impl<P, I, B> ExactlyOncePacket<P, I, B>
where
    P: Encode,
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

use super::{ExponentialBackoff, Instant, QoS1Stage};
// while this struct handles the STATE of the last packet received in a chain of packets.
///
/// When choosing a function from this struct to use, choose the function with the name of the last known STATE
/// of the packet. For instance if you are sending a packet, the last known state is origin, while when receiving a publish packet
/// the last known state is publish.
#[derive(Clone, Debug)]
pub struct AtLeastOnceList<P, I, B>
where
    P: Encode,
    I: Instant,
    B: ExponentialBackoff,
{
    inner: Vec<AtLeastOncePacket<P, I, B>>,
}

impl<P, I, B> AtLeastOnceList<P, I, B>
where
    P: Encode,
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

    /// Removes the packets that have been acknowledged inside the queue. Returns the id's of the packets.
    pub fn clean(&mut self) -> Vec<u16> {
        let mut id_idxs = vec![];
        for (idx, packet) in self.iter().enumerate() {
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

/// A struct representing a QoS1 packet.
///
//// Tracks sent packets that have not reached the end of their lifetime.
///
/// This struct is only required to be implemented for senders
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
    P: Encode,
    I: Instant,
    B: ExponentialBackoff,
{
    packet: P,
    new_id: u16,
    stage: QoS1Stage,
    last_received: I,
    retry_duration: B,
}

///
/// ## Note
/// The functionality of the AtLeastOnceList is slightly different from the client crate.
///
/// The client crate handles the ACTION of sending packets,
/// end state is PUBACK, but we do not need to maintain that state, only send the packet
impl<P, I, B> AtLeastOncePacket<P, I, B>
where
    P: Encode,
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
