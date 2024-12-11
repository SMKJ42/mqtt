use std::{error::Error, fmt::Display};

#[derive(Debug, Clone)]
pub struct EncodeError {
    kind: EncodeErrorKind,
    message: String,
}

impl EncodeError {
    pub fn new(kind: EncodeErrorKind, message: String) -> Self {
        return Self { kind, message };
    }

    pub fn kind(&self) -> EncodeErrorKind {
        return self.kind;
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum EncodeErrorKind {
    OversizedPayload,
}

impl Error for DecodeError {}

#[derive(Debug, Clone)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    message: String,
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl DecodeError {
    pub fn new(kind: DecodeErrorKind, message: String) -> Self {
        return Self { kind, message };
    }

    pub fn kind(&self) -> DecodeErrorKind {
        return self.kind;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecodeErrorKind {
    FlagBits,
    PacketType,
    WillQoS,
    Will,
    QoS,
    Utf8ParseError,
    MalformedLength,
    MalformedTopicFilter,
    MalformedTopicName,
    UsernamePassword,
    InvalidProtocol,
    InvalidReturnCode,
    ImproperDisconnect,
    ProtocolError,
    Timeout,
}

pub mod client {
    use std::fmt::Display;

    use tokio::io;

    use super::{DecodeError, EncodeError};

    #[derive(Debug)]
    pub enum ErrorKind {
        IoError(io::Error),
        ProtocolError,
        TopicDoesNotExist(String),
        DecodeError,
        EncodeError,
    }

    impl Display for ErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(f, "{:?}", self);
        }
    }

    #[derive(Debug)]
    pub struct ClientError {
        kind: ErrorKind,
        message: String,
    }

    impl ClientError {
        pub fn new(kind: ErrorKind, message: String) -> Self {
            return Self { kind, message };
        }

        pub fn kind(&self) -> &ErrorKind {
            return &self.kind;
        }
    }

    impl From<DecodeError> for ClientError {
        fn from(value: DecodeError) -> Self {
            return Self {
                kind: ErrorKind::DecodeError,
                message: value.message,
            };
        }
    }

    impl From<EncodeError> for ClientError {
        fn from(value: EncodeError) -> Self {
            return Self {
                kind: ErrorKind::EncodeError,
                message: value.message,
            };
        }
    }

    impl From<std::io::Error> for ClientError {
        fn from(value: io::Error) -> Self {
            return Self {
                kind: ErrorKind::IoError(value),
                message: String::new(),
            };
        }
    }

    impl Display for ClientError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(f, "{}. {}", self.kind, self.message);
        }
    }
}

pub mod server {
    use std::fmt::Display;

    use tokio::io;

    use super::{DecodeError, EncodeError};

    #[derive(Debug)]
    pub struct ServerError {
        kind: ErrorKind,
        message: String,
    }

    #[derive(Debug)]
    pub enum ErrorKind {
        DecodeError,
        EncodeError,
        IoError(io::Error),
        ProtocolError,
        BroadcastError,
        FullMailbox(u64),
        ReusedClientId,
    }

    impl Display for ErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(f, "{:?}", self);
        }
    }

    impl From<DecodeError> for ServerError {
        fn from(value: DecodeError) -> Self {
            return Self {
                kind: ErrorKind::DecodeError,
                message: value.message,
            };
        }
    }

    impl From<EncodeError> for ServerError {
        fn from(value: EncodeError) -> Self {
            return Self {
                kind: ErrorKind::EncodeError,
                message: value.message,
            };
        }
    }

    impl From<std::io::Error> for ServerError {
        fn from(value: io::Error) -> Self {
            return Self {
                kind: ErrorKind::IoError(value),
                message: String::new(),
            };
        }
    }

    impl ServerError {
        pub fn new(kind: ErrorKind, message: String) -> Self {
            Self { kind, message }
        }

        pub fn kind(&self) -> &ErrorKind {
            return &self.kind;
        }

        pub fn message(&self) -> &str {
            return &self.message;
        }
    }

    impl Display for ServerError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(f, "{}. {}", self.kind, self.message);
        }
    }
}
