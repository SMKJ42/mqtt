use core::fmt::Display;

use err::{DecodeError, DecodeErrorKind};

mod codec;
pub mod err;
pub mod id;
pub mod io;
pub mod msg_assurance;
pub mod qos;
pub mod topic;
pub use codec::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConnectReturnCode {
    Accept = 0,
    InvalidProtocol = 1,
    IdentifierRejected = 2,
    ServerUnavailable = 3,
    BadUsernameOrPassword = 4,
    NotAuthorized = 5,
}

impl TryFrom<u8> for ConnectReturnCode {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, DecodeError> {
        match value {
            0 => return Ok(Self::Accept),
            1 => return Ok(Self::InvalidProtocol),
            2 => return Ok(Self::IdentifierRejected),
            3 => return Ok(Self::ServerUnavailable),
            4 => return Ok(Self::BadUsernameOrPassword),
            5 => return Ok(Self::NotAuthorized),
            _ => Err(DecodeError::new(
                DecodeErrorKind::InvalidReturnCode,
                format!("Return code: {value}, is invalid, only values of 0-5 are valid."),
            )),
        }
    }
}

impl Display for ConnectReturnCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => {
                write!(f, "Accept")
            }
            Self::BadUsernameOrPassword => {
                write!(f, "Bad username or password")
            }
            Self::IdentifierRejected => {
                write!(f, "Identifier rejected")
            }
            Self::InvalidProtocol => {
                write!(f, "Invalid protocol")
            }
            Self::NotAuthorized => {
                write!(f, "Not authorized")
            }
            Self::ServerUnavailable => {
                write!(f, "Server unavailable")
            }
        }
    }
}
