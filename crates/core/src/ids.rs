use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// 그룹 · 페이지의 식별자.
///
/// data.json 을 Swift 판 LinkGroups 와 그대로 주고받을 수 있도록, Swift 의 `UUID` 처럼 대문자로 적는다
/// (읽을 때는 대소문자를 가리지 않는다).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(Uuid);

impl Id {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text).ok().map(Self)
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = Uuid::encode_buffer();
        f.write_str(self.0.as_hyphenated().encode_upper(&mut buffer))
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Id {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text).map(Self)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
