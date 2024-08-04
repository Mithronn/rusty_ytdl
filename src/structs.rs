use mime::Mime;
use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter, Result as fmtResult},
    ops::{Bound, RangeBounds},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoInfo {
    #[serde(rename = "dashManifestUrl")]
    pub dash_manifest_url: Option<String>,
    #[serde(rename = "hlsManifestUrl")]
    pub hls_manifest_url: Option<String>,
    pub formats: Vec<VideoFormat>,
    #[serde(rename = "relatedVideos")]
    pub related_videos: Vec<RelatedVideo>,
    #[serde(rename = "videoDetails")]
    pub video_details: VideoDetails,
}

#[derive(Clone, derive_more::Display)]
pub enum VideoSearchOptions {
    /// Video & Audio
    #[display(fmt = "Video & Audio")]
    VideoAudio,
    /// Only Video
    #[display(fmt = "Video")]
    Video,
    /// Only Audio
    #[display(fmt = "Audio")]
    Audio,
    /// Custom filter
    #[display(fmt = "Custom")]
    Custom(Arc<dyn Fn(&VideoFormat) -> bool + Sync + Send + 'static>),
}

impl Debug for VideoSearchOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        match self {
            VideoSearchOptions::VideoAudio => write!(f, "VideoAudio"),
            VideoSearchOptions::Video => write!(f, "Video"),
            VideoSearchOptions::Audio => write!(f, "Audio"),
            VideoSearchOptions::Custom(_) => write!(f, "Custom"),
        }
    }
}

impl PartialEq for VideoSearchOptions {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VideoSearchOptions::VideoAudio, VideoSearchOptions::VideoAudio) => true,
            (VideoSearchOptions::Video, VideoSearchOptions::Video) => true,
            (VideoSearchOptions::Audio, VideoSearchOptions::Audio) => true,
            (VideoSearchOptions::Custom(a), VideoSearchOptions::Custom(b)) => {
                // Compare the function pointer
                Arc::ptr_eq(a, b)
            }
            _ => false,
        }
    }
}

type CustomVideoQualityComparator =
    Arc<dyn Fn(&VideoFormat, &VideoFormat) -> Ordering + Sync + Send + 'static>;

#[derive(Clone, derive_more::Display)]
pub enum VideoQuality {
    /// Highest Video & Audio
    #[display(fmt = "Highest")]
    Highest,
    /// Lowest Video & Audio
    #[display(fmt = "Lowest")]
    Lowest,
    /// Only Highest Audio
    #[display(fmt = "Highest Audio")]
    HighestAudio,
    /// Only Lowest Audio
    #[display(fmt = "Lowest Audio")]
    LowestAudio,
    /// Only Highest Video
    #[display(fmt = "Highest Video")]
    HighestVideo,
    /// Only Lowest Video
    #[display(fmt = "Lowest Video")]
    LowestVideo,
    /// Custom ranking function and filter
    #[display(fmt = "Custom")]
    Custom(VideoSearchOptions, CustomVideoQualityComparator),
}

impl Debug for VideoQuality {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        match self {
            VideoQuality::Highest => write!(f, "Highest"),
            VideoQuality::Lowest => write!(f, "Lowest"),
            VideoQuality::HighestAudio => write!(f, "HighestAudio"),
            VideoQuality::LowestAudio => write!(f, "LowestAudio"),
            VideoQuality::HighestVideo => write!(f, "HighestVideo"),
            VideoQuality::LowestVideo => write!(f, "LowestVideo"),
            VideoQuality::Custom(filter, _) => write!(f, "Custom({filter:?})"),
        }
    }
}

impl PartialEq for VideoQuality {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VideoQuality::Highest, VideoQuality::Highest) => true,
            (VideoQuality::Lowest, VideoQuality::Lowest) => true,
            (VideoQuality::HighestAudio, VideoQuality::HighestAudio) => true,
            (VideoQuality::LowestAudio, VideoQuality::LowestAudio) => true,
            (VideoQuality::HighestVideo, VideoQuality::HighestVideo) => true,
            (VideoQuality::LowestVideo, VideoQuality::LowestVideo) => true,
            (VideoQuality::Custom(i, a), VideoQuality::Custom(j, b)) => {
                // Compare the function pointer
                Arc::ptr_eq(a, b) && i == j
            }
            _ => false,
        }
    }
}

/// Video search and download options
#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "VideoOptions(quality: {quality}, filter: {filter})")]
#[derivative(Debug, PartialEq, Eq)]
pub struct VideoOptions {
    pub quality: VideoQuality,
    pub filter: VideoSearchOptions,
    pub download_options: DownloadOptions,
    #[derivative(PartialEq = "ignore")]
    pub request_options: RequestOptions,
}

impl Default for VideoOptions {
    fn default() -> Self {
        VideoOptions {
            quality: VideoQuality::Highest,
            filter: VideoSearchOptions::Audio,
            download_options: DownloadOptions::default(),
            request_options: RequestOptions::default(),
        }
    }
}

/// Video download options
#[derive(Clone, PartialEq, Debug, Default, derive_more::Display)]
#[display(fmt = "DownloadOptions()")]
pub struct DownloadOptions {
    /// Maximum chunk size on per request
    pub dl_chunk_size: Option<u64>,
}

#[derive(Clone, Debug, Default, derive_more::Display)]
#[display(fmt = "RequestOptions()")]
pub struct RequestOptions {
    /// [`reqwest::Client`] to on use request. If provided in the request options `proxy`, `cookies`, and `ipv6_block` will be ignored
    ///
    /// # Example
    ///
    /// ```ignore
    ///     let video_options = VideoOptions {
    ///         request_options: RequestOptions {
    ///              client: Some(
    ///                  reqwest::Client::builder()
    ///                  .build()
    ///                  .unwrap(),
    ///              ),
    ///              ..Default::default()
    ///         },
    ///         ..Default::default()
    ///     };
    /// ```
    pub client: Option<reqwest::Client>,
    /// [`reqwest::Proxy`] to on use request
    ///
    /// # Example
    /// ```ignore
    ///     let video_options = VideoOptions {
    ///         request_options: RequestOptions {
    ///              proxy: Some(
    ///                   reqwest::Proxy::http("https://my.prox")
    ///                   .unwrap()
    ///                   .basic_auth("a", "b"),
    ///              ),
    ///              ..Default::default()
    ///         },
    ///         ..Default::default()
    ///     };
    /// ```
    pub proxy: Option<reqwest::Proxy>,
    /// Cookies String
    ///
    /// # Example
    /// ```ignore
    /// Some("key1=value1; key2=value2; key3=value3".to_string())
    /// ```
    pub cookies: Option<String>,
    /// Custom IPv6 String
    ///
    /// # Example
    /// ```ignore
    ///     // Custom IPv6 block
    ///     let ipv6_block = "2001:4::/48".to_string();

    ///     let video_options = VideoOptions {
    ///          request_options: RequestOptions {
    ///               ipv6_block: Some(ipv6_block),
    ///                ..Default::default()
    ///          },
    ///          ..Default::default()
    ///     };
    /// ```
    pub ipv6_block: Option<String>,
    /// Override the default number of retries to allow per web request (ie, per chunk downloaded)
    /// Default is [`crate::constants::DEFAULT_MAX_RETRIES`].
    ///
    /// # Example
    /// ```ignore
    ///     // Allow 5 retries per chunk.
    ///     let video_options = VideoOptions {
    ///          request_options: RequestOptions {
    ///               override_max_retries: Some(5),
    ///                ..Default::default()
    ///          },
    ///          ..Default::default()
    ///     };
    /// ```
    pub max_retries: Option<u32>,
}

#[derive(thiserror::Error, Debug)]
pub enum VideoError {
    /// The video not found
    #[error("The video not found")]
    VideoNotFound,
    /// Video source empty
    #[error("Video source empty")]
    VideoSourceNotFound,
    /// Video is private
    #[error("Video is private")]
    VideoIsPrivate,
    /// Reqwest error
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    /// ReqwestMiddleware error
    #[error(transparent)]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),
    /// URL cannot parsed
    #[error(transparent)]
    URLParseError(#[from] url::ParseError),
    /// Body cannot parsed
    #[error("Body cannot parsed")]
    BodyCannotParsed,
    /// Format not found
    #[error("Format not found")]
    FormatNotFound,
    /// Invalid IPv6 format
    #[error("Invalid IPv6 format")]
    InvalidIPv6Format,
    /// Invalid IPv6 subnet
    #[error("Invalid IPv6 subnet")]
    InvalidIPv6Subnet,
    /// M3U8 parse error
    #[error("M3U8 Parse Error: {0}")]
    M3U8ParseError(String),
    /// URL is not playlist
    #[error("{0} is not a playlist URL")]
    IsNotPlaylist(String),
    /// Playlist body cannot parsed
    #[error("Playlist body cannot parsed")]
    PlaylistBodyCannotParsed,
    /// Download error
    #[error("Download Error: {0}")]
    DownloadError(String),
    /// Encryption error
    #[error("Encryption Error: {0}")]
    EncryptionError(String),
    /// Decryption error
    #[error("Decryption Error: {0}")]
    DecryptionError(String),
    /// Hex encdode error
    #[error(transparent)]
    HexError(#[from] hex::FromHexError),
    /// Child process error
    #[error("Process Error: {0}")]
    ChildProcessError(String),
    /// Downloading live streams not supported, compile with `live` feature to enable
    #[error("Downloading live streams not supported, compile with `live` feature to enable")]
    LiveStreamNotSupported,
    /// Provided cookie contains invalid header value characters, an error is returned. Only visible ASCII characters (32-127) are permitted.
    #[error("Provided cookie contains invalid header value characters, an error is returned. Only visible ASCII characters (32-127) are permitted")]
    CookieError,
    /// FFmpeg command error
    #[error("FFmpeg command error: {0}")]
    #[cfg(feature = "ffmpeg")]
    FFmpeg(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFormat {
    /// Video format itag number
    pub itag: u64,
    /// Video format mime type
    #[serde(rename = "mimeType")]
    pub mime_type: MimeType,
    pub bitrate: u64,
    /// Video format width
    pub width: Option<u64>, // VIDEO & DASH MPD ONLY
    /// Video format height
    pub height: Option<u64>, // VIDEO & DASH MPD ONLY
    #[serde(rename = "initRange")]
    pub init_range: Option<RangeObject>,
    #[serde(rename = "indexRange")]
    pub index_range: Option<RangeObject>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "contentLength")]
    pub content_length: Option<String>,
    pub quality: Option<String>,
    pub fps: Option<u64>, // VIDEO & DASH MPD ONLY
    #[serde(rename = "qualityLabel")]
    pub quality_label: Option<String>,
    #[serde(rename = "projectionType")]
    pub projection_type: Option<String>,
    #[serde(rename = "averageBitrate")]
    pub average_bitrate: Option<u64>,
    #[serde(rename = "highReplication")]
    pub high_replication: Option<bool>, // AUDIO ONLY
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>, // AUDIO ONLY
    #[serde(rename = "colorInfo")]
    pub color_info: Option<ColorInfo>, // VIDEO ONLY
    #[serde(rename = "approxDurationMs")]
    pub approx_duration_ms: Option<String>,
    #[serde(rename = "audioSampleRate")]
    pub audio_sample_rate: Option<String>, // AUDIO & DASH MPD ONLY
    #[serde(rename = "audioChannels")]
    pub audio_channels: Option<u8>, // AUDIO ONLY
    #[serde(rename = "audioBitrate")]
    pub audio_bitrate: Option<u64>, // LIVE HLS VIDEO ONLY
    #[serde(rename = "loudnessDb")]
    pub loudness_db: Option<f64>, // AUDIO ONLY
    /// Video format URL
    pub url: String,
    /// Video format has video or not
    #[serde(rename = "hasVideo")]
    pub has_video: bool,
    /// Video format has audio or not
    #[serde(rename = "hasAudio")]
    pub has_audio: bool,
    /// Video is live or not
    #[serde(rename = "isLive")]
    pub is_live: bool,
    /// Video format is HLS or not
    #[serde(rename = "isHLS")]
    pub is_hls: bool,
    /// Video format is DashMPD or not
    #[serde(rename = "isDashMPD")]
    pub is_dash_mpd: bool,
}

impl From<StreamingDataFormat> for VideoFormat {
    fn from(value: StreamingDataFormat) -> Self {
        Self {
            itag: value.itag.unwrap_or_default(),
            mime_type: value.mime_type.clone().unwrap(),
            bitrate: value.bitrate.unwrap_or_default(),
            width: value.width,
            height: value.height,
            init_range: value.init_range.clone(),
            index_range: value.index_range.clone(),
            last_modified: value.last_modified.clone(),
            content_length: value.content_length.clone(),
            quality: value.quality.clone(),
            fps: value.fps,
            quality_label: value.quality_label.clone(),
            projection_type: value.projection_type.clone(),
            average_bitrate: value.average_bitrate,
            high_replication: value.high_replication,
            audio_quality: value.audio_quality.clone(),
            color_info: value.color_info.as_ref().map(|x| ColorInfo {
                primaries: x.primaries.clone().unwrap_or_default(),
                transfer_characteristics: x.transfer_characteristics.clone().unwrap_or_default(),
                matrix_coefficients: x.matrix_coefficients.clone().unwrap_or_default(),
            }),
            approx_duration_ms: value.approx_duration_ms.clone(),
            audio_sample_rate: value.audio_sample_rate.clone(),
            audio_channels: value.audio_channels,
            audio_bitrate: value.audio_bitrate,
            loudness_db: value.loudness_db,
            url: value.url.clone().unwrap_or_default(),
            has_video: false,
            has_audio: false,
            is_live: false,
            is_hls: false,
            is_dash_mpd: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeObject {
    #[serde(rename = "start")]
    pub start: Option<String>,
    #[serde(rename = "end")]
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorInfo {
    pub primaries: String,
    #[serde(rename = "transferCharacteristics")]
    pub transfer_characteristics: String,
    #[serde(rename = "matrixCoefficients")]
    pub matrix_coefficients: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoDetails {
    pub author: Option<Author>,
    pub likes: u64,
    pub dislikes: u64,
    #[serde(rename = "ageRestricted")]
    pub age_restricted: bool,
    #[serde(rename = "videoUrl")]
    pub video_url: String,
    pub storyboards: Vec<StoryBoard>,
    pub chapters: Vec<Chapter>,
    pub embed: Embed,
    pub title: String,
    pub description: String,
    #[serde(rename = "lengthSeconds")]
    pub length_seconds: String,
    #[serde(rename = "ownerProfileUrl")]
    pub owner_profile_url: String,
    #[serde(rename = "externalChannelId")]
    pub external_channel_id: String,
    #[serde(rename = "isFamilySafe")]
    pub is_family_safe: bool,
    #[serde(rename = "availableCountries")]
    pub available_countries: Vec<String>,
    #[serde(rename = "isUnlisted")]
    pub is_unlisted: bool,
    #[serde(rename = "hasYpcMetadata")]
    pub has_ypc_metadata: bool,
    #[serde(rename = "viewCount")]
    pub view_count: String,
    pub category: String,
    #[serde(rename = "publishDate")]
    pub publish_date: String,
    #[serde(rename = "ownerChannelName")]
    pub owner_channel_name: String,
    #[serde(rename = "uploadDate")]
    pub upload_date: String,
    #[serde(rename = "videoId")]
    pub video_id: String,
    pub keywords: Vec<String>,
    pub channel_id: String,
    #[serde(rename = "isOwnerViewing")]
    pub is_owner_viewing: bool,
    #[serde(rename = "isCrawlable")]
    pub is_crawlable: bool,
    #[serde(rename = "allowRatings")]
    pub allow_ratings: bool,
    #[serde(rename = "isPrivate")]
    pub is_private: bool,
    #[serde(rename = "isUnpluggedCropus")]
    pub is_unplugged_corpus: bool,
    #[serde(rename = "isLiveContent")]
    pub is_live_content: bool,
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedVideo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub published: String,
    pub author: Option<Author>,
    #[serde(rename = "shortViewCountText")]
    pub short_view_count_text: String,
    #[serde(rename = "viewCount")]
    pub view_count: String,
    #[serde(rename = "lengthSeconds")]
    pub length_seconds: String,
    pub thumbnails: Vec<Thumbnail>,
    pub is_live: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub name: String,
    pub user: String,
    #[serde(rename = "channelUrl")]
    pub channel_url: String,
    #[serde(rename = "externalChannelUrl")]
    pub external_channel_url: String,
    #[serde(rename = "userUrl")]
    pub user_url: String,
    pub thumbnails: Vec<Thumbnail>,
    pub verified: bool,
    #[serde(rename = "subscriberCount")]
    pub subscriber_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    #[serde(rename = "startTime")]
    pub start_time: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryBoard {
    #[serde(rename = "templateUrl")]
    pub template_url: String,
    #[serde(rename = "thumbnailWidth")]
    pub thumbnail_width: i32,
    #[serde(rename = "thumbnailHeight")]
    pub thumbnail_height: i32,
    #[serde(rename = "thumbnailCount")]
    pub thumbnail_count: i32,
    pub interval: i32,
    pub columns: i32,
    pub rows: i32,
    #[serde(rename = "storyboardCount")]
    pub storyboard_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thumbnail {
    pub width: u64,
    pub height: u64,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Embed {
    #[serde(rename = "flashSecureUrl")]
    pub flash_secure_url: String,
    #[serde(rename = "flashUrl")]
    pub flash_url: String,
    #[serde(rename = "iframeUrl")]
    pub iframe_url: String,
    pub height: i32,
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticFormat {
    #[serde(rename = "mimeType")]
    pub mime_type: MimeType,
    #[serde(rename = "qualityLabel")]
    pub quality_label: Option<String>,
    pub bitrate: Option<u64>,
    #[serde(rename = "audioBitrate")]
    pub audio_bitrate: Option<u64>,
}

pub trait StringUtils {
    fn substring(&self, start: usize, end: usize) -> &str;
    fn substr(&self, start: usize, len: usize) -> &str;
    fn slice(&self, range: impl RangeBounds<usize>) -> &str;
}

impl StringUtils for str {
    fn substring(&self, start: usize, end: usize) -> &str {
        let (start, end) = if end >= start {
            (start, end)
        } else {
            (end, start)
        };
        let mut char_pos = 0;
        let mut byte_start = 0;
        let mut it = self.chars();

        loop {
            if char_pos == start {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_start += c.len_utf8();
            } else {
                break;
            }
        }

        char_pos = 0;

        let mut byte_end = byte_start;
        let end_pos = end - start;

        loop {
            if char_pos == end_pos {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_end += c.len_utf8();
            } else {
                break;
            }
        }

        &self[byte_start..byte_end]
    }

    fn substr(&self, start: usize, len: usize) -> &str {
        let mut char_pos = 0;
        let mut byte_start = 0;
        let mut it = self.chars();
        loop {
            if char_pos == start {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_start += c.len_utf8();
            } else {
                break;
            }
        }
        char_pos = 0;
        let mut byte_end = byte_start;
        loop {
            if char_pos == len {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_end += c.len_utf8();
            } else {
                break;
            }
        }
        &self[byte_start..byte_end]
    }

    fn slice(&self, range: impl RangeBounds<usize>) -> &str {
        let start = match range.start_bound() {
            Bound::Included(bound) | Bound::Excluded(bound) => *bound,
            Bound::Unbounded => 0,
        };
        let len = match range.end_bound() {
            Bound::Included(bound) => *bound + 1,
            Bound::Excluded(bound) => *bound,
            Bound::Unbounded => self.len(),
        } - start;
        self.substr(start, len)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeType {
    pub mime: Mime,
    /// Mime container
    pub container: String,
    /**
     * Mime codec parameters

     **Mime type:** [`mime::AUDIO`] or [`mime::VIDEO`] => contains 1 element and its audio/video codec

     **Mime type:** [`mime::VIDEO`] => if contains 2 element, first is video and second is audio codec
    */
    pub codecs: Vec<String>,
    /// Video codec parameter
    pub video_codec: Option<String>,
    /// Audio codec parameter
    pub audio_codec: Option<String>,
}

impl Serialize for MimeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!(
            r#"{}/{}; codecs="{}""#,
            self.mime.type_(),
            self.mime.subtype(),
            self.codecs.join(", "),
        );

        s.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MimeType {
    fn deserialize<D>(deserializer: D) -> Result<MimeType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        let mime: Mime = Mime::from_str(&s).map_err(|_| {
            D::Error::invalid_value(
                Unexpected::Str(&s),
                &r#"valid mime type format must be `(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)"`"#,
            )
        })?;

        let codecs: Vec<String> = mime
            .get_param("codecs")
            .map(|x| x.as_str().split(", ").map(|x| x.to_string()).collect())
            .unwrap_or_default();

        let container: String = mime.subtype().to_string();

        let video_codec = if mime.type_() == mime::VIDEO {
            codecs.first().cloned()
        } else {
            None
        };

        let audio_codec = if mime.type_() == mime::AUDIO {
            codecs.first().cloned()
        } else {
            codecs.get(1).cloned()
        };

        Ok(MimeType {
            mime,
            container,
            codecs,
            video_codec,
            audio_codec,
        })
    }
}

#[cfg(feature = "ffmpeg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FFmpegArgs {
    pub format: Option<String>,
    pub audio_filter: Option<String>,
    pub video_filter: Option<String>,
}

#[cfg(feature = "ffmpeg")]
impl FFmpegArgs {
    pub fn build(&self) -> Vec<String> {
        let mut args: Vec<String> = vec![];

        if let Some(format) = &self.format {
            args.push("-f".to_string());
            args.push(format.to_string());
        }

        if let Some(audio_filter) = &self.audio_filter {
            args.push("-af".to_string());
            args.push(audio_filter.to_string());
        }

        if let Some(video_filter) = &self.video_filter {
            args.push("-vf".to_string());
            args.push(video_filter.to_string());
        }

        if self.format.is_some() || self.audio_filter.is_some() || self.video_filter.is_some() {
            args = [
                vec![
                    // input as stdin
                    "-i".to_string(),
                    // aliases of pipe:0
                    "-".to_string(),
                    // loggers
                    "-analyzeduration".to_string(),
                    "0".to_string(),
                    "-loglevel".to_string(),
                    "0".to_string(),
                ],
                args,
            ]
            .concat();

            // pipe to stdout
            args.push("pipe:1".to_string());
        }

        args
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PlayerResponse {
    #[serde(rename = "streamingData")]
    pub streaming_data: Option<StreamingData>,
    #[serde(rename = "playabilityStatus")]
    pub playability_status: Option<PlayabilityStatus>,
    #[serde(rename = "microformat")]
    pub micro_format: Option<MicroFormat>,
    #[serde(rename = "videoDetails")]
    pub video_details: Option<PlayerResponseVideoDetails>,
    #[serde(rename = "storyboards")]
    pub storyboards: Option<PlayerResponseStoryboards>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerResponseStoryboards {
    #[serde(rename = "playerStoryboardSpecRenderer")]
    pub player_storyboard_spec_renderer: Option<PlayerResponseStoryboardsSpecRenderer>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerResponseStoryboardsSpecRenderer {
    #[serde(rename = "spec")]
    pub spec: Option<String>,
    #[serde(rename = "recommendedLevel")]
    pub recommended_level: Option<i32>,
    #[serde(rename = "highResolutionRecommendedLevel")]
    pub high_resolution_recommended_level: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicroFormat {
    #[serde(rename = "playerMicroformatRenderer")]
    pub player_micro_format_renderer: Option<PlayerMicroFormatRenderer>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerMicroFormatRenderer {
    pub thumbnail: Option<PlayerMicroFormatRendererThumbnail>,
    pub embed: Option<PlayerMicroFormatRendererEmbed>,
    pub title: Option<PlayerMicroFormatRendererTitle>,
    pub description: Option<PlayerMicroFormatRendererTitle>,
    #[serde(rename = "lengthSeconds")]
    pub length_seconds: Option<String>,
    #[serde(rename = "ownerProfileUrl")]
    pub owner_profile_url: Option<String>,
    #[serde(rename = "externalChannelId")]
    pub external_channel_id: Option<String>,
    #[serde(rename = "isFamilySafe")]
    pub is_family_safe: Option<bool>,
    #[serde(rename = "availableCountries")]
    pub available_countries: Option<Vec<String>>,
    #[serde(rename = "isUnlisted")]
    pub is_unlisted: Option<bool>,
    #[serde(rename = "hasYpcMetadata")]
    pub has_ypc_metadata: Option<bool>,
    #[serde(rename = "viewCount")]
    pub view_count: Option<String>,
    #[serde(rename = "category")]
    pub category: Option<String>,
    #[serde(rename = "publishDate")]
    pub publish_date: Option<String>,
    #[serde(rename = "ownerChannelName")]
    pub owner_channel_name: Option<String>,
    #[serde(rename = "uploadDate")]
    pub upload_date: Option<String>,
    #[serde(rename = "isShortsEligible")]
    pub is_shorts_eligible: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerMicroFormatRendererTitle {
    #[serde(rename = "simpleText")]
    pub simple_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerMicroFormatRendererEmbed {
    #[serde(rename = "flashSecureUrl")]
    pub flash_secure_url: Option<String>,
    #[serde(rename = "flashUrl")]
    pub flash_url: Option<String>,
    #[serde(rename = "iframeUrl")]
    pub iframe_url: Option<String>,
    pub height: Option<i32>,
    pub width: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerMicroFormatRendererThumbnail {
    pub thumbnails: Option<Vec<Thumbnail>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerResponseVideoDetails {
    #[serde(rename = "videoId")]
    pub video_id: Option<String>,
    #[serde(rename = "title")]
    pub title: Option<String>,
    #[serde(rename = "lengthSeconds")]
    pub length_seconds: Option<String>,
    #[serde(rename = "keywords")]
    pub keywords: Option<Vec<String>>,
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(rename = "isOwnerViewing")]
    pub is_owner_viewing: Option<bool>,
    #[serde(rename = "shortDescription")]
    pub short_description: Option<String>,
    #[serde(rename = "isCrawlable")]
    pub is_crawlable: Option<bool>,
    pub thumbnail: Option<PlayerMicroFormatRendererThumbnail>,
    #[serde(rename = "allowRatings")]
    pub allow_ratings: Option<bool>,
    #[serde(rename = "viewCount")]
    pub view_count: Option<String>,
    #[serde(rename = "author")]
    pub author: Option<String>,
    #[serde(rename = "isPrivate")]
    pub is_private: Option<bool>,
    #[serde(rename = "isUnpluggedCorpus")]
    pub is_unplugged_corpus: Option<bool>,
    #[serde(rename = "isLiveContent")]
    pub is_live_content: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamingData {
    #[serde(rename = "dashManifestUrl")]
    pub dash_manifest_url: Option<String>,
    #[serde(rename = "hlsManifestUrl")]
    pub hls_manifest_url: Option<String>,
    #[serde(rename = "formats")]
    pub formats: Option<Vec<StreamingDataFormat>>,
    #[serde(rename = "adaptiveFormats")]
    pub adaptive_formats: Option<Vec<StreamingDataFormat>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StreamingDataFormat {
    /// Video format itag number
    pub itag: Option<u64>,
    /// Video format mime type
    #[serde(rename = "mimeType")]
    pub mime_type: Option<MimeType>,
    pub bitrate: Option<u64>,
    /// Video format width
    pub width: Option<u64>, // VIDEO & DASH MPD ONLY
    /// Video format height
    pub height: Option<u64>, // VIDEO & DASH MPD ONLY
    #[serde(rename = "initRange")]
    pub init_range: Option<RangeObject>,
    #[serde(rename = "indexRange")]
    pub index_range: Option<RangeObject>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "contentLength")]
    pub content_length: Option<String>,
    pub quality: Option<String>,
    pub fps: Option<u64>, // VIDEO & DASH MPD ONLY
    #[serde(rename = "qualityLabel")]
    pub quality_label: Option<String>,
    #[serde(rename = "projectionType")]
    pub projection_type: Option<String>,
    #[serde(rename = "averageBitrate")]
    pub average_bitrate: Option<u64>,
    #[serde(rename = "highReplication")]
    pub high_replication: Option<bool>, // AUDIO ONLY
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>, // AUDIO ONLY
    #[serde(rename = "colorInfo")]
    pub color_info: Option<StreamingDataFormatColorInfo>, // VIDEO ONLY
    #[serde(rename = "approxDurationMs")]
    pub approx_duration_ms: Option<String>,
    #[serde(rename = "audioSampleRate")]
    pub audio_sample_rate: Option<String>, // AUDIO & DASH MPD ONLY
    #[serde(rename = "audioChannels")]
    pub audio_channels: Option<u8>, // AUDIO ONLY
    #[serde(rename = "audioBitrate")]
    pub audio_bitrate: Option<u64>, // LIVE HLS VIDEO ONLY
    #[serde(rename = "loudnessDb")]
    pub loudness_db: Option<f64>, // AUDIO ONLY
    /// Video format URL
    pub url: Option<String>,
    #[serde(rename = "signatureCipher")]
    pub signature_cipher: Option<String>,
    #[serde(rename = "cipher")]
    pub cipher: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamingDataFormatColorInfo {
    pub primaries: Option<String>,
    #[serde(rename = "transferCharacteristics")]
    pub transfer_characteristics: Option<String>,
    #[serde(rename = "matrixCoefficients")]
    pub matrix_coefficients: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayabilityStatus {
    pub status: Option<String>,
    #[serde(rename = "errorScreen")]
    pub error_screen: Option<ErrorScreen>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorScreen {
    #[serde(rename = "playerLegacyDesktopYpcOfferRenderer")]
    pub player_legacy_desktop_ypc_offer_renderer: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YTConfig {
    #[serde(rename = "STS")]
    pub sts: Option<u64>,
}

pub struct CustomRetryableStrategy;

impl reqwest_retry::RetryableStrategy for CustomRetryableStrategy {
    fn handle(
        &self,
        res: &reqwest_middleware::Result<reqwest::Response>,
    ) -> Option<reqwest_retry::Retryable> {
        match res {
            // retry if 201
            Ok(success) => custom_on_request_success(success),
            Err(error) => reqwest_retry::default_on_request_failure(error),
        }
    }
}

/// Custom request success retry strategy.
///
/// Will only retry if:
/// * The status was 5XX (server error)
/// * The status was 4XX (client error)
///
/// Note that success here means that the request finished without interruption, not that it was logically OK.
fn custom_on_request_success(success: &reqwest::Response) -> Option<reqwest_retry::Retryable> {
    let status = success.status();
    if status.is_server_error() || status.is_client_error() {
        Some(reqwest_retry::Retryable::Transient)
    } else if status.is_success() {
        None
    } else {
        Some(reqwest_retry::Retryable::Fatal)
    }
}
