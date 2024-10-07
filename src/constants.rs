use mime::Mime;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{HeaderMap, USER_AGENT};
use std::{collections::HashMap, str::FromStr};

use crate::structs::{MimeType, StaticFormat};

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

// 10485760 -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
pub(crate) const DEFAULT_DL_CHUNK_SIZE: u64 = 10485760;

/// Default max number of retries for a web reqwest.
pub(crate) const DEFAULT_MAX_RETRIES: u32 = 3;

pub static INNERTUBE_CLIENT: Lazy<HashMap<&str, (&str, &str, &str)>> =
    // (clientVersion, clientName, json value)
    Lazy::new(|| {
        HashMap::from([
            (
                "web",
                (
                    "2.20240726.00.00",
                    "1",
                    r#""context": {
                        "client": {
                            "clientName": "WEB",
                            "clientVersion": "2.20240726.00.00",
                            "hl": "en"
                        }
                    },"#,
                ),
            ),
            (
                "ios",
                (
                    "19.29.1",
                    "5",
                    r#""context": {
                        "client": {
                            "clientName": "IOS",
                            "clientVersion": "19.29.1",
                            "deviceMake": "Apple",
                            "deviceModel": "iPhone16,2",
                            "userAgent": "com.google.ios.youtube/19.29.1 (iPhone16,2; U; CPU iOS 17_5_1 like Mac OS X;)",
                            "osName": "iPhone",
                            "osVersion": "17.5.1.21F90",
                            "hl": "en"
                        }
                    },"#,
                ),
            ),
            (
                // This client can access age restricted videos (unless the uploader has disabled the 'allow embedding' option)
                // See: https://github.com/yt-dlp/yt-dlp/blob/28d485714fef88937c82635438afba5db81f9089/yt_dlp/extractor/youtube.py#L231
                "tv_embedded",
                (
                    "2.0",
                    "85",
                    r#""context": {
                        "client": {
                            "clientName": "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
                            "clientVersion": "2.0",
                            "hl": "en",
                            "clientScreen": "EMBED"
                        },
                        "thirdParty": {
                            "embedUrl": "https://google.com"
                        }
                    },"#,
                ),
            ),
        ])
    });

pub static FORMATS: Lazy<HashMap<&str, StaticFormat>> = Lazy::new(|| {
    HashMap::from([
        (
            "5",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/flv; codecs=\"Sorenson H.283, mp3\"")
                        .expect("Static mime error: itag 5"),
                    container: "flv".to_string(),
                    codecs: vec!["Sorenson H.283".to_string(), "mp3".to_string()],
                    video_codec: Some("Sorenson H.283".to_string()),
                    audio_codec: Some("mp3".to_string()),
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(250000),
                audio_bitrate: Some(64),
            },
        ),
        (
            "6",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/flv; codecs=\"Sorenson H.263, mp3\"")
                        .expect("Static mime error: itag 6"),
                    container: "flv".to_string(),
                    codecs: vec!["Sorenson H.263".to_string(), "mp3".to_string()],
                    video_codec: Some("Sorenson H.263".to_string()),
                    audio_codec: Some("mp3".to_string()),
                },
                quality_label: Some("270p".to_string()),
                bitrate: Some(800000),
                audio_bitrate: Some(64),
            },
        ),
        (
            "13",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/3gp; codecs=\"MPEG-4 Visual, aac\"")
                        .expect("Static mime error: itag 13"),
                    container: "3gp".to_string(),
                    codecs: vec!["MPEG-4 Visual".to_string(), "aac".to_string()],
                    video_codec: Some("MPEG-4 Visual".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: Some(500000),
                audio_bitrate: None,
            },
        ),
        (
            "17",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/3gp; codecs=\"MPEG-4 Visual, aac\"")
                        .expect("Static mime error: itag 17"),
                    container: "3gp".to_string(),
                    codecs: vec!["MPEG-4 Visual".to_string(), "aac".to_string()],
                    video_codec: Some("MPEG-4 Visual".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("144p".to_string()),
                bitrate: Some(50000),
                audio_bitrate: Some(24),
            },
        ),
        (
            "18",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 18"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(96),
            },
        ),
        (
            "22",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 22"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(2000000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "34",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/flv; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 34"),
                    container: "flv".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "35",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/flv; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 35"),
                    container: "flv".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("480p".to_string()),
                bitrate: Some(800000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "36",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/3gp; codecs=\"MPEG-4 Visual, aac\"")
                        .expect("Static mime error: itag 36"),
                    container: "3gp".to_string(),
                    codecs: vec!["MPEG-4 Visual".to_string(), "aac".to_string()],
                    video_codec: Some("MPEG-4 Visual".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(175000),
                audio_bitrate: Some(32),
            },
        ),
        (
            "37",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 37"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(3000000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "38",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 38"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("3072p".to_string()),
                bitrate: Some(3500000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "43",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP8, vorbis\"")
                        .expect("Static mime error: itag 43"),
                    container: "webm".to_string(),
                    codecs: vec!["VP8".to_string(), "vorbis".to_string()],
                    video_codec: Some("VP8".to_string()),
                    audio_codec: Some("vorbis".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "44",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP8, vorbis\"")
                        .expect("Static mime error: itag 44"),
                    container: "webm".to_string(),
                    codecs: vec!["VP8".to_string(), "vorbis".to_string()],
                    video_codec: Some("VP8".to_string()),
                    audio_codec: Some("vorbis".to_string()),
                },
                quality_label: Some("480p".to_string()),
                bitrate: Some(1000000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "45",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP8, vorbis\"")
                        .expect("Static mime error: itag 45"),
                    container: "webm".to_string(),
                    codecs: vec!["VP8".to_string(), "vorbis".to_string()],
                    video_codec: Some("VP8".to_string()),
                    audio_codec: Some("vorbis".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(2000000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "46",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vp8, vorbis\"")
                        .expect("Static mime error: itag 46"),
                    container: "webm".to_string(),
                    codecs: vec!["vp8".to_string(), "vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vp8".to_string()),
                },
                quality_label: Some("1080p".to_string()),
                bitrate: None,
                audio_bitrate: Some(192),
            },
        ),
        (
            "82",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 82"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(96),
            },
        ),
        (
            "83",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 83"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(96),
            },
        ),
        (
            "84",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 84"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(2000000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "85",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 85"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(3000000),
                audio_bitrate: Some(192),
            },
        ),
        (
            "91",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 91"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("144p".to_string()),
                bitrate: Some(100000),
                audio_bitrate: Some(48),
            },
        ),
        (
            "92",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 92"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(150000),
                audio_bitrate: Some(48),
            },
        ),
        (
            "93",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 93"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "94",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 94"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("480p".to_string()),
                bitrate: Some(800000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "95",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 95"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(1500000),
                audio_bitrate: Some(256),
            },
        ),
        (
            "96",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 96"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(2500000),
                audio_bitrate: Some(256),
            },
        ),
        (
            "100",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vp8, vorbis\"")
                        .expect("Static mime error: itag 100"),
                    container: "webm".to_string(),
                    codecs: vec!["vp8".to_string(), "vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vp8".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: None,
                audio_bitrate: Some(128),
            },
        ),
        (
            "101",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vp8, vorbis\"")
                        .expect("Static mime error: itag 101"),
                    container: "webm".to_string(),
                    codecs: vec!["vp8".to_string(), "vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vp8".to_string()),
                },
                quality_label: Some("360p".to_string()),
                bitrate: None,
                audio_bitrate: Some(192),
            },
        ),
        (
            "102",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vp8, vorbis\"")
                        .expect("Static mime error: itag 102"),
                    container: "webm".to_string(),
                    codecs: vec!["vp8".to_string(), "vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vp8".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: None,
                audio_bitrate: Some(192),
            },
        ),
        (
            "120",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/flv; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 120"),
                    container: "flv".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(2000000),
                audio_bitrate: Some(128),
            },
        ),
        (
            "127",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/ts; codecs=\"aac\"")
                        .expect("Static mime error: itag 127"),
                    container: "ts".to_string(),
                    codecs: vec!["aac".to_string()],
                    video_codec: None,
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(96),
            },
        ),
        (
            "128",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/ts; codecs=\"aac\"")
                        .expect("Static mime error: itag 128"),
                    container: "ts".to_string(),
                    codecs: vec!["aac".to_string()],
                    video_codec: None,
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(96),
            },
        ),
        (
            "132",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 132"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(150000),
                audio_bitrate: Some(48),
            },
        ),
        (
            "133",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 133"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(200000),
                audio_bitrate: None,
            },
        ),
        (
            "134",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 134"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(300000),
                audio_bitrate: None,
            },
        ),
        (
            "135",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 135"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("480p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: None,
            },
        ),
        (
            "136",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 136"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(1000000),
                audio_bitrate: None,
            },
        ),
        (
            "137",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 137"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(2500000),
                audio_bitrate: None,
            },
        ),
        (
            "138",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 138"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("4320p".to_string()),
                bitrate: Some(13500000),
                audio_bitrate: None,
            },
        ),
        (
            "139",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/mp4; codecs=\"aac\"")
                        .expect("Static mime error: itag 139"),
                    container: "mp4".to_string(),
                    codecs: vec!["aac".to_string()],
                    video_codec: None,
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(48),
            },
        ),
        (
            "140",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/m4a; codecs=\"aac\"")
                        .expect("Static mime error: itag 140"),
                    container: "m4a".to_string(),
                    codecs: vec!["aac".to_string()],
                    video_codec: None,
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(128),
            },
        ),
        (
            "141",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/mp4; codecs=\"aac\"")
                        .expect("Static mime error: itag 141"),
                    container: "mp4".to_string(),
                    codecs: vec!["aac".to_string()],
                    video_codec: None,
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(256),
            },
        ),
        (
            "151",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 151"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(50000),
                audio_bitrate: Some(24),
            },
        ),
        (
            "160",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 160"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("144p".to_string()),
                bitrate: Some(100000),
                audio_bitrate: None,
            },
        ),
        (
            "171",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vorbis\"")
                        .expect("Static mime error: itag 171"),
                    container: "webm".to_string(),
                    codecs: vec!["vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vorbis".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(128),
            },
        ),
        (
            "172",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"vorbis\"")
                        .expect("Static mime error: itag 172"),
                    container: "webm".to_string(),
                    codecs: vec!["vorbis".to_string()],
                    video_codec: None,
                    audio_codec: Some("vorbis".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(192),
            },
        ),
        (
            "242",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 242"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("240p".to_string()),
                bitrate: Some(100000),
                audio_bitrate: None,
            },
        ),
        (
            "243",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 243"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("360p".to_string()),
                bitrate: Some(250000),
                audio_bitrate: None,
            },
        ),
        (
            "244",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 244"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("480p".to_string()),
                bitrate: Some(500000),
                audio_bitrate: None,
            },
        ),
        (
            "247",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 247"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(700000),
                audio_bitrate: None,
            },
        ),
        (
            "248",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 248"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(1500000),
                audio_bitrate: None,
            },
        ),
        (
            "249",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"opus\"")
                        .expect("Static mime error: itag 249"),
                    container: "webm".to_string(),
                    codecs: vec!["opus".to_string()],
                    video_codec: None,
                    audio_codec: Some("opus".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(48),
            },
        ),
        (
            "250",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"opus\"")
                        .expect("Static mime error: itag 250"),
                    container: "webm".to_string(),
                    codecs: vec!["opus".to_string()],
                    video_codec: None,
                    audio_codec: Some("opus".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(64),
            },
        ),
        (
            "251",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("audio/webm; codecs=\"opus\"")
                        .expect("Static mime error: itag 251"),
                    container: "webm".to_string(),
                    codecs: vec!["opus".to_string()],
                    video_codec: None,
                    audio_codec: Some("opus".to_string()),
                },
                quality_label: None,
                bitrate: None,
                audio_bitrate: Some(160),
            },
        ),
        (
            "264",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 264"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1440p".to_string()),
                bitrate: Some(4000000),
                audio_bitrate: None,
            },
        ),
        (
            "266",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 266"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("2160p".to_string()),
                bitrate: Some(12500000),
                audio_bitrate: None,
            },
        ),
        (
            "271",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 271"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1440p".to_string()),
                bitrate: Some(9000000),
                audio_bitrate: None,
            },
        ),
        (
            "272",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 272"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("4320p".to_string()),
                bitrate: Some(20000000),
                audio_bitrate: None,
            },
        ),
        (
            "278",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 278"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("144p 30fps".to_string()),
                bitrate: Some(80000),
                audio_bitrate: None,
            },
        ),
        (
            "298",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 298"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(3000000),
                audio_bitrate: None,
            },
        ),
        (
            "299",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/mp4; codecs=\"H.264\"")
                        .expect("Static mime error: itag 299"),
                    container: "mp4".to_string(),
                    codecs: vec!["H.264".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1080p".to_string()),
                bitrate: Some(5500000),
                audio_bitrate: None,
            },
        ),
        (
            "300",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/ts; codecs=\"H.264, aac\"")
                        .expect("Static mime error: itag 300"),
                    container: "ts".to_string(),
                    codecs: vec!["H.264".to_string(), "aac".to_string()],
                    video_codec: Some("H.264".to_string()),
                    audio_codec: Some("aac".to_string()),
                },
                quality_label: Some("720p".to_string()),
                bitrate: Some(1318000),
                audio_bitrate: Some(48),
            },
        ),
        (
            "302",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 302"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("720p HFR".to_string()),
                bitrate: Some(2500000),
                audio_bitrate: None,
            },
        ),
        (
            "303",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 303"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1080p HFR".to_string()),
                bitrate: Some(5000000),
                audio_bitrate: None,
            },
        ),
        (
            "308",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 308"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1440p HFR".to_string()),
                bitrate: Some(10000000),
                audio_bitrate: None,
            },
        ),
        (
            "313",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 313"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("2160p".to_string()),
                bitrate: Some(13000000),
                audio_bitrate: None,
            },
        ),
        (
            "315",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 315"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("2160p HFR".to_string()),
                bitrate: Some(20000000),
                audio_bitrate: None,
            },
        ),
        (
            "330",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 330"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("144p HDR, HFR".to_string()),
                bitrate: Some(80000),
                audio_bitrate: None,
            },
        ),
        (
            "331",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 331"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("240p HDR, HFR".to_string()),
                bitrate: Some(100000),
                audio_bitrate: None,
            },
        ),
        (
            "332",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 332"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("360p HDR, HFR".to_string()),
                bitrate: Some(250000),
                audio_bitrate: None,
            },
        ),
        (
            "333",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 333"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("240p HDR, HFR".to_string()),
                bitrate: Some(500000),
                audio_bitrate: None,
            },
        ),
        (
            "334",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 334"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("720p HDR, HFR".to_string()),
                bitrate: Some(1000000),
                audio_bitrate: None,
            },
        ),
        (
            "335",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 335"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1080p HDR, HFR".to_string()),
                bitrate: Some(1500000),
                audio_bitrate: None,
            },
        ),
        (
            "336",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 336"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("1440p HDR, HFR".to_string()),
                bitrate: Some(5000000),
                audio_bitrate: None,
            },
        ),
        (
            "337",
            StaticFormat {
                mime_type: MimeType {
                    mime: Mime::from_str("video/webm; codecs=\"VP9\"")
                        .expect("Static mime error: itag 337"),
                    container: "webm".to_string(),
                    codecs: vec!["VP9".to_string()],
                    video_codec: Some("VP9".to_string()),
                    audio_codec: None,
                },
                quality_label: Some("2160p HDR, HFR".to_string()),
                bitrate: Some(12000000),
                audio_bitrate: None,
            },
        ),
    ])
});
