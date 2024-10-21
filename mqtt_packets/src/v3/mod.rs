use bytes::Bytes;

pub mod conack;
pub mod connect;
pub mod disconnect;
pub mod pingreq;
pub mod pingresp;
pub mod puback;
pub mod pubcomp;
pub mod publish;
pub mod pubrec;
pub mod pubrel;
pub mod suback;
pub mod subscribe;
pub mod unsuback;
pub mod unsubscribe;

use crate::{
    err::{PacketError, PacketErrorKind},
    io::decode_utf8,
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
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

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct TopicFilter(Vec<TopicToken>);

impl TopicFilter {
    pub fn decode(bytes: Bytes) -> Result<(Self, Bytes), PacketError> {
        let (string, bytes) = decode_utf8(bytes)?;
        let tokens = Self::from_str(string.as_str())?;
        return Ok((tokens, bytes));
    }

    pub fn from_str(str: &'_ str) -> Result<Self, PacketError> {
        let mut tokens = Vec::new();
        let mut strs = str.split('/').peekable();

        loop {
            if let Some(str) = strs.peek() {
                let token = TopicToken::from_str(*str);
                tokens.push(token);

                // consume the str to advance the iterator.
                strs.next();
            } else {
                break;
            }
        }
        if let Some(token) = tokens.iter().last() {
            if token == &TopicToken::MultiLevel {
                if strs.peek().is_some() {
                    return Err(PacketError::new(
                        PacketErrorKind::MalformedTopicFilter,
                        format!(
                            "The multi-level wildcard '#' must be at the end of a topic filter. {}",
                            &str
                        ),
                    ));
                }
            }
        }

        return Ok(Self(tokens));
    }

    pub fn to_string(self) -> String {
        let mut string = String::new();
        for token in self.into_iter() {
            string += token.as_str();
        }
        return string;
    }

    //TODO: this is really inefficient...
    pub fn len(&self) -> usize {
        let mut len = 0;
        for token in &self.0 {
            match token {
                TopicToken::String(string) => len += string.len(),
                _ => len += 1,
            }
        }

        return len;
    }
}

impl IntoIterator for TopicFilter {
    type Item = TopicToken;
    type IntoIter = std::vec::IntoIter<TopicToken>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Eq, PartialOrd, Ord, Clone, Debug, PartialEq)]
pub enum TopicToken {
    MultiLevel,
    SingleLevel,
    String(String),
}

impl TopicToken {
    fn as_str<'a>(&'a self) -> &'a str {
        match self {
            Self::MultiLevel => return "#",
            Self::SingleLevel => return "+",
            Self::String(string) => return string.as_str(),
        }
    }

    fn from_str(string: &'_ str) -> Self {
        match string {
            "#" => return Self::MultiLevel,
            "+" => return Self::SingleLevel,
            _ => return Self::String(String::from(string)),
        }
    }
}
