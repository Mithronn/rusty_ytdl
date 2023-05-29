#![recursion_limit = "256"]
#![allow(dead_code)]
#![allow(unused_imports)]

mod info;
mod info_extras;
mod structs;
mod utils;

pub mod constants;
pub mod stream;

#[cfg(feature = "blocking")]
pub mod blocking;

#[cfg(feature = "search")]
pub mod search;

pub use info::Video;
pub use structs::{
    Author, Chapter, ColorInfo, DownloadOptions, Embed, RangeObject, RelatedVideo, RequestOptions,
    StoryBoard, Thumbnail, VideoDetails, VideoError, VideoFormat, VideoInfo, VideoOptions,
    VideoQuality, VideoSearchOptions,
};
pub use utils::{choose_format, get_random_v6_ip, get_video_id};
// export to access proxy feature
pub use reqwest;
