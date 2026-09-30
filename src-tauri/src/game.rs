use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameId {
    #[default]
    Genshin,
    StarRail,
}

impl GameId {
    pub fn key(self) -> &'static str {
        match self {
            Self::Genshin => "genshin",
            Self::StarRail => "star_rail",
        }
    }
}
