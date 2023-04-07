use m3u8_rs::{parse_playlist_res, Playlist};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::VideoError;

/// Return tuple => (m3u8 regex, dash_mpd regex)
pub(crate) const STREAM_TYPE_REGEX: Lazy<(regex::Regex, regex::Regex)> = Lazy::new(|| {
    let m3u8_regex = Regex::new(r"\.m3u8$").unwrap();
    let mpd_regex = Regex::new(r"\.mpd$").unwrap();

    return (m3u8_regex, mpd_regex);
});

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[derivative(Debug, PartialEq, Eq)]
pub enum StreamType {
    #[display(fmt = "StreamType(M3U8)")]
    M3U8,
    #[display(fmt = "StreamType(DASH MPD)")]
    DashMPD,
}

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "LiveStream({url})")]
#[derivative(Debug, PartialEq, Eq)]
pub struct LiveStream {
    stream_type: StreamType,
    url: String,
    #[derivative(PartialEq = "ignore")]
    client: reqwest_middleware::ClientWithMiddleware,
}

impl LiveStream {
    pub fn new(url: impl Into<String>, client: reqwest_middleware::ClientWithMiddleware) -> Self {
        let url: String = url.into();

        Self {
            stream_type: if STREAM_TYPE_REGEX.1.is_match(&url) {
                StreamType::DashMPD
            } else {
                StreamType::M3U8
            },
            url,
            client,
        }
    }

    pub async fn parse(&self) -> Result<(), VideoError> {
        let response = self.client.get(&self.url).send().await;

        if response.is_err() {
            return Err(VideoError::ReqwestMiddleware(response.err().unwrap()));
        }

        let response = response.unwrap().text().await;

        if response.is_err() {
            return Err(VideoError::BodyCannotParsed);
        }

        let body = response.unwrap();

        let parsed = parse_playlist_res(body.as_bytes());

        if parsed.is_err() {
            return Err(VideoError::M3U8ParseError(
                parsed.err().unwrap().to_string(),
            ));
        }

        let playlist = parsed.expect("IMPOSSIBLE");

        println!("{playlist:?}");

        Ok(())
    }
}
