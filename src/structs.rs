use serde::{Deserialize, Serialize};
use std::{
    ops::{Bound, RangeBounds},
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

impl std::fmt::Debug for VideoSearchOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    Custom(
        VideoSearchOptions,
        Arc<dyn Fn(&VideoFormat, &VideoFormat) -> std::cmp::Ordering + Sync + Send + 'static>,
    ),
}

impl std::fmt::Debug for VideoQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFormat {
    /// Video format itag number
    pub itag: u64,
    /// Video format mime type
    #[serde(rename = "mimeType")]
    pub mime_type: String,
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
    pub container: Option<String>,
    pub codecs: Option<String>,
    #[serde(rename = "videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "audioCodec")]
    pub audio_codec: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeObject {
    pub start: Option<String>,
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
    pub likes: i32,
    pub dislikes: i32,
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
    pub subscriber_count: i32,
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

#[derive(Debug, Clone)]
pub(crate) struct EscapeSequence {
    pub start: String,
    pub end: String,
    pub start_prefix: Option<regex::Regex>,
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
