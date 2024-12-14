use bytes::Bytes;

use crate::{
    err::{DecodeError, DecodeErrorKind},
    io::decode_utf8,
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct TopicFilter(Vec<TopicToken>);

impl TopicFilter {
    pub fn decode(bytes: &mut Bytes) -> Result<Self, DecodeError> {
        let string = decode_utf8(bytes)?;
        let tokens = Self::from_str(string.as_str())?;
        return Ok(tokens);
    }

    pub fn from_str(str: &'_ str) -> Result<Self, DecodeError> {
        if str.len() == 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedTopicFilter,
                format!("Invalid topic filter, filter contains no bytes."),
            ));
        }

        let mut tokens = Vec::new();
        let mut strs = str.split('/').peekable();

        loop {
            if let Some(str) = strs.next() {
                let token = TopicToken::from_str(str);

                match token {
                    TopicToken::MultiLevel => {
                        if strs.peek().is_some() {
                            return Err(DecodeError::new(
                                DecodeErrorKind::MalformedTopicFilter,
                                format!("Invalid topic filter: {str}"),
                            ));
                        }
                    }
                    _ => {}
                }
                tokens.push(token);
            } else {
                break;
            }
        }

        return Ok(Self(tokens));
    }

    //TODO: this is really inefficient...
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

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Debug, Hash)]
pub struct TopicName(Vec<TopicToken>);

impl TopicName {
    pub fn decode(bytes: &mut Bytes) -> Result<(Self, &mut Bytes), DecodeError> {
        let string = decode_utf8(bytes)?;
        let tokens = Self::from_str(string.as_str())?;
        return Ok((tokens, bytes));
    }

    pub fn from_str(str: &'_ str) -> Result<Self, DecodeError> {
        if str.len() == 0 {
            return Err(DecodeError::new(
                DecodeErrorKind::MalformedTopicName,
                format!("Invalid topic name: {str}"),
            ));
        }

        let mut tokens = Vec::new();
        let mut strs = str.split('/');

        loop {
            if let Some(str) = strs.next() {
                let token = TopicToken::from_str(str);
                match token {
                    // valid token for a TopicName
                    TopicToken::String(_) | TopicToken::Dollar(_) => {
                        tokens.push(token);
                    }
                    // TopicName tokens cannot contain wildcards.
                    _ => {
                        return Err(DecodeError::new(
                            DecodeErrorKind::MalformedTopicName,
                            format!("Invalid topic name: {str}"),
                        ))
                    }
                }
            } else {
                break;
            }
        }

        return Ok(Self(tokens));
    }

    // this is really inefficient...
    pub fn to_string(self) -> String {
        let mut string = String::new();
        for token in self.into_iter() {
            string += token.as_str();
            string.push('/');
        }
        string.pop();
        return string;
    }

    // this is really inefficient... should be a property
    pub fn len(&self) -> usize {
        let mut len = 0;
        for token in &self.0 {
            match token {
                TopicToken::String(string) => len += string.len() + 1,
                _ => len += 2,
            }
        }

        return len - 1;
    }
}

impl IntoIterator for TopicName {
    type Item = TopicToken;
    type IntoIter = std::vec::IntoIter<TopicToken>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl PartialEq<TopicFilter> for TopicName {
    fn eq(&self, filter: &TopicFilter) -> bool {
        let mut name_iter = self.0.iter();
        let mut filter_iter = filter.0.iter().peekable();

        loop {
            match (name_iter.next(), filter_iter.next()) {
                (Some(name_token), Some(filter_token)) => {
                    if name_token != filter_token {
                        return false;
                    }

                    if let Some(next_filter) = filter_iter.peek() {
                        match next_filter {
                            TopicToken::MultiLevel => {
                                // check and make sure that the remaining topic name does not include a wildcard negator ($-prefixed).
                                loop {
                                    if let Some(name_token) = name_iter.next() {
                                        match name_token {
                                            TopicToken::Dollar(_) => return false,
                                            _ => {}
                                        }
                                    } else {
                                        return true;
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

impl PartialEq<TopicName> for TopicFilter {
    fn eq(&self, topic: &TopicName) -> bool {
        topic == self
    }
}

impl IntoIterator for TopicFilter {
    type Item = TopicToken;
    type IntoIter = std::vec::IntoIter<TopicToken>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(PartialOrd, Eq, Ord, Clone, Debug, Hash)]
pub enum TopicToken {
    Dollar(String),
    MultiLevel,
    SingleLevel,
    String(String),
}

impl PartialEq for TopicToken {
    fn eq(&self, other: &Self) -> bool {
        match self {
            TopicToken::Dollar(string) => match other {
                TopicToken::Dollar(o_string) => o_string == string,
                _ => false,
            },

            TopicToken::MultiLevel | TopicToken::SingleLevel => match other {
                TopicToken::Dollar(_) => false,
                _ => return true,
            },

            TopicToken::String(string) => match other {
                TopicToken::MultiLevel | TopicToken::SingleLevel => true,
                TopicToken::String(o_string) => string == o_string,
                _ => false,
            },
        }
    }
}

impl TopicToken {
    fn as_str<'a>(&'a self) -> &'a str {
        match self {
            Self::Dollar(string) => return string.as_str(),
            Self::MultiLevel => return "#",
            Self::SingleLevel => return "+",
            Self::String(string) => return string.as_str(),
        }
    }

    fn from_str(string: &'_ str) -> Self {
        if string.starts_with('$') {
            return Self::Dollar(String::from(string));
        }
        match string {
            "#" => return Self::MultiLevel,
            "+" => return Self::SingleLevel,
            _ => return Self::String(String::from(string)),
        }
    }
}

#[cfg(test)]
mod parsing {

    use crate::topic::TopicName;

    use super::TopicFilter;

    #[test]
    fn topic_filter_multi_level_wildcard() {
        let topic_filter = TopicFilter::from_str("sport/tennis/player1/#").unwrap();

        // pattern match on a generic topic_filter
        assert_eq!(
            TopicName::from_str("sport/tennis/player1").unwrap(),
            topic_filter
        );
        assert_eq!(
            TopicName::from_str("sport/tennis/player1/ranking").unwrap(),
            topic_filter
        );
        assert_eq!(
            TopicName::from_str("sport/tennis/player1/score/wimbledon").unwrap(),
            topic_filter
        );

        // multi-level wildcards also include the parent topic
        assert_eq!(
            TopicName::from_str("sport").unwrap(),
            TopicFilter::from_str("sport/#").unwrap()
        );

        // topic filters must be used with a seperator, otherwise demote from wildcard into character of the name.
        assert_ne!(TopicName::from_str("sport/tennis#").unwrap(), topic_filter);

        // multi-level wildcards must be at the end of a filter.
        assert!(TopicFilter::from_str("sport/tennis/#/ranking").is_err())
    }

    #[test]
    fn topic_filter_single_level_wildcard() {
        let topic_filter = TopicFilter::from_str("sport/tennis/+").unwrap();

        assert_eq!(
            topic_filter,
            TopicName::from_str("sport/tennis/player1").unwrap()
        );

        assert_eq!(
            topic_filter,
            TopicName::from_str("sport/tennis/player1").unwrap()
        );

        assert_ne!(
            topic_filter,
            TopicName::from_str("sport/tennis/player1/ranking").unwrap()
        );

        let topic_filter = TopicFilter::from_str("sport/+").unwrap();

        assert_eq!(topic_filter, TopicName::from_str("sport/").unwrap());
        assert_ne!(topic_filter, TopicName::from_str("sport").unwrap());

        assert_eq!(
            TopicFilter::from_str("sport/+/player1").unwrap(),
            TopicName::from_str("sport/tennis/player1").unwrap()
        );

        let topic_name = TopicName::from_str("/finance").unwrap();

        assert_eq!(TopicFilter::from_str("+/+").unwrap(), topic_name);
        assert_eq!(TopicFilter::from_str("/+").unwrap(), topic_name);
        assert_ne!(TopicFilter::from_str("+").unwrap(), topic_name);
    }

    #[test]
    fn topic_begining_with_dollar_sign() {
        let root_sub = TopicFilter::from_str("#").unwrap();

        assert_ne!(root_sub, TopicName::from_str("$SYS").unwrap());

        assert_ne!(
            TopicFilter::from_str("+/monitor/Clients").unwrap(),
            TopicName::from_str("$SYS/monitor/Clients").unwrap()
        );

        assert_eq!(
            TopicFilter::from_str("$SYS/#").unwrap(),
            TopicName::from_str("$SYS/anything/else").unwrap()
        );

        assert_eq!(
            TopicFilter::from_str("$SYS/monitor/+").unwrap(),
            TopicName::from_str("$SYS/monitor/Clients").unwrap()
        );

        // testing on nested $-negations
        assert_ne!(root_sub, TopicName::from_str("other/$something").unwrap());
    }
}
