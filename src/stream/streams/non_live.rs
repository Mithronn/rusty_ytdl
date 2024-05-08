use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::sync::RwLock;

use crate::constants::DEFAULT_HEADERS;
use crate::stream::streams::Stream;
use crate::structs::VideoError;

#[cfg(feature = "ffmpeg")]
use crate::{structs::FFmpegArgs, utils::ffmpeg_cmd_run};

pub struct NonLiveStreamOptions {
    pub client: Option<reqwest_middleware::ClientWithMiddleware>,
    pub link: String,
    pub content_length: u64,
    pub dl_chunk_size: u64,
    pub start: u64,
    pub end: u64,

    #[cfg(feature = "ffmpeg")]
    pub ffmpeg_args: Option<FFmpegArgs>,
}

pub struct NonLiveStream {
    link: String,
    content_length: u64,
    dl_chunk_size: u64,
    start: RwLock<u64>,
    end: RwLock<u64>,
    start_static: u64,
    end_static: u64,

    client: reqwest_middleware::ClientWithMiddleware,

    #[cfg(feature = "ffmpeg")]
    ffmpeg_args: Vec<String>,
    #[cfg(feature = "ffmpeg")]
    ffmpeg_start_byte: RwLock<Bytes>,
    #[cfg(feature = "ffmpeg")]
    ffmpeg_end_byte: RwLock<usize>,
}

impl NonLiveStream {
    pub fn new(options: NonLiveStreamOptions) -> Result<Self, VideoError> {
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

        #[cfg(feature = "ffmpeg")]
        {
            let ffmpeg_args = options
                .ffmpeg_args
                .clone()
                .map(|x| x.build())
                .unwrap_or_default();

            Ok(Self {
                client,
                link: options.link,
                content_length: options.content_length,
                dl_chunk_size: options.dl_chunk_size,
                start: RwLock::new(options.start),
                end: RwLock::new(options.end),
                start_static: options.start,
                end_static: options.end,
                ffmpeg_args: ffmpeg_args.clone(),
                ffmpeg_end_byte: RwLock::new(0),
                ffmpeg_start_byte: RwLock::new(Bytes::new()),
            })
        }

        #[cfg(not(feature = "ffmpeg"))]
        {
            Ok(Self {
                client,
                link: options.link,
                content_length: options.content_length,
                dl_chunk_size: options.dl_chunk_size,
                start: RwLock::new(options.start),
                end: RwLock::new(options.end),
                start_static: options.start,
                end_static: options.end,
            })
        }
    }

    pub fn content_length(&self) -> u64 {
        self.content_length
    }

    async fn end_index(&self) -> u64 {
        *self.end.read().await
    }

    async fn start_index(&self) -> u64 {
        *self.start.read().await
    }

    #[cfg(feature = "ffmpeg")]
    async fn ffmpeg_end_byte_index(&self) -> usize {
        *self.ffmpeg_end_byte.read().await
    }

    #[cfg(feature = "ffmpeg")]
    async fn ffmpeg_start_byte_index(&self) -> Bytes {
        self.ffmpeg_start_byte.read().await.to_vec().into()
    }
}

#[async_trait]
impl Stream for NonLiveStream {
    async fn chunk(&self) -> Result<Option<Bytes>, VideoError> {
        let end = self.end_index().await;

        // Nothing else remain set controllers to the beginning state and send None to finish
        if end == 0 {
            let mut end = self.end.write().await;
            let mut start = self.start.write().await;
            *end = self.end_static;
            *start = self.start_static;

            #[cfg(feature = "ffmpeg")]
            {
                let mut ffmpeg_end_byte = self.ffmpeg_end_byte.write().await;
                let mut ffmpeg_start_byte = self.ffmpeg_start_byte.write().await;
                *ffmpeg_end_byte = 0;
                *ffmpeg_start_byte = Bytes::new();
            }

            // Send None to close
            return Ok(None);
        }

        if end >= self.content_length {
            let mut end = self.end.write().await;
            *end = 0;
        }

        let mut headers = DEFAULT_HEADERS.clone();

        let end = self.end_index().await;
        let range_end = if end == 0 {
            "".to_string()
        } else {
            end.to_string()
        };

        headers.insert(
            reqwest::header::RANGE,
            format!("bytes={}-{}", self.start_index().await, range_end)
                .parse()
                .unwrap(),
        );

        let mut response = self
            .client
            .get(&self.link)
            .headers(headers)
            .send()
            .await
            .map_err(VideoError::ReqwestMiddleware)?;

        let mut buf: BytesMut = BytesMut::new();

        while let Some(chunk) = response.chunk().await.map_err(VideoError::Reqwest)? {
            buf.extend(chunk);
        }

        #[cfg(feature = "ffmpeg")]
        {
            if !self.ffmpeg_args.is_empty() {
                let ffmpeg_start_byte_index = self.ffmpeg_start_byte_index().await;

                let cmd_output = ffmpeg_cmd_run(
                    &self.ffmpeg_args,
                    Bytes::from(
                        [
                            BytesMut::from_iter(ffmpeg_start_byte_index.clone()),
                            buf.clone(),
                        ]
                        .concat(),
                    ),
                )
                .await?;

                let end_index = self.ffmpeg_end_byte_index().await;

                let mut first_buffer_trim = if cmd_output.is_empty() { 0 } else { 1 };
                if ffmpeg_start_byte_index.is_empty() {
                    let mut start_byte = self.ffmpeg_start_byte.write().await;
                    *start_byte = buf.into();
                    let mut end_byte = self.ffmpeg_end_byte.write().await;
                    *end_byte = cmd_output.len();

                    first_buffer_trim = 0;
                }

                buf = BytesMut::from(&cmd_output[(end_index + first_buffer_trim)..]);
            }
        }

        if end != 0 {
            let mut start = self.start.write().await;
            *start = end + 1;
            let mut end = self.end.write().await;
            *end += self.dl_chunk_size;
        }

        Ok(Some(buf.into()))
    }

    fn content_length(&self) -> usize {
        self.content_length() as usize
    }
}
