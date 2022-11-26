mod info;
mod info_extras;
mod utils;

use tokio_stream::StreamExt;

use serde::{Deserialize, Serialize};
use std::ops::{Bound, RangeBounds};
use tokio::{fs::File, io::AsyncWriteExt};

use info::get_info;
use utils::choose_format;

const BASE_URL: &str = "https://www.youtube.com/watch?v=";

trait StringUtils {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoInfo {
    player_response: serde_json::Value,
    initial_response: serde_json::Value,
    html5player: String,
    formats: Vec<serde_json::Value>,
    related_videos: Vec<serde_json::Value>,
    video_details: VideoDetails,
}

#[derive(PartialEq)]
pub enum VideoSearchOptions {
    VideoAuido,
    Video,
    Audio,
}

#[derive(PartialEq)]
pub enum VideoQuality {
    Highest,
    Lowest,
}

#[allow(dead_code)]
pub struct VideoOptions {
    seek: i32,
    fmt: String,
    encoder_args: Vec<String>,
    quality: VideoQuality,
    filter: VideoSearchOptions,
    high_water_mark: i32,
}

impl Default for VideoOptions {
    fn default() -> Self {
        VideoOptions {
            seek: 0,
            fmt: String::from("s16le"),
            encoder_args: vec![],
            quality: VideoQuality::Highest,
            filter: VideoSearchOptions::Audio,
            high_water_mark: 1 << 20, //1 << 14, // 16kb,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoDetails {
    author: Author,
    media: serde_json::Value,
    likes: i32,
    dislikes: i32,
    age_restricted: bool,
    video_url: String,
    storyboards: Vec<StoryBoard>,
    chapters: Vec<Chapter>,
    embed: Embed,
    title: String,
    description: String,
    length_seconds: String,
    owner_profile_url: String,
    external_channel_id: String,
    is_family_safe: bool,
    available_countries: Vec<String>,
    is_unlisted: bool,
    has_ypc_metadata: bool,
    view_count: String,
    category: String,
    publish_date: String,
    owner_channel_name: String,
    upload_date: String,
    video_id: String,
    keywords: Vec<String>,
    channel_id: String,
    is_owner_viewing: bool,
    is_crawlable: bool,
    allow_ratings: bool,
    is_private: bool,
    is_unplugged_corpus: bool,
    is_live_content: bool,
    thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    id: String,
    name: String,
    user: String,
    channel_url: String,
    external_channel_url: String,
    user_url: String,
    thumbnails: Vec<Thumbnail>,
    verified: bool,
    subscriber_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    title: String,
    start_time: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoryBoard {
    template_url: String,
    thumbnail_width: i32,
    thumbnail_height: i32,
    thumbnail_count: i32,
    interval: i32,
    columns: i32,
    rows: i32,
    storyboard_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Thumbnail {
    width: i32,
    height: i32,
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Embed {
    flash_secure_url: String,
    flash_url: String,
    iframe_url: String,
    height: i32,
    width: i32,
}

#[tokio::main]
async fn main() {
    let start_time = std::time::Instant::now();
    let video_info = get_info("https://www.youtube.com/watch?v=FZ8BxMU3BYc").await;
    // let video_info = get_info("https://www.youtube.com/watch?v=z3wAjJXbYzA").await;
    let video_options = VideoOptions::default();

    println!("{:#?}", video_info.formats);
    // download_from_info(&video_info, &video_options).await;
    println!("Time elapsed: {}", start_time.elapsed().as_secs_f64());
}

async fn download_from_info(info: &VideoInfo, options: &VideoOptions) {
    let video = choose_format(&info.formats, options);

    println!("{:#?}", video);

    let url = video.get("url").and_then(|x| x.as_str()).unwrap_or("");

    let client = reqwest::Client::new();
    let response = client.head(url).send().await.unwrap();
    let length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .ok_or("response doesn't include the content length")
        .unwrap();

    let length = length
        .to_str()
        .unwrap()
        .parse::<u64>()
        .map_err(|_| "invalid Content-Length header")
        .unwrap();

    let mut output_file = File::create("download.mp3").await.unwrap();

    println!("starting download... Bytes: {}", length);
    let now = std::time::Instant::now();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.101 Safari/537.36").unwrap());

    let mut stream = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .unwrap()
        .bytes_stream();

    let mut downloaded_byte = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        downloaded_byte += chunk.len();
        println!(
            "Received {} bytes - %{:.1} downloaded",
            chunk.len(),
            (downloaded_byte as f64 / length as f64) as f64 * 100f64
        );

        output_file.write_all(&chunk).await.unwrap();
    }

    println!("Finished with success! in {}", now.elapsed().as_secs_f64());
}
