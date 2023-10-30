use crate::constants::DEFAULT_HEADERS;
use crate::stream::streams::Stream;
use crate::structs::VideoError;
use async_trait::async_trait;
use tokio::sync::RwLock;

pub struct NonLiveStreamOptions {
    pub client: Option<reqwest_middleware::ClientWithMiddleware>,
    pub link: String,
    pub content_length: u64,
    pub dl_chunk_size: u64,
    pub start: u64,
    pub end: u64,
}

pub struct NonLiveStream {
    link: String,
    content_length: u64,
    dl_chunk_size: u64,
    start: RwLock<u64>,
    end: RwLock<u64>,

    client: reqwest_middleware::ClientWithMiddleware,
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

        let response = self.client.get(&self.link).headers(headers).send().await;

        if response.is_err() {
            return Err(VideoError::ReqwestMiddleware(response.err().unwrap()));
        }

        let mut response = response.expect("IMPOSSIBLE");

        let mut buf: Vec<u8> = vec![];

        while let Some(chunk) = response.chunk().await.map_err(VideoError::Reqwest)? {
            let chunk = chunk.to_vec();
            buf.extend(chunk.iter());
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
