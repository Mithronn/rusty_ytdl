use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{HeaderMap, USER_AGENT};
use std::collections::HashMap;

use crate::structs::{StaticFormat, StaticFormatRaw};

pub const BASE_URL: &str = "https://www.youtube.com/watch?v=";

pub const VALID_QUERY_DOMAINS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "gaming.youtube.com",
];

pub const AGE_RESTRICTED_URLS: &[&str] = &[
    "support.google.com/youtube/?p=age_restrictions",
    "youtube.com/t/community_guidelines",
];

pub const AUDIO_ENCODING_RANKS: &[&str] = &["mp4a", "mp3", "vorbis", "aac", "opus", "flac"];
pub const VIDEO_ENCODING_RANKS: &[&str] = &[
    "mp4v",
    "avc1",
    "Sorenson H.283",
    "MPEG-4 Visual",
    "VP8",
    "VP9",
    "H.264",
];

pub(crate) static DEFAULT_HEADERS: Lazy<HeaderMap> = Lazy::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.101 Safari/537.36".parse().unwrap());

    headers
});

pub(crate) static IPV6_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(([0-9a-f]{1,4}:)(:[0-9a-f]{1,4}){1,6}|([0-9a-f]{1,4}:){1,2}(:[0-9a-f]{1,4}){1,5}|([0-9a-f]{1,4}:){1,3}(:[0-9a-f]{1,4}){1,4}|([0-9a-f]{1,4}:){1,4}(:[0-9a-f]{1,4}){1,3}|([0-9a-f]{1,4}:){1,5}(:[0-9a-f]{1,4}){1,2}|([0-9a-f]{1,4}:){1,6}(:[0-9a-f]{1,4})|([0-9a-f]{1,4}:){1,7}(([0-9a-f]{1,4})|:))/(1[0-1]\d|12[0-8]|\d{1,2})$").unwrap()
});

pub(crate) static PARSE_INT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*((\-|\+)?[0-9]+)\s*").unwrap());

pub static FORMATS: Lazy<HashMap<&str, StaticFormat>> = Lazy::new(|| {
    HashMap::from(
        [
            (
                "5",
                StaticFormatRaw {
                    mime_type: r#"video/flv; codecs="Sorenson H.283, mp3""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(250000),
                    audio_bitrate: Some(64),
                },
            ),
            (
                "6",
                StaticFormatRaw {
                    mime_type: r#"video/flv; codecs="Sorenson H.263, mp3""#.to_string(),
                    quality_label: Some("270p".to_string()),
                    bitrate: Some(800000),
                    audio_bitrate: Some(64),
                },
            ),
            (
                "13",
                StaticFormatRaw {
                    mime_type: r#"video/3gp; codecs="MPEG-4 Visual, aac""#.to_string(),
                    quality_label: None,
                    bitrate: Some(500000),
                    audio_bitrate: None,
                },
            ),
            (
                "17",
                StaticFormatRaw {
                    mime_type: r#"video/3gp; codecs="MPEG-4 Visual, aac""#.to_string(),
                    quality_label: Some("144p".to_string()),
                    bitrate: Some(50000),
                    audio_bitrate: Some(24),
                },
            ),
            (
                "18",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(96),
                },
            ),
            (
                "22",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(2000000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "34",
                StaticFormatRaw {
                    mime_type: r#"video/flv; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "35",
                StaticFormatRaw {
                    mime_type: r#"video/flv; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("480p".to_string()),
                    bitrate: Some(800000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "36",
                StaticFormatRaw {
                    mime_type: r#"video/3gp; codecs="MPEG-4 Visual, aac""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(175000),
                    audio_bitrate: Some(32),
                },
            ),
            (
                "37",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(3000000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "38",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("3072p".to_string()),
                    bitrate: Some(3500000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "43",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "44",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("480p".to_string()),
                    bitrate: Some(1000000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "45",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(2000000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "46",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="vp8, vorbis""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: None,
                    audio_bitrate: Some(192),
                },
            ),
            (
                "82",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(96),
                },
            ),
            (
                "83",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(96),
                },
            ),
            (
                "84",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(2000000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "85",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(3000000),
                    audio_bitrate: Some(192),
                },
            ),
            (
                "91",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("144p".to_string()),
                    bitrate: Some(100000),
                    audio_bitrate: Some(48),
                },
            ),
            (
                "92",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(150000),
                    audio_bitrate: Some(48),
                },
            ),
            (
                "93",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "94",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("480p".to_string()),
                    bitrate: Some(800000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "95",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(1500000),
                    audio_bitrate: Some(256),
                },
            ),
            (
                "96",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(2500000),
                    audio_bitrate: Some(256),
                },
            ),
            (
                "100",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: None,
                    audio_bitrate: Some(128),
                },
            ),
            (
                "101",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: None,
                    audio_bitrate: Some(192),
                },
            ),
            (
                "102",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="VP8, vorbis""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: None,
                    audio_bitrate: Some(192),
                },
            ),
            (
                "120",
                StaticFormatRaw {
                    mime_type: r#"video/flv; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(2000000),
                    audio_bitrate: Some(128),
                },
            ),
            (
                "127",
                StaticFormatRaw {
                    mime_type: r#"audio/ts; codecs="aac""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(96),
                },
            ),
            (
                "128",
                StaticFormatRaw {
                    mime_type: r#"audio/ts; codecs="aac""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(96),
                },
            ),
            (
                "132",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(150000),
                    audio_bitrate: Some(48),
                },
            ),
            (
                "133",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(200000),
                    audio_bitrate: None,
                },
            ),
            (
                "134",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(300000),
                    audio_bitrate: None,
                },
            ),
            (
                "135",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("480p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: None,
                },
            ),
            (
                "136",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(1000000),
                    audio_bitrate: None,
                },
            ),
            (
                "137",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(2500000),
                    audio_bitrate: None,
                },
            ),
            (
                "138",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("4320p".to_string()),
                    bitrate: Some(13500000),
                    audio_bitrate: None,
                },
            ),
            (
                "139",
                StaticFormatRaw {
                    mime_type: r#"audio/mp4; codecs="aac""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(48),
                },
            ),
            (
                "140",
                StaticFormatRaw {
                    mime_type: r#"audio/m4a; codecs="aac""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(128),
                },
            ),
            (
                "141",
                StaticFormatRaw {
                    mime_type: r#"audio/mp4; codecs="aac""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(256),
                },
            ),
            (
                "151",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(50000),
                    audio_bitrate: Some(24),
                },
            ),
            (
                "160",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("144p".to_string()),
                    bitrate: Some(100000),
                    audio_bitrate: None,
                },
            ),
            (
                "171",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="vorbis""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(128),
                },
            ),
            (
                "172",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="vorbis""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(192),
                },
            ),
            (
                "242",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("240p".to_string()),
                    bitrate: Some(100000),
                    audio_bitrate: None,
                },
            ),
            (
                "243",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("360p".to_string()),
                    bitrate: Some(250000),
                    audio_bitrate: None,
                },
            ),
            (
                "244",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("480p".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: None,
                },
            ),
            (
                "247",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(700000),
                    audio_bitrate: None,
                },
            ),
            (
                "248",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(1500000),
                    audio_bitrate: None,
                },
            ),
            (
                "249",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="opus""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(48),
                },
            ),
            (
                "250",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="opus""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(64),
                },
            ),
            (
                "251",
                StaticFormatRaw {
                    mime_type: r#"audio/webm; codecs="opus""#.to_string(),
                    quality_label: None,
                    bitrate: None,
                    audio_bitrate: Some(160),
                },
            ),
            (
                "264",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("1440p".to_string()),
                    bitrate: Some(4000000),
                    audio_bitrate: None,
                },
            ),
            (
                "266",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("2160p".to_string()),
                    bitrate: Some(12500000),
                    audio_bitrate: None,
                },
            ),
            (
                "271",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1440p".to_string()),
                    bitrate: Some(9000000),
                    audio_bitrate: None,
                },
            ),
            (
                "272",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("4320p".to_string()),
                    bitrate: Some(20000000),
                    audio_bitrate: None,
                },
            ),
            (
                "278",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("144p 30fps".to_string()),
                    bitrate: Some(80000),
                    audio_bitrate: None,
                },
            ),
            (
                "298",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(3000000),
                    audio_bitrate: None,
                },
            ),
            (
                "299",
                StaticFormatRaw {
                    mime_type: r#"video/mp4; codecs="H.264""#.to_string(),
                    quality_label: Some("1080p".to_string()),
                    bitrate: Some(5500000),
                    audio_bitrate: None,
                },
            ),
            (
                "300",
                StaticFormatRaw {
                    mime_type: r#"video/ts; codecs="H.264, aac""#.to_string(),
                    quality_label: Some("720p".to_string()),
                    bitrate: Some(1318000),
                    audio_bitrate: Some(48),
                },
            ),
            (
                "302",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("720p HFR".to_string()),
                    bitrate: Some(2500000),
                    audio_bitrate: None,
                },
            ),
            (
                "303",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1080p HFR".to_string()),
                    bitrate: Some(5000000),
                    audio_bitrate: None,
                },
            ),
            (
                "308",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1440p HFR".to_string()),
                    bitrate: Some(10000000),
                    audio_bitrate: None,
                },
            ),
            (
                "313",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("2160p".to_string()),
                    bitrate: Some(13000000),
                    audio_bitrate: None,
                },
            ),
            (
                "315",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("2160p HFR".to_string()),
                    bitrate: Some(20000000),
                    audio_bitrate: None,
                },
            ),
            (
                "330",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("144p HDR, HFR".to_string()),
                    bitrate: Some(80000),
                    audio_bitrate: None,
                },
            ),
            (
                "331",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("240p HDR, HFR".to_string()),
                    bitrate: Some(100000),
                    audio_bitrate: None,
                },
            ),
            (
                "332",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("360p HDR, HFR".to_string()),
                    bitrate: Some(250000),
                    audio_bitrate: None,
                },
            ),
            (
                "333",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("240p HDR, HFR".to_string()),
                    bitrate: Some(500000),
                    audio_bitrate: None,
                },
            ),
            (
                "334",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("720p HDR, HFR".to_string()),
                    bitrate: Some(1000000),
                    audio_bitrate: None,
                },
            ),
            (
                "335",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1080p HDR, HFR".to_string()),
                    bitrate: Some(1500000),
                    audio_bitrate: None,
                },
            ),
            (
                "336",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("1440p HDR, HFR".to_string()),
                    bitrate: Some(5000000),
                    audio_bitrate: None,
                },
            ),
            (
                "337",
                StaticFormatRaw {
                    mime_type: r#"video/webm; codecs="VP9""#.to_string(),
                    quality_label: Some("2160p HDR, HFR".to_string()),
                    bitrate: Some(12000000),
                    audio_bitrate: None,
                },
            ),
        ]
        .map(|(itag, raw_static_format)| (itag, StaticFormat::from(raw_static_format))),
    )
});
