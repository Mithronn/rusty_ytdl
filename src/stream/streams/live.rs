use crate::constants::DEFAULT_HEADERS;
use crate::stream::encryption::Encryption;
use crate::stream::media_format::MediaFormat;
use crate::stream::remote_data::RemoteData;
use crate::stream::segment::Segment;
use crate::stream::streams::Stream;
use crate::structs::VideoError;
use crate::utils::{get_html, make_absolute_url};
use async_trait::async_trait;
use m3u8_rs::parse_media_playlist;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

pub struct LiveStreamOptions {
    pub client: Option<reqwest_middleware::ClientWithMiddleware>,
    pub stream_url: String,
}

pub struct LiveStream {
    client: reqwest_middleware::ClientWithMiddleware,
    stream_url: String,

    last_refresh: RwLock<u128>,
    segments: RwLock<Vec<(Segment, Encryption)>>,
    is_end: RwLock<bool>,
    last_seg: RwLock<Option<(u64, u64)>>,
}

impl LiveStream {
    pub fn new(options: LiveStreamOptions) -> Result<Self, VideoError> {
        let client = if options.client.is_some() {
            options.client.unwrap()
        } else {
            let client = reqwest::Client::builder()
                .build()
                .map_err(VideoError::Reqwest)?;

            let retry_policy = reqwest_retry::policies::ExponentialBackoff::builder()
                .retry_bounds(
                    std::time::Duration::from_millis(500),
                    std::time::Duration::from_millis(10000),
                )
                .build_with_max_retries(3);
            reqwest_middleware::ClientBuilder::new(client)
                .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
                    retry_policy,
                ))
                .build()
        };

        Ok(Self {
            client,
            stream_url: options.stream_url,
            last_refresh: RwLock::new(0),
            segments: RwLock::new(vec![]),
            is_end: RwLock::new(false),
            last_seg: RwLock::new(None),
        })
    }

    async fn last_refresh(&self) -> u128 {
        *self.last_refresh.read().await
    }

    async fn segments(&self) -> Vec<(Segment, Encryption)> {
        (*self.segments.read().await).clone()
    }

    async fn is_end(&self) -> bool {
        *self.is_end.read().await
    }

    async fn last_seg(&self) -> Option<(u64, u64)> {
        *self.last_seg.read().await
    }

    async fn refresh_playlist(&self) -> Result<(), VideoError> {
        let body = get_html(&self.client, &self.stream_url, None).await?;

        let media_playlist = parse_media_playlist(body.as_bytes())
            .map_err(|e| VideoError::M3U8ParseError(e.to_string()))?
            .1;

        let mut cur_init = None;

        // Loop through media segments
        let mut discon_offset = 0;
        let mut encryption = Encryption::None;
        for (seq, segment) in (media_playlist.media_sequence..).zip(media_playlist.segments.iter())
        {
            // Calculate segment discontinuity
            if segment.discontinuity {
                discon_offset += 1;
            }
            let discon_seq = media_playlist.discontinuity_sequence + discon_offset;

            // Skip segment if already downloaded
            if let Some(s) = self.last_seg().await {
                if s >= (discon_seq, seq) {
                    continue;
                }
            }

            // Check encryption
            if let Some(key) = &segment.key {
                encryption = Encryption::new(key, &self.stream_url, seq).await?;
            }

            // Segment is new
            let mut mut_last_seg = self.last_seg.write().await;
            *mut_last_seg = Some((discon_seq, seq));

            // Parse URL
            let seg_url = make_absolute_url(&self.stream_url, &segment.uri)?;

            // Make Initialization
            let init = if let Some(map) = &segment.map {
                let init = RemoteData::new(
                    make_absolute_url(&self.stream_url, &map.uri)?,
                    map.byte_range.clone(),
                );
                cur_init = Some(init.clone());
                Some(init)
            } else {
                cur_init.clone()
            };

            let segment = Segment {
                data: RemoteData::new(seg_url, segment.byte_range.clone()),
                discon_seq,
                seq,
                format: MediaFormat::Unknown,
                initialization: init,
            };

            // if segments already in segment vector skip it
            if !self
                .segments()
                .await
                .iter()
                .any(|x| (x.0.discon_seq, x.0.seq) == (segment.discon_seq, segment.seq))
            {
                let mut segment_vector = self.segments.write().await;
                segment_vector.push((segment.clone(), encryption.clone()));
            }
        }

        // Set last refresh to check refresh playlist functionality
        let mut last_refresh = self.last_refresh.write().await;
        let start = SystemTime::now();
        *last_refresh = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        drop(last_refresh);

        // Set is_end bool to control chunk function
        // if stream ended
        if media_playlist.end_list {
            let mut is_end = self.is_end.write().await;
            *is_end = media_playlist.end_list;
        }

        Ok(())
    }
}

#[async_trait]
impl Stream for LiveStream {
    async fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError> {
        let segments = self.segments().await;

        // if stream end and no segments left end it
        if self.is_end().await && segments.is_empty() {
            return Ok(None);
        }

        let live_seconds = 20000; // refresh millis

        let start = SystemTime::now();
        let current_time = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        let sleep_time = current_time - self.last_refresh().await;

        // Sleep until to wait new segments uploaded to get new segments
        if sleep_time < live_seconds && segments.is_empty() && !self.is_end().await {
            tokio::time::sleep_until(
                tokio::time::Instant::now()
                    + Duration::from_millis((live_seconds - sleep_time) as u64),
            )
            .await;
        }

        let start = SystemTime::now();
        let current_time = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // if last refresh bigger than live_seconds refresh playlist
        if current_time - self.last_refresh().await >= live_seconds && !self.is_end().await {
            self.refresh_playlist().await?;
        }

        // cannot get any segments return empty buffer array
        let segments = self.segments().await;
        if segments.is_empty() {
            return Ok(Some(vec![]));
        }

        let first_segment = segments.first().unwrap();

        let headers = DEFAULT_HEADERS.clone();

        let response = self
            .client
            .get(first_segment.0.url().as_str())
            .headers(headers)
            .send()
            .await;

        if response.is_err() {
            return Err(VideoError::ReqwestMiddleware(response.err().unwrap()));
        }

        let mut response = response.expect("IMPOSSIBLE");

        let mut buf: Vec<u8> = vec![];

        while let Some(chunk) = response.chunk().await.map_err(VideoError::Reqwest)? {
            let chunk = chunk.to_vec();
            buf.extend(chunk.iter());
        }

        // Decrypt data bytes
        buf = first_segment.1.decrypt(&self.client, &buf).await?;

        // Delete downloaded segment from segments array
        let mut segment_vector = self.segments.write().await;
        segment_vector.remove(0);

        Ok(Some(buf))
    }
}
