use crate::structs::EscapeSequenze;
use once_cell::sync::Lazy;

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

pub(crate) static DEFAULT_HEADERS: Lazy<reqwest::header::HeaderMap> = Lazy::new(|| {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.101 Safari/537.36".parse().unwrap());

    headers
});

pub(crate) static IPV6_REGEX: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r#"^(([0-9a-f]{1,4}:)(:[0-9a-f]{1,4}){1,6}|([0-9a-f]{1,4}:){1,2}(:[0-9a-f]{1,4}){1,5}|([0-9a-f]{1,4}:){1,3}(:[0-9a-f]{1,4}){1,4}|([0-9a-f]{1,4}:){1,4}(:[0-9a-f]{1,4}){1,3}|([0-9a-f]{1,4}:){1,5}(:[0-9a-f]{1,4}){1,2}|([0-9a-f]{1,4}:){1,6}(:[0-9a-f]{1,4})|([0-9a-f]{1,4}:){1,7}(([0-9a-f]{1,4})|:))/(1[0-1]\d|12[0-8]|\d{1,2})$"#).unwrap()
});

pub(crate) static PARSE_INT_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"(?m)^\s*((\-|\+)?[0-9]+)\s*"#).unwrap());

pub(crate) static ESCAPING_SEQUENZES: Lazy<[EscapeSequenze; 4]> = Lazy::new(|| {
    [
        EscapeSequenze {
            start: r#"""#.to_string(),
            end: r#"""#.to_string(),
            start_prefix: None,
        },
        EscapeSequenze {
            start: "'".to_string(),
            end: "'".to_string(),
            start_prefix: None,
        },
        EscapeSequenze {
            start: "`".to_string(),
            end: "`".to_string(),
            start_prefix: None,
        },
        EscapeSequenze {
            start: "/".to_string(),
            end: "/".to_string(),
            start_prefix: Some(regex::Regex::new(r#"(^|[\[{:;,/])\s?$"#).expect("IMPOSSIBLE")),
        },
    ]
});

pub static FORMATS: Lazy<serde_json::Value> = Lazy::new(|| {
    serde_json::json!({
      "5": {
        "mimeType": r#"video/flv; codecs="Sorenson H.283, mp3""#,
        "qualityLabel": "240p",
        "bitrate": 250000,
        "audioBitrate": 64,
      },

      "6": {
        "mimeType": r#"video/flv; codecs="Sorenson H.263, mp3""#,
        "qualityLabel": "270p",
        "bitrate": 800000,
        "audioBitrate": 64,
      },

      "13": {
        "mimeType": r#"video/3gp; codecs="MPEG-4 Visual, aac""#,
        "qualityLabel": null,
        "bitrate": 500000,
        "audioBitrate": null,
      },

      "17": {
        "mimeType": r#"video/3gp; codecs="MPEG-4 Visual, aac""#,
        "qualityLabel": "144p",
        "bitrate": 50000,
        "audioBitrate": 24,
      },

      "18": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "360p",
        "bitrate": 500000,
        "audioBitrate": 96,
      },

      "22": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 2000000,
        "audioBitrate": 192,
      },

      "34": {
        "mimeType": r#"video/flv; codecs="H.264, aac""#,
        "qualityLabel": "360p",
        "bitrate": 500000,
        "audioBitrate": 128,
      },

      "35": {
        "mimeType": r#"video/flv; codecs="H.264, aac""#,
        "qualityLabel": "480p",
        "bitrate": 800000,
        "audioBitrate": 128,
      },

      "36": {
        "mimeType": r#"video/3gp; codecs="MPEG-4 Visual, aac""#,
        "qualityLabel": "240p",
        "bitrate": 175000,
        "audioBitrate": 32,
      },

      "37": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "1080p",
        "bitrate": 3000000,
        "audioBitrate": 192,
      },

      "38": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "3072p",
        "bitrate": 3500000,
        "audioBitrate": 192,
      },

      "43": {
        "mimeType": r#"video/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "360p",
        "bitrate": 500000,
        "audioBitrate": 128,
      },

      "44": {
        "mimeType": r#"video/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "480p",
        "bitrate": 1000000,
        "audioBitrate": 128,
      },

      "45": {
        "mimeType": r#"video/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "720p",
        "bitrate": 2000000,
        "audioBitrate": 192,
      },

      "46": {
        "mimeType": r#"audio/webm; codecs="vp8, vorbis""#,
        "qualityLabel": "1080p",
        "bitrate": null,
        "audioBitrate": 192,
      },

      "82": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "360p",
        "bitrate": 500000,
        "audioBitrate": 96,
      },

      "83": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "240p",
        "bitrate": 500000,
        "audioBitrate": 96,
      },

      "84": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 2000000,
        "audioBitrate": 192,
      },

      "85": {
        "mimeType": r#"video/mp4; codecs="H.264, aac""#,
        "qualityLabel": "1080p",
        "bitrate": 3000000,
        "audioBitrate": 192,
      },

      "91": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "144p",
        "bitrate": 100000,
        "audioBitrate": 48,
      },

      "92": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "240p",
        "bitrate": 150000,
        "audioBitrate": 48,
      },

      "93": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "360p",
        "bitrate": 500000,
        "audioBitrate": 128,
      },

      "94": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "480p",
        "bitrate": 800000,
        "audioBitrate": 128,
      },

      "95": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 1500000,
        "audioBitrate": 256,
      },

      "96": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "1080p",
        "bitrate": 2500000,
        "audioBitrate": 256,
      },

      "100": {
        "mimeType": r#"audio/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "360p",
        "bitrate": null,
        "audioBitrate": 128,
      },

      "101": {
        "mimeType": r#"audio/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "360p",
        "bitrate": null,
        "audioBitrate": 192,
      },

      "102": {
        "mimeType": r#"audio/webm; codecs="VP8, vorbis""#,
        "qualityLabel": "720p",
        "bitrate": null,
        "audioBitrate": 192,
      },

      "120": {
        "mimeType": r#"video/flv; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 2000000,
        "audioBitrate": 128,
      },

      "127": {
        "mimeType": r#"audio/ts; codecs="aac""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 96,
      },

      "128": {
        "mimeType": r#"audio/ts; codecs="aac""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 96,
      },

      "132": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "240p",
        "bitrate": 150000,
        "audioBitrate": 48,
      },

      "133": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "240p",
        "bitrate": 200000,
        "audioBitrate": null,
      },

      "134": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "360p",
        "bitrate": 300000,
        "audioBitrate": null,
      },

      "135": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "480p",
        "bitrate": 500000,
        "audioBitrate": null,
      },

      "136": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "720p",
        "bitrate": 1000000,
        "audioBitrate": null,
      },

      "137": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "1080p",
        "bitrate": 2500000,
        "audioBitrate": null,
      },

      "138": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "4320p",
        "bitrate": 13500000,
        "audioBitrate": null,
      },

      "139": {
        "mimeType": r#"audio/mp4; codecs="aac""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 48,
      },

      "140": {
        "mimeType": r#"audio/m4a; codecs="aac""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 128,
      },

      "141": {
        "mimeType": r#"audio/mp4; codecs="aac""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 256,
      },

      "151": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 50000,
        "audioBitrate": 24,
      },

      "160": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "144p",
        "bitrate": 100000,
        "audioBitrate": null,
      },

      "171": {
        "mimeType": r#"audio/webm; codecs="vorbis""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 128,
      },

      "172": {
        "mimeType": r#"audio/webm; codecs="vorbis""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 192,
      },

      "242": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "240p",
        "bitrate": 100000,
        "audioBitrate": null,
      },

      "243": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "360p",
        "bitrate": 250000,
        "audioBitrate": null,
      },

      "244": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "480p",
        "bitrate": 500000,
        "audioBitrate": null,
      },

      "247": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "720p",
        "bitrate": 700000,
        "audioBitrate": null,
      },

      "248": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1080p",
        "bitrate": 1500000,
        "audioBitrate": null,
      },

      "249": {
        "mimeType": r#"audio/webm; codecs="opus""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 48,
      },

      "250": {
        "mimeType": r#"audio/webm; codecs="opus""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 64,
      },

      "251": {
        "mimeType": r#"audio/webm; codecs="opus""#,
        "qualityLabel": null,
        "bitrate": null,
        "audioBitrate": 160,
      },

      "264": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "1440p",
        "bitrate": 4000000,
        "audioBitrate": null,
      },

      "266": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "2160p",
        "bitrate": 12500000,
        "audioBitrate": null,
      },

      "271": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1440p",
        "bitrate": 9000000,
        "audioBitrate": null,
      },

      "272": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "4320p",
        "bitrate": 20000000,
        "audioBitrate": null,
      },

      "278": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "144p 30fps",
        "bitrate": 80000,
        "audioBitrate": null,
      },

      "298": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "720p",
        "bitrate": 3000000,
        "audioBitrate": null,
      },

      "299": {
        "mimeType": r#"video/mp4; codecs="H.264""#,
        "qualityLabel": "1080p",
        "bitrate": 5500000,
        "audioBitrate": null,
      },

      "300": {
        "mimeType": r#"video/ts; codecs="H.264, aac""#,
        "qualityLabel": "720p",
        "bitrate": 1318000,
        "audioBitrate": 48,
      },

      "302": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "720p HFR",
        "bitrate": 2500000,
        "audioBitrate": null,
      },

      "303": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1080p HFR",
        "bitrate": 5000000,
        "audioBitrate": null,
      },

      "308": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1440p HFR",
        "bitrate": 10000000,
        "audioBitrate": null,
      },

      "313": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "2160p",
        "bitrate": 13000000,
        "audioBitrate": null,
      },

      "315": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "2160p HFR",
        "bitrate": 20000000,
        "audioBitrate": null,
      },

      "330": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "144p HDR, HFR",
        "bitrate": 80000,
        "audioBitrate": null,
      },

      "331": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "240p HDR, HFR",
        "bitrate": 100000,
        "audioBitrate": null,
      },

      "332": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "360p HDR, HFR",
        "bitrate": 250000,
        "audioBitrate": null,
      },

      "333": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "240p HDR, HFR",
        "bitrate": 500000,
        "audioBitrate": null,
      },

      "334": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "720p HDR, HFR",
        "bitrate": 1000000,
        "audioBitrate": null,
      },

      "335": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1080p HDR, HFR",
        "bitrate": 1500000,
        "audioBitrate": null,
      },

      "336": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "1440p HDR, HFR",
        "bitrate": 5000000,
        "audioBitrate": null,
      },

      "337": {
        "mimeType": r#"video/webm; codecs="VP9""#,
        "qualityLabel": "2160p HDR, HFR",
        "bitrate": 12000000,
        "audioBitrate": null,
      },
    })
});
