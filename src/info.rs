use reqwest::{
    header::{HeaderMap, HeaderValue, COOKIE},
    Client,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use scraper::{Html, Selector};
use std::{path::Path, time::Duration};
use url::Url;

#[cfg(feature = "live")]
use crate::stream::{LiveStream, LiveStreamOptions};
#[cfg(feature = "ffmpeg")]
use crate::structs::FFmpegArgs;

use crate::{
    constants::BASE_URL,
    info_extras::{get_media, get_related_videos},
    stream::{NonLiveStream, NonLiveStreamOptions, Stream},
    structs::{PlayerResponse, VideoError, VideoInfo, VideoOptions},
    utils::{
        between, choose_format, clean_video_details, get_functions, get_html, get_html5player,
        get_random_v6_ip, get_video_id, is_not_yet_broadcasted, is_play_error, is_private_video,
        is_rental, parse_live_video_formats, parse_video_formats, sort_formats,
    },
};

// 10485760 -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
pub(crate) const DEFAULT_DL_CHUNK_SIZE: u64 = 10485760;

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "Video({video_id})")]
#[derivative(Debug, PartialEq, Eq)]
pub struct Video {
    video_id: String,
    options: VideoOptions,
    #[derivative(PartialEq = "ignore")]
    client: ClientWithMiddleware,
}

impl Video {
    /// Crate [`Video`] struct to get info or download with default [`VideoOptions`]
    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        let video_id = get_video_id(&url_or_id.into()).ok_or(VideoError::VideoNotFound)?;

        let client = Client::builder().build().map_err(VideoError::Reqwest)?;

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(500), Duration::from_millis(10000))
            .build_with_max_retries(3);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            video_id,
            options: VideoOptions::default(),
            client,
        })
    }

    /// Crate [`Video`] struct to get info or download with custom [`VideoOptions`]
    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: VideoOptions,
    ) -> Result<Self, VideoError> {
        let video_id = get_video_id(&url_or_id.into()).ok_or(VideoError::VideoNotFound)?;

        let client = match options.request_options.client.clone() {
            Some(client) => client,
            None => {
                let mut client_builder = Client::builder();

                if let Some(proxy) = &options.request_options.proxy {
                    client_builder = client_builder.proxy(proxy.clone());
                }

                if let Some(ipv6_block) = &options.request_options.ipv6_block {
                    let ipv6 = get_random_v6_ip(ipv6_block)?;
                    client_builder = client_builder.local_address(ipv6);
                }

                if let Some(cookie) = &options.request_options.cookies {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        COOKIE,
                        HeaderValue::from_str(cookie).map_err(|_x| VideoError::CookieError)?,
                    );

                    client_builder = client_builder.default_headers(headers)
                }

                client_builder.build().map_err(VideoError::Reqwest)?
            }
        };

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(500), Duration::from_millis(10000))
            .build_with_max_retries(3);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            video_id,
            options,
            client,
        })
    }

    /// Try to get basic information about video
    /// - `HLS` and `DashMPD` formats excluded!
    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    pub async fn get_basic_info(&self) -> Result<VideoInfo, VideoError> {
        let client = &self.client;

        let url_parsed = Url::parse_with_params(self.get_video_url().as_str(), &[("hl", "en")])
            .map_err(VideoError::URLParseError)?;

        let response = get_html(client, url_parsed.as_str(), None).await?;

        let (player_response, initial_response): (PlayerResponse, serde_json::Value) = {
            let document = Html::parse_document(&response);
            let scripts_selector = Selector::parse("script").unwrap();
            let player_response_string = document
                .select(&scripts_selector)
                .filter(|x| x.inner_html().contains("var ytInitialPlayerResponse ="))
                .map(|x| x.inner_html().replace("var ytInitialPlayerResponse =", ""))
                .next()
                .unwrap_or(String::from(""));
            let mut initial_response_string = document
                .select(&scripts_selector)
                .filter(|x| x.inner_html().contains("var ytInitialData ="))
                .map(|x| x.inner_html().replace("var ytInitialData =", ""))
                .next()
                .unwrap_or(String::from(""));

            // remove json object last element (;)
            initial_response_string.pop();

            let player_response = serde_json::from_str::<PlayerResponse>(
                format!(
                    "{{{}}}}}}}",
                    between(player_response_string.trim(), "{", "}}};")
                )
                .as_str(),
            )
            .unwrap();

            let initial_response =
                serde_json::from_str::<serde_json::Value>(initial_response_string.trim()).unwrap();

            (player_response, initial_response)
        };

        if is_play_error(&player_response, ["ERROR"].to_vec()) {
            return Err(VideoError::VideoNotFound);
        }

        if is_private_video(&player_response) {
            return Err(VideoError::VideoIsPrivate);
        }

        if player_response.streaming_data.is_none()
            || is_rental(&player_response)
            || is_not_yet_broadcasted(&player_response)
        {
            return Err(VideoError::VideoSourceNotFound);
        }

        let video_details = clean_video_details(
            &initial_response,
            &player_response,
            get_media(&initial_response).unwrap(),
            self.video_id.clone(),
        );

        let dash_manifest_url = player_response
            .streaming_data
            .as_ref()
            .and_then(|x| x.dash_manifest_url.clone());

        let hls_manifest_url = player_response
            .streaming_data
            .as_ref()
            .and_then(|x| x.hls_manifest_url.clone());

        Ok(VideoInfo {
            dash_manifest_url,
            hls_manifest_url,
            formats: {
                parse_video_formats(
                    &player_response,
                    get_functions(get_html5player(response.as_str()).unwrap(), client).await?,
                )
                .unwrap_or_default()
            },
            related_videos: { get_related_videos(&initial_response).unwrap_or_default() },
            video_details,
        })
    }

    /// Try to get full information about video
    /// - `HLS` and `DashMPD` formats included!
    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    pub async fn get_info(&self) -> Result<VideoInfo, VideoError> {
        let mut info = self.get_basic_info().await?;

        if let Some(url) = &info.hls_manifest_url {
            if let Ok(unformated_formats) = get_m3u8(url, &self.client).await {
                info.formats
                    .extend(parse_live_video_formats(unformated_formats));
            }
        }

        // Last sort formats
        info.formats.sort_by(sort_formats);
        Ok(info)
    }

    /// Try to turn [`Stream`] implemented [`LiveStream`] or [`NonLiveStream`] depend on the video.
    /// If function successfully return can download video chunk by chunk
    /// # Example
    /// ```ignore
    ///     let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";
    ///
    ///     let video = Video::new(video_url).unwrap();
    ///
    ///     let stream = video.stream().await.unwrap();
    ///
    ///     while let Some(chunk) = stream.chunk().await.unwrap() {
    ///           println!("{:#?}", chunk);
    ///     }
    /// ```
    pub async fn stream(&self) -> Result<Box<dyn Stream + Send + Sync>, VideoError> {
        let client = &self.client;

        let info = self.get_info().await?;
        let format = choose_format(&info.formats, &self.options)
            .map_err(|_op| VideoError::VideoSourceNotFound)?;

        let link = format.url;

        if link.is_empty() {
            return Err(VideoError::VideoSourceNotFound);
        }

        // Only check for HLS formats for live streams
        if format.is_hls {
            #[cfg(feature = "live")]
            {
                let stream = LiveStream::new(LiveStreamOptions {
                    client: Some(client.clone()),
                    stream_url: link,
                })?;

                return Ok(Box::new(stream));
            }
            #[cfg(not(feature = "live"))]
            {
                return Err(VideoError::LiveStreamNotSupported);
            }
        }

        let dl_chunk_size = self
            .options
            .download_options
            .dl_chunk_size
            .unwrap_or(DEFAULT_DL_CHUNK_SIZE);

        let start = 0;
        let end = start + dl_chunk_size;

        let mut content_length = format
            .content_length
            .unwrap_or("0".to_string())
            .parse::<u64>()
            .unwrap_or(0);

        // Get content length from source url if content_length is 0
        if content_length == 0 {
            let content_length_response = client
                .get(&link)
                .send()
                .await
                .map_err(VideoError::ReqwestMiddleware)?
                .content_length()
                .ok_or(VideoError::VideoNotFound)?;

            content_length = content_length_response;
        }

        let stream = NonLiveStream::new(NonLiveStreamOptions {
            client: Some(client.clone()),
            link,
            content_length,
            dl_chunk_size,
            start,
            end,
            #[cfg(feature = "ffmpeg")]
            ffmpeg_args: None,
        })?;

        Ok(Box::new(stream))
    }

    #[cfg(feature = "ffmpeg")]
    /// Try to turn [`Stream`] implemented [`LiveStream`] or [`NonLiveStream`] depend on the video with [`FFmpegArgs`].
    /// If function successfully return can download video with applied ffmpeg filters and formats chunk by chunk
    /// # Example
    /// ```ignore
    ///     let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";
    ///
    ///     let video = Video::new(video_url).unwrap();
    ///
    ///     let stream = video.stream_with_ffmpeg(Some(FFmpegArgs {
    ///            format: Some("mp3".to_string()),
    ///            audio_filter: Some("aresample=48000,asetrate=48000*0.8".to_string()),
    ///            video_filter: Some("eq=brightness=150:saturation=2".to_string()),
    ///        })).await.unwrap();
    ///
    ///     while let Some(chunk) = stream.chunk().await.unwrap() {
    ///           println!("{:#?}", chunk);
    ///     }
    /// ```
    pub async fn stream_with_ffmpeg(
        &self,
        ffmpeg_args: Option<FFmpegArgs>,
    ) -> Result<Box<dyn Stream + Send + Sync>, VideoError> {
        let client = &self.client;

        let info = self.get_info().await?;
        let format = choose_format(&info.formats, &self.options)
            .map_err(|_op| VideoError::VideoSourceNotFound)?;

        let link = format.url;

        if link.is_empty() {
            return Err(VideoError::VideoSourceNotFound);
        }

        // Only check for HLS formats for live streams
        if format.is_hls {
            #[cfg(feature = "live")]
            {
                let stream = LiveStream::new(LiveStreamOptions {
                    client: Some(client.clone()),
                    stream_url: link,
                })?;

                return Ok(Box::new(stream));
            }
            #[cfg(not(feature = "live"))]
            {
                return Err(VideoError::LiveStreamNotSupported);
            }
        }

        let dl_chunk_size = self
            .options
            .download_options
            .dl_chunk_size
            .unwrap_or(DEFAULT_DL_CHUNK_SIZE);

        let start = 0;
        let end = start + dl_chunk_size;

        let mut content_length = format
            .content_length
            .unwrap_or("0".to_string())
            .parse::<u64>()
            .unwrap_or(0);

        // Get content length from source url if content_length is 0
        if content_length == 0 {
            let content_length_response = client
                .get(&link)
                .send()
                .await
                .map_err(VideoError::ReqwestMiddleware)?
                .content_length()
                .ok_or(VideoError::VideoNotFound)?;

            content_length = content_length_response;
        }

        let stream = NonLiveStream::new(NonLiveStreamOptions {
            client: Some(client.clone()),
            link,
            content_length,
            dl_chunk_size,
            start,
            end,
            ffmpeg_args,
        })?;

        Ok(Box::new(stream))
    }

    /// Download video directly to the file
    pub async fn download<P: AsRef<Path>>(&self, path: P) -> Result<(), VideoError> {
        use std::{fs::File, io::Write};

        let stream = self.stream().await?;

        let mut file = File::create(path).map_err(|e| VideoError::DownloadError(e.to_string()))?;

        while let Some(chunk) = stream.chunk().await? {
            file.write_all(&chunk)
                .map_err(|e| VideoError::DownloadError(e.to_string()))?;
        }

        Ok(())
    }

    #[cfg(feature = "ffmpeg")]
    /// Download video with ffmpeg args directly to the file
    pub async fn download_with_ffmpeg<P: AsRef<Path>>(
        &self,
        path: P,
        ffmpeg_args: Option<FFmpegArgs>,
    ) -> Result<(), VideoError> {
        use std::{fs::File, io::Write};

        let stream = self.stream_with_ffmpeg(ffmpeg_args).await?;

        let mut file = File::create(path).map_err(|e| VideoError::DownloadError(e.to_string()))?;

        while let Some(chunk) = stream.chunk().await? {
            file.write_all(&chunk)
                .map_err(|e| VideoError::DownloadError(e.to_string()))?;
        }

        Ok(())
    }

    /// Get video URL
    pub fn get_video_url(&self) -> String {
        format!("{}{}", BASE_URL, &self.video_id)
    }

    /// Get video id
    pub fn get_video_id(&self) -> String {
        self.video_id.clone()
    }

    // Necessary to blocking api
    #[allow(dead_code)]
    pub(crate) fn get_client(&self) -> &reqwest_middleware::ClientWithMiddleware {
        &self.client
    }

    // Necessary to blocking api
    #[allow(dead_code)]
    pub(crate) fn get_options(&self) -> VideoOptions {
        self.options.clone()
    }
}

async fn get_m3u8(
    url: &str,
    client: &reqwest_middleware::ClientWithMiddleware,
) -> Result<Vec<(String, String)>, VideoError> {
    let base_url = Url::parse(BASE_URL).expect("BASE_URL corrapt");
    let base_url_host = base_url.host_str().expect("BASE_URL host corrapt");

    let url = Url::parse(url)
        .and_then(|mut x| {
            let set_host_result = x.set_host(Some(base_url_host));
            if set_host_result.is_err() {
                return Err(set_host_result.expect_err("How can be posible"));
            }
            Ok(x)
        })
        .map(|x| x.as_str().to_string())
        .unwrap_or("".to_string());

    let body = get_html(client, &url, None).await?;

    let http_regex = regex::Regex::new(r"^https?://").unwrap();
    let itag_regex = regex::Regex::new(r"/itag/(\d+)/").unwrap();

    let itag_and_url = body
        .split('\n')
        .filter(|x| http_regex.is_match(x) && itag_regex.is_match(x));

    let itag_and_url: Vec<(String, String)> = itag_and_url
        .map(|line| {
            let itag = itag_regex
                .captures(line)
                .expect("IMPOSSIBLE")
                .get(1)
                .map(|x| x.as_str())
                .unwrap_or("");

            // println!("itag: {}, url: {}", itag, line);
            (itag.to_string(), line.to_string())
        })
        .collect();

    Ok(itag_and_url)
}
