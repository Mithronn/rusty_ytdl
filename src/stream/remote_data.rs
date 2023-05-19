#![allow(dead_code)]

use m3u8_rs::ByteRange;
use reqwest::header::{self, HeaderMap};

use super::hashable_byte_range::HashableByteRange;
use crate::VideoError;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RemoteData(url::Url, Option<HashableByteRange>);

impl RemoteData {
    pub fn new(url: impl Into<url::Url>, byte_range: Option<ByteRange>) -> Self {
        let url: url::Url = url.into();
        Self(url, byte_range.map(HashableByteRange::new))
    }

    pub fn url(&self) -> &url::Url {
        &self.0
    }

    pub fn byte_range_string(&self) -> Option<String> {
        let start = self.1.as_ref()?.offset.unwrap_or(0);
        let end = start + self.1.as_ref()?.length.saturating_sub(1);

        Some(format!("bytes={}-{}", start, end))
    }

    /// Fetch this segment and return (bytes, final url)
    pub async fn fetch(
        &self,
        client: &reqwest_middleware::ClientWithMiddleware,
    ) -> Result<(Vec<u8>, url::Url), VideoError> {
        // Add byte range headers if needed
        let mut header_map = HeaderMap::new();
        if let Some(ref range) = self.byte_range_string() {
            header_map.insert(
                header::RANGE,
                header::HeaderValue::from_str(range)
                    .unwrap_or(header::HeaderValue::from_str("").unwrap()),
            );
        }

        // Fetch data
        let resp = client
            .get(self.url().clone())
            .headers(header_map)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(VideoError::BodyCannotParsed);
        }
        let final_url = resp.url().clone();
        let bytes = resp.bytes().await?.into_iter().collect();

        Ok((bytes, final_url))
    }
}
