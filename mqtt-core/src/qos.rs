use std::u8;

use serde::{de::Visitor, Deserialize, Serialize};

use crate::err::{DecodeError, DecodeErrorKind};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
#[repr(u8)]
pub enum QosLevel {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl PartialEq<SubAckQoS> for QosLevel {
    fn eq(&self, other: &SubAckQoS) -> bool {
        return other == self;
    }
}

impl TryFrom<u8> for QosLevel {
    type Error = DecodeError;
    /// Takes a byte with non-QoS bits masked, and QoS bits right-shifted to the right-hand side (idx 0)
    fn try_from(value: u8) -> Result<Self, DecodeError> {
        // left shift 5, the right shift 6 to isolate the QoS bitflags
        let out = match value {
            0 => Self::AtMostOnce,
            1 => Self::AtLeastOnce,
            2 => Self::ExactlyOnce,
            _ => {
                // value of 0b0000_0110 is the only reachable value here
                // return Err(DecodeError::QoS);
                return Err(DecodeError::new(
                    DecodeErrorKind::QoS,
                    format!("Invalid QoS: {value}, only values 0-2 are valid"),
                ));
            }
        };

        return Ok(out);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum SubAckQoS {
    QOS(QosLevel),
    Err,
}

impl PartialEq<QosLevel> for SubAckQoS {
    fn eq(&self, other: &QosLevel) -> bool {
        match self {
            Self::Err => return false,
            Self::QOS(qos) => return qos == other,
        }
    }
}

impl From<QosLevel> for SubAckQoS {
    fn from(value: QosLevel) -> Self {
        return Self::QOS(value);
    }
}

/*
* Allowed return codes:
* 0x00 - Success - Maximum QoS 0
* 0x01 - Success - Maximum QoS 1
* 0x02 - Success - Maximum QoS 2
* 0x80 - Failure
*/
impl Into<u8> for SubAckQoS {
    fn into(self) -> u8 {
        match self {
            Self::Err => return 0b1000_0000,
            Self::QOS(qos) => return qos as u8,
        }
    }
}

impl TryFrom<u8> for SubAckQoS {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 0b1000_0000 {
            return Ok(Self::Err);
        } else {
            return Ok(Self::QOS(QosLevel::try_from(value)?));
        }
    }
}

struct QosLevelVisitor;

impl<'de> Visitor<'de> for QosLevelVisitor {
    type Value = QosLevel;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an integer between 0 and 2 (inclusive)")
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value <= i8::MAX as i64 {
            QosLevel::try_from(value as u8).map_err(|_| {
                E::invalid_value(serde::de::Unexpected::Signed(value), &"values 0, 1 or 2")
            })
        } else {
            Err(E::invalid_value(
                serde::de::Unexpected::Signed(value),
                &"a valid QoS level (0, 1, or 2)",
            ))
        }
    }
}

impl<'de> Deserialize<'de> for QosLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_i64(QosLevelVisitor);
    }
}

impl Serialize for QosLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}
