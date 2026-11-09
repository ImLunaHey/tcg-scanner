use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardLanguage {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ko")]
    Korean,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardGame {
    Pokemon,
    Yugioh,
    StarwarsUnlimited,
    #[default]
    Unknown,
}
