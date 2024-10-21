use crate::err::{DecodeError, DecodeErrorKind};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
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
