use mqtt_core::err::PacketError;

pub enum MqttClientError {
    PacketError(PacketError),
    ProtocolError,
}

impl From<PacketError> for MqttClientError {
    fn from(value: PacketError) -> Self {
        match value.kind() {
            _ => return Self::PacketError(value),
        }
    }
}
