use once_cell::sync::Lazy;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, COOKIE},
    Client,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use scraper::{Html, Selector};
use serde_json::json;
use std::{borrow::{Borrow, Cow}, path::Path, time::Duration};
use url::Url;

#[cfg(feature = "live")]
use crate::stream::{LiveStream, LiveStreamOptions};
#[cfg(feature = "ffmpeg")]
use crate::structs::FFmpegArgs;

use crate::{
    constants::{BASE_URL, DEFAULT_DL_CHUNK_SIZE, DEFAULT_MAX_RETRIES, INNERTUBE_CLIENT},
    info_extras::{get_media, get_related_videos},
    stream::{NonLiveStream, NonLiveStreamOptions, Stream},
    structs::{
        CustomRetryableStrategy, PlayerResponse, VideoError, VideoInfo, VideoOptions, YTConfig,
    },
    utils::{
        between, choose_format, clean_video_details, get_functions, get_html, get_html5player, get_random_v6_ip, get_video_id, get_visitor_data, get_ytconfig, is_age_restricted_from_html, is_live, is_not_yet_broadcasted, is_play_error, is_player_response_error, is_private_video, is_rental, parse_live_video_formats, parse_video_formats, sort_formats
    },
};

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display("Video({video_id})")]
#[derivative(Debug, PartialEq, Eq)]
/// If a video was created with a reference to options, it is tied to their lifetime `'opts`.
pub struct Video<'opts> {
    video_id: String,
    options: Cow<'opts, VideoOptions>,
    #[derivative(PartialEq = "ignore")]
    client: ClientWithMiddleware,
}

impl Video<'static> {
    /// Crate [`Video`] struct to get info or download with default [`VideoOptions`]
    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        let video_id = get_video_id(&url_or_id.into()).ok_or(VideoError::VideoNotFound)?;

        let client = Client::builder().build().map_err(VideoError::Reqwest)?;

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(1000), Duration::from_millis(30000))
            .build_with_max_retries(DEFAULT_MAX_RETRIES);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy_and_strategy(
                retry_policy,
                CustomRetryableStrategy,
            ))
            .build();

        Ok(Self {
            video_id,
            options: Cow::Owned(VideoOptions::default()),
            client,
        })
    }
}

impl<'opts> Video<'opts> {
    /// Crate [`Video`] struct to get info or download with custom [`VideoOptions`]
    /// `VideoOptions` can be passed by value or by reference, if passed by
    /// reference, returned `Video` will be tied to the lifetime of the `VideoOptions`.
    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: impl Into<Cow<'opts, VideoOptions>>,
    ) -> Result<Self, VideoError> {
        let options = options.into();
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

        let max_retries = options
            .request_options
            .max_retries
            .unwrap_or(DEFAULT_MAX_RETRIES);

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(1000), Duration::from_millis(30000))
            .build_with_max_retries(max_retries);
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy_and_strategy(
                retry_policy,
                CustomRetryableStrategy,
            ))
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

        let (mut player_response, initial_response): (PlayerResponse, serde_json::Value) = {
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
            .unwrap_or_default();

            let initial_response =
                serde_json::from_str::<serde_json::Value>(initial_response_string.trim())
                    .unwrap_or_default();

            (player_response, initial_response)
        };

        if is_play_error(&player_response, ["ERROR"].to_vec()) {
            return Err(VideoError::VideoNotFound);
        }

        if let Some(reason) = is_player_response_error(&player_response, &["not a bot"]) {
            return Err(VideoError::VideoPlayerResponseError(reason));
        }

        let is_age_restricted = is_age_restricted_from_html(&player_response, &response);

        if is_private_video(&player_response) && !is_age_restricted {
            return Err(VideoError::VideoIsPrivate);
        }

        // POToken experiment detected fallback to ios client (Webpage contains broken formats)
        if !is_live(&player_response) {
            let ios_ytconfig = self
                .get_player_ytconfig(
                    &response,
                    INNERTUBE_CLIENT.get("ios").cloned().unwrap_or_default(),
                    self.options.request_options.po_token.as_ref()
                )
                .await?;

            let player_response_new =
                serde_json::from_str::<PlayerResponse>(&ios_ytconfig).unwrap_or_default();

            player_response.streaming_data = player_response_new.streaming_data;
        }

        if is_age_restricted {
            let embed_ytconfig = self
                .get_player_ytconfig(
                    &response,
                    INNERTUBE_CLIENT
                        .get("tv_embedded")
                        .cloned()
                        .unwrap_or_default(),
                    self.options.request_options.po_token.as_ref()
                )
                .await?;

            let player_response_new =
                serde_json::from_str::<PlayerResponse>(&embed_ytconfig).unwrap_or_default();

            player_response.streaming_data = player_response_new.streaming_data;
            player_response.storyboards = player_response_new.storyboards;
        }

        if is_rental(&player_response) || is_not_yet_broadcasted(&player_response) {
            return Err(VideoError::VideoSourceNotFound);
        }

        let video_details = clean_video_details(
            &initial_response,
            &player_response,
            get_media(&initial_response).unwrap_or_default(),
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
                    get_functions(
                        get_html5player(response.as_str()).unwrap_or_default(),
                        client,
                    )
                    .await?,
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
    pub(crate) fn get_options(&self) -> &VideoOptions {
        &self.options
    }

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    async fn get_player_ytconfig(
        &self,
        html: &str,
        configs: (&str, &str, &str),
        po_token: Option<&String>,
    ) -> Result<String, VideoError> {
        use std::str::FromStr;

        let ytcfg = get_ytconfig(html)?;

        let client = configs.2;
        let sts = ytcfg.sts.unwrap_or(0);
        let video_id = self.get_video_id();

        let visitor_data = get_visitor_data(&html)?;

        let mut query = serde_json::from_str::<serde_json::Value>(&format!(
            r#"{{
            {client}
            "playbackContext": {{
                "contentPlaybackContext": {{
                    "signatureTimestamp": {sts},
                    "html5Preference": "HTML5_PREF_WANTS"
                }}
            }},
            "videoId": "{video_id}"
        }}"#
        ))
        .unwrap_or_default();
        if let Some(po_token) = po_token {
            query
                .as_object_mut()
                .expect("Declared as object above")
                .insert(
                    "serviceIntegrityDimensions".to_string(),
                    json!({"poToken": po_token})
                );
        }

        static CONFIGS: Lazy<(HeaderMap, &str)> = Lazy::new(|| {
            (HeaderMap::from_iter([
            (HeaderName::from_str("content-type").unwrap(), HeaderValue::from_str("application/json").unwrap()),
            (HeaderName::from_str("Origin").unwrap(), HeaderValue::from_str("https://www.youtube.com").unwrap()),
            (HeaderName::from_str("User-Agent").unwrap(), HeaderValue::from_str("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3513.0 Safari/537.36").unwrap()),
            (HeaderName::from_str("Referer").unwrap(), HeaderValue::from_str("https://www.youtube.com/").unwrap()),
            (HeaderName::from_str("Accept").unwrap(), HeaderValue::from_str("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8").unwrap()),
            (HeaderName::from_str("Accept-Language").unwrap(), HeaderValue::from_str("en-US,en;q=0.5").unwrap()),
            (HeaderName::from_str("Accept-Encoding").unwrap(), HeaderValue::from_str("gzip, deflate").unwrap()),
        ]),"AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8")
        });

        let mut headers = CONFIGS.0.clone();
        headers.insert(
            HeaderName::from_str("X-Youtube-Client-Version").unwrap(),
            HeaderValue::from_str(configs.0).unwrap(),
        );
        headers.insert(
            HeaderName::from_str("X-Youtube-Client-Name").unwrap(),
            HeaderValue::from_str(configs.1).unwrap(),
        );
        headers.insert(
            HeaderName::from_str("X-Goog-Visitor-Id").unwrap(),
            HeaderValue::from_str(&visitor_data).unwrap()
        );

        let response = self
            .client
            .post("https://www.youtube.com/youtubei/v1/player")
            .headers(headers)
            .query(&[("key", CONFIGS.1)])
            .json(&query)
            .send()
            .await
            .map_err(VideoError::ReqwestMiddleware)?;

        let response = response
            .error_for_status()
            .map_err(VideoError::Reqwest)?
            .text()
            .await?;

        Ok(response)
    }
}

async fn get_m3u8(
    url: &str,
    client: &reqwest_middleware::ClientWithMiddleware,
) -> Result<Vec<(String, String)>, VideoError> {
    let base_url = Url::parse(BASE_URL)?;
    let base_url_host = base_url.host_str();

    let url = Url::parse(url)
        .and_then(|mut x| {
            x.set_host(base_url_host)?;
            Ok(x)
        })
        .map(|x| x.as_str().to_string())
        .unwrap_or("".to_string());

    let body = get_html(client, &url, None).await?;

    static HTTP_REGEX: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"^https?://").unwrap());
    static ITAG_REGEX: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"/itag/(\d+)/").unwrap());

    let itag_and_url = body
        .split('\n')
        .filter(|x| HTTP_REGEX.is_match(x) && ITAG_REGEX.is_match(x));

    Ok(itag_and_url
        .filter_map(|line| {
            ITAG_REGEX.captures(line).and_then(|caps| {
                caps.get(1)
                    .map(|itag| (itag.as_str().to_string(), line.to_string()))
            })
        })
        .collect::<Vec<(String, String)>>())
}
