use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CardLanguage {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ko")]
    Korean,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardGame {
    Pokemon,
    Yugioh,
    StarwarsUnlimited,
    #[default]
    Unknown,
}

impl CardLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Korean => "ko",
        }
    }
}
impl CardGame {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pokemon => "pokemon",
            Self::Yugioh => "yugioh",
            Self::StarwarsUnlimited => "starwarsUnlimited",
            Self::Unknown => "unknown",
        }
    }
}
