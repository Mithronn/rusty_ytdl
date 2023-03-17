use serde::{Deserialize, Serialize};
use std::ops::{Bound, RangeBounds};

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoInfo {
    pub player_response: serde_json::Value,
    pub initial_response: serde_json::Value,
    pub html5player: String,
    pub formats: Vec<serde_json::Value>,
    pub related_videos: Vec<serde_json::Value>,
    pub video_details: VideoDetails,
}

#[derive(Clone, PartialEq, Debug, derive_more::Display)]
pub enum VideoSearchOptions {
    #[display(fmt = "Video & Audio")]
    VideoAuido,
    #[display(fmt = "Video")]
    Video,
    #[display(fmt = "Audio")]
    Audio,
}

#[derive(Clone, PartialEq, Debug, derive_more::Display)]
pub enum VideoQuality {
    #[display(fmt = "Highest")]
    Highest,
    #[display(fmt = "Lowest")]
    Lowest,
    #[display(fmt = "Highest Audio")]
    HighestAudio,
    #[display(fmt = "Lowest Audio")]
    LowestAudio,
    #[display(fmt = "Highest Video")]
    HighestVideo,
    #[display(fmt = "Lowest Video")]
    LowestVideo,
}

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

#[derive(Clone, PartialEq, Debug, derive_more::Display)]
#[display(fmt = "DownloadOptions()")]
pub struct DownloadOptions {
    pub dl_chunk_size: Option<u64>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        DownloadOptions {
            dl_chunk_size: None,
        }
    }
}

#[derive(Clone, Debug, derive_more::Display)]
#[display(fmt = "RequestOptions()")]
pub struct RequestOptions {
    pub proxy: Option<reqwest::Proxy>,
    /// **Example**: Some("key1=value1; key2=value2; key3=value3".to_string())
    pub cookies: Option<String>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        RequestOptions {
            proxy: None,
            cookies: None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum VideoError {
    #[error("The video not found")]
    VideoNotFound,
    #[error("Video source empty")]
    VideoSourceNotFound,
    #[error("Video is private")]
    VideoIsPrivate,
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    URLParseError(#[from] url::ParseError),
    #[error("Body cannot parsed")]
    BodyCannotParsed,
    #[error("Format not found")]
    FormatNotFound,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoDetails {
    pub author: Author,
    pub media: serde_json::Value,
    pub likes: i32,
    pub dislikes: i32,
    pub age_restricted: bool,
    pub video_url: String,
    pub storyboards: Vec<StoryBoard>,
    pub chapters: Vec<Chapter>,
    pub embed: Embed,
    pub title: String,
    pub description: String,
    pub length_seconds: String,
    pub owner_profile_url: String,
    pub external_channel_id: String,
    pub is_family_safe: bool,
    pub available_countries: Vec<String>,
    pub is_unlisted: bool,
    pub has_ypc_metadata: bool,
    pub view_count: String,
    pub category: String,
    pub publish_date: String,
    pub owner_channel_name: String,
    pub upload_date: String,
    pub video_id: String,
    pub keywords: Vec<String>,
    pub channel_id: String,
    pub is_owner_viewing: bool,
    pub is_crawlable: bool,
    pub allow_ratings: bool,
    pub is_private: bool,
    pub is_unplugged_corpus: bool,
    pub is_live_content: bool,
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub name: String,
    pub user: String,
    pub channel_url: String,
    pub external_channel_url: String,
    pub user_url: String,
    pub thumbnails: Vec<Thumbnail>,
    pub verified: bool,
    pub subscriber_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub start_time: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoryBoard {
    pub template_url: String,
    pub thumbnail_width: i32,
    pub thumbnail_height: i32,
    pub thumbnail_count: i32,
    pub interval: i32,
    pub columns: i32,
    pub rows: i32,
    pub storyboard_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Thumbnail {
    pub width: i32,
    pub height: i32,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Embed {
    pub flash_secure_url: String,
    pub flash_url: String,
    pub iframe_url: String,
    pub height: i32,
    pub width: i32,
}

pub trait StringUtils {
    fn substring(&self, start: usize, len: usize) -> &str;
    fn slice(&self, range: impl RangeBounds<usize>) -> &str;
}

impl StringUtils for str {
    fn substring(&self, start: usize, len: usize) -> &str {
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
        self.substring(start, len)
    }
}
