use async_trait::async_trait;
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

    client: reqwest_middleware::ClientWithMiddleware,

    #[cfg(feature = "ffmpeg")]
    ffmpeg_args: Option<FFmpegArgs>,
    #[cfg(feature = "ffmpeg")]
    ffmpeg_start_byte: RwLock<Vec<u8>>,
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

        Ok(Self {
            client,
            link: options.link,
            content_length: options.content_length,
            dl_chunk_size: options.dl_chunk_size,
            start: RwLock::new(options.start),
            end: RwLock::new(options.end),
            #[cfg(feature = "ffmpeg")]
            ffmpeg_args: options.ffmpeg_args,
            #[cfg(feature = "ffmpeg")]
            ffmpeg_end_byte: RwLock::new(0),
            #[cfg(feature = "ffmpeg")]
            ffmpeg_start_byte: RwLock::new(vec![]),
        })
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
    async fn ffmpeg_start_byte_index(&self) -> Vec<u8> {
        self.ffmpeg_start_byte.read().await.to_vec()
    }
}

#[async_trait]
impl Stream for NonLiveStream {
    async fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError> {
        let end = self.end_index().await;

        // Nothing else remain send None to finish
        if end == 0 {
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

        let mut buf: Vec<u8> = vec![];

        while let Some(chunk) = response.chunk().await.map_err(VideoError::Reqwest)? {
            let chunk = chunk.to_vec();
            buf.extend(chunk.iter());
        }

        #[cfg(feature = "ffmpeg")]
        {
            let ffmpeg_args = self
                .ffmpeg_args
                .clone()
                .map(|x| x.build())
                .unwrap_or_default();

            if !ffmpeg_args.is_empty() {
                let ffmpeg_start_byte_index = self.ffmpeg_start_byte_index().await;

                let cmd_output = ffmpeg_cmd_run(
                    &ffmpeg_args,
                    &[&ffmpeg_start_byte_index, buf.as_slice()].concat(),
                )
                .await?;

                let end_index = self.ffmpeg_end_byte_index().await;

                let mut first_buffer_trim = 1;
                if ffmpeg_start_byte_index.is_empty() {
                    let mut start_byte = self.ffmpeg_start_byte.write().await;
                    *start_byte = buf.clone();
                    let mut end_byte = self.ffmpeg_end_byte.write().await;
                    *end_byte = cmd_output.len();

                    first_buffer_trim = 0;
                }

                buf = (cmd_output[(end_index + first_buffer_trim)..]).to_vec();
            }
        }

        if end != 0 {
            let mut start = self.start.write().await;
            *start = end + 1;
            let mut end = self.end.write().await;
            *end += self.dl_chunk_size;
        }

        Ok(Some(buf))
    }

    fn content_length(&self) -> usize {
        self.content_length() as usize
    }
}
