use std::{error::Error, fmt::Display};

impl Error for PacketError {}

#[derive(Clone, Debug)]
pub struct PacketError {
    kind: PacketErrorKind,
    message: String,
}

impl PacketError {
    pub fn new(kind: PacketErrorKind, message: String) -> Self {
        return Self { kind, message };
    }

    pub fn kind(&self) -> PacketErrorKind {
        return self.kind;
    }

    pub fn message(&self) -> &String {
        return &self.message;
    }
}

impl Display for PacketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind: {:?}, message: {}", self.kind, self.message)
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum PacketErrorKind {
    ProtocolName,
    ProtocolLevel,
    FlagBits,
    PacketType,
    WillQoS,
    Will,
    QoS,
    AccessToReservedBit,
    Utf8ParseError,
    MalformedLength,
    MalformedTopicFilter,
    MalformedTopicName,
    UsernamePassword,
    InvalidReturnCode,
}
