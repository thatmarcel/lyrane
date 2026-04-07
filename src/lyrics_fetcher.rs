use crate::lyrics_line::LyricsLine;

pub fn fetch_lyrics(title: &str, artist_name: &str) -> anyhow::Result<Vec<LyricsLine>> {
    const RETRY_COUNT: u8 = 3;

    let mut result = _fetch_lyrics(title, artist_name);

    for _ in 0..RETRY_COUNT {
        if result.is_ok() {
            break;
        }

        result = _fetch_lyrics(title, artist_name);
    }

    result
}

fn _fetch_lyrics(title: &str, artist_name: &str) -> anyhow::Result<Vec<LyricsLine>> {
    Ok(ureq::get("https://prv.textyl.co/api/lyrics")
        .query("name", title)
        .query("artist", artist_name)
        .query("pt", "true")
        .call()?
        .body_mut()
        .read_json::<Vec<LyricsLine>>()?
    )
}