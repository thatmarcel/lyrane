use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum NowPlayingUpdateMessage {
    WithInfo(NowPlayingInfo),
    Empty {}
}

#[derive(Deserialize, Debug, Clone)]
pub struct NowPlayingInfo {
    pub title: String,
    #[serde(rename = "artistName")]
    pub artist_name: String,
    #[serde(rename = "currentPosition")]
    pub current_position_seconds: f64,
    #[serde(rename = "duration")]
    pub duration_seconds: f64,
    #[serde(rename = "playbackRate")]
    pub playback_rate: f64
}

impl NowPlayingInfo {
    pub fn is_same_item_as(&self, other: &Self) -> bool {
        self.title == other.title && self.artist_name == other.artist_name && self.duration_seconds == other.duration_seconds
    }

    pub fn is_same_item_as_in_option(&self, other: &Option<Self>) -> bool {
        if let &Some(other) = &other {
            self.is_same_item_as(other)
        } else {
            false
        }
    }
}