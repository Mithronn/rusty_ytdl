use scraper::{Html, Selector};

use crate::constants::{BASE_URL, FORMATS};
use crate::info_extras::{get_media, get_related_videos};
#[cfg(feature = "live")]
use crate::stream::{LiveStream, LiveStreamOptions};
use crate::stream::{NonLiveStream, NonLiveStreamOptions, Stream};

use crate::structs::{VideoError, VideoFormat, VideoInfo, VideoOptions};

#[cfg(feature = "ffmpeg")]
use crate::structs::FFmpegArgs;

use crate::utils::{
    add_format_meta, between, choose_format, clean_video_details, get_functions, get_html,
    get_html5player, get_random_v6_ip, get_video_id, is_not_yet_broadcasted, is_play_error,
    is_private_video, is_rental, parse_video_formats, sort_formats,
};

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "Video({video_id})")]
#[derivative(Debug, PartialEq, Eq)]
pub struct Video {
    video_id: String,
    options: VideoOptions,
    #[derivative(PartialEq = "ignore")]
    client: reqwest_middleware::ClientWithMiddleware,
}

impl Video {
    /// Crate [`Video`] struct to get info or download with default [`VideoOptions`]
    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        let video_id = get_video_id(&url_or_id.into()).ok_or(VideoError::VideoNotFound)?;

        let client = reqwest::Client::builder()
            .build()
            .map_err(VideoError::Reqwest)?;

        let retry_policy = reqwest_retry::policies::ExponentialBackoff::builder()
            .retry_bounds(
                std::time::Duration::from_millis(500),
                std::time::Duration::from_millis(10000),
            )
            .build_with_max_retries(3);
        let client = reqwest_middleware::ClientBuilder::new(client)
            .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
                retry_policy,
            ))
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

        let client = if let Some(client) = options.request_options.client.as_ref() {
            client.clone()
        } else {
            let mut client = reqwest::Client::builder();

            if let Some(proxy) = options.request_options.proxy.as_ref() {
                client = client.proxy(proxy.clone());
            }

            if let Some(ipv6_block) = options.request_options.ipv6_block.as_ref() {
                let ipv6 = get_random_v6_ip(ipv6_block)?;
                client = client.local_address(ipv6);
            }

            if let Some(cookie) = options.request_options.cookies.as_ref() {
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::COOKIE,
                    reqwest::header::HeaderValue::from_str(cookie)
                        .map_err(|_x| VideoError::CookieError)?,
                );

                client = client.default_headers(headers)
            }

            client.build().map_err(VideoError::Reqwest)?
        };

        let retry_policy = reqwest_retry::policies::ExponentialBackoff::builder()
            .retry_bounds(
                std::time::Duration::from_millis(500),
                std::time::Duration::from_millis(10000),
            )
            .build_with_max_retries(3);
        let client = reqwest_middleware::ClientBuilder::new(client)
            .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
                retry_policy,
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

        let url_parsed =
            url::Url::parse_with_params(self.get_video_url().as_str(), &[("hl", "en")])
                .map_err(VideoError::URLParseError)?;

        let response = get_html(client, url_parsed.as_str(), None).await?;

        let (player_response, initial_response): (serde_json::Value, serde_json::Value) = {
            let document = Html::parse_document(&response);
            let scripts_selector = Selector::parse("script").unwrap();
            let player_response_string = document
                .select(&scripts_selector)
                .filter(|x| x.inner_html().contains("var ytInitialPlayerResponse ="))
                .map(|x| x.inner_html().replace("var ytInitialPlayerResponse =", ""))
                .next()
                .unwrap_or(String::from(""))
                .trim()
                .to_string();
            let mut initial_response_string = document
                .select(&scripts_selector)
                .filter(|x| x.inner_html().contains("var ytInitialData ="))
                .map(|x| x.inner_html().replace("var ytInitialData =", ""))
                .next()
                .unwrap_or(String::from(""))
                .trim()
                .to_string();

            // remove json object last element (;)
            initial_response_string.pop();

            let player_response: serde_json::Value = serde_json::from_str(
                format!(
                    "{{{}}}}}}}",
                    between(player_response_string.as_str(), "{", "}}};")
                )
                .as_str(),
            )
            .unwrap();
            let initial_response: serde_json::Value =
                serde_json::from_str(&initial_response_string).unwrap();

            (player_response, initial_response)
        };

        if is_play_error(&player_response, ["ERROR"].to_vec()) {
            return Err(VideoError::VideoNotFound);
        }

        if is_private_video(&player_response) {
            return Err(VideoError::VideoIsPrivate);
        }

        if player_response.get("streamingData").is_none()
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
            .get("streamingData")
            .and_then(|x| x.get("dashManifestUrl"))
            .and_then(|x| x.as_str())
            .map(|x| x.to_string());

        let hls_manifest_url = player_response
            .get("streamingData")
            .and_then(|x| x.get("hlsManifestUrl"))
            .and_then(|x| x.as_str())
            .map(|x| x.to_string());

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
        let client = &self.client;

        let mut info = self.get_basic_info().await?;

        let has_manifest = info.dash_manifest_url.is_some() || info.hls_manifest_url.is_some();

        // if has_manifest && info.dash_manifest_url.is_some() {}

        if has_manifest && info.hls_manifest_url.is_some() {
            let url = info.hls_manifest_url.as_ref().expect("IMPOSSIBLE");
            let unformated_formats = get_m3u8(url, client).await;

            // Skip if error occured
            if let Ok(unformated_formats) = unformated_formats {
                let default_formats = FORMATS.as_object().expect("IMPOSSIBLE");
                // Push formated infos to formats
                for (itag, url) in unformated_formats {
                    let static_format = default_formats.get(&itag);
                    if static_format.is_none() {
                        continue;
                    }
                    let static_format = static_format.unwrap();

                    let mut format = serde_json::json!({
                        "itag": itag.parse::<i32>().unwrap_or(0),
                        "mimeType": static_format.get("mimeType").expect("IMPOSSIBLE"),
                    });

                    let format_as_object_mut = format.as_object_mut().unwrap();

                    if !static_format.get("qualityLabel").unwrap().is_null() {
                        format_as_object_mut.insert(
                            "qualityLabel".to_string(),
                            static_format.get("qualityLabel").unwrap().clone(),
                        );
                    }

                    if !static_format.get("bitrate").unwrap().is_null() {
                        format_as_object_mut.insert(
                            "bitrate".to_string(),
                            static_format.get("bitrate").unwrap().clone(),
                        );
                    }

                    if !static_format.get("audioBitrate").unwrap().is_null() {
                        format_as_object_mut.insert(
                            "audioBitrate".to_string(),
                            static_format.get("audioBitrate").unwrap().clone(),
                        );
                    }

                    // Insert stream url to format map
                    format_as_object_mut.insert("url".to_string(), serde_json::Value::String(url));

                    // Add other metadatas to format map
                    add_format_meta(format_as_object_mut);

                    let format: Result<VideoFormat, serde_json::Error> =
                        serde_json::from_value(format);
                    if format.is_err() {
                        continue;
                    }
                    info.formats.push(format.unwrap());
                }
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
            // 1024 * 1024 * 10_u64 -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
            .unwrap_or(1024 * 1024 * 10_u64);

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
            // 1024 * 1024 * 10_u64 -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
            .unwrap_or(1024 * 1024 * 10_u64);

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
    pub async fn download<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), VideoError> {
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
    pub async fn download_with_ffmpeg<P: AsRef<std::path::Path>>(
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
    let base_url = url::Url::parse(BASE_URL).expect("BASE_URL corrapt");
    let base_url_host = base_url.host_str().expect("BASE_URL host corrapt");

    let url = url::Url::parse(url)
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
