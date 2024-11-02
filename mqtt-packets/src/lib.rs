use bytes::Bytes;
use mqtt_core::err::PacketError;
use v3::{FixedHeader, MqttPacket};

pub mod v3;

pub fn decode_packet(f_header: FixedHeader, buf: &mut Bytes) -> Result<MqttPacket, PacketError> {
    MqttPacket::decode(f_header, &mut Bytes::copy_from_slice(buf))
}
