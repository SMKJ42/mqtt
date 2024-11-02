pub mod client {
    use std::fmt::Error;

    use mqtt_core::err::{PacketError, PacketErrorKind};
    use tokio::io;

    #[derive(Debug)]
    pub enum ErrorKind {
        IoError(io::Error),
        PacketError(PacketErrorKind),
        ProtocolError,
        ImproperDisconnect,
        TopicDoesNotExist(String),
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

        pub fn message(&self) -> &str {
            return &self.message;
        }
    }

    impl From<std::io::Error> for ClientError {
        fn from(value: io::Error) -> Self {
            return Self {
                kind: ErrorKind::IoError(value),
                message: String::from("IO error."),
            };
        }
    }

    impl From<PacketError> for ClientError {
        fn from(value: PacketError) -> Self {
            return Self {
                kind: ErrorKind::PacketError(value.kind()),
                message: value.message().to_owned(),
            };
        }
    }

    impl From<tokio::io::Error> for ErrorKind {
        fn from(value: io::Error) -> Self {
            return Self::IoError(value);
        }
    }
}

pub mod server {
    #[derive(Debug, Clone)]
    pub struct ServerError {
        kind: ErrorKind,
        message: String,
    }

    #[derive(Debug, Copy, Clone)]
    pub enum ErrorKind {
        BroadcastError,
        FullMailbox,
    }

    impl ServerError {
        pub fn new(kind: ErrorKind, message: String) -> Self {
            Self { kind, message }
        }

        pub fn kind(&self) -> ErrorKind {
            return self.kind;
        }

        pub fn message(&self) -> &str {
            return &self.message;
        }
    }
}
