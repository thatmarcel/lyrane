use std::str::FromStr;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Debug, Clone)]
pub struct LyricsLine {
    #[serde(deserialize_with = "deserialize_f64_from_f64_or_string")]
    pub seconds: f64,
    #[serde(rename = "lyrics")]
    pub content: String
}

pub fn deserialize_f64_from_f64_or_string<'de, D>(deserializer: D) -> Result<f64, D::Error> where D: Deserializer<'de> {
    StringOrF64::deserialize(deserializer).map(|value| match value {
        StringOrF64::Str(s) => f64::from_str(&s).unwrap_or(-1f64),
        StringOrF64::Float(f) => f
    })
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum StringOrF64 {
    Str(String),
    Float(f64)
}