use crate::err::{PacketError, PacketErrorKind};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub enum QosLevel {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl TryFrom<u8> for QosLevel {
    type Error = PacketError;
    /// Takes a byte with non-QoS bits masked, and QoS bits right-shifted to the right-hand side (idx 0)
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        // left shift 5, the right shift 6 to isolate the QoS bitflags
        let out = match value {
            0 => Self::AtMostOnce,
            1 => Self::AtLeastOnce,
            2 => Self::ExactlyOnce,
            _ => {
                // value of 0b0000_0110 is the only reachable value here
                return Err(PacketError::new(
                    PacketErrorKind::QoS,
                    format!("QoS MUST equal 0, 1, or 2: {}", value.to_string()),
                ));
            }
        };

        return Ok(out);
    }
}
