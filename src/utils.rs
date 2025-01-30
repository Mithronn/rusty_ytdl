use boa_engine::{Context, Source};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cmp::{min, Ordering},
    collections::HashMap,
    net::IpAddr,
};
use tokio::sync::RwLock;
use urlencoding::decode;

use crate::{
    constants::{
        AGE_RESTRICTED_URLS, AUDIO_ENCODING_RANKS, BASE_URL, FORMATS, IPV6_REGEX, PARSE_INT_REGEX,
        VALID_QUERY_DOMAINS, VIDEO_ENCODING_RANKS,
    },
    info_extras::{get_author, get_chapters, get_dislikes, get_likes, get_storyboards},
    structs::{
        Embed, PlayerResponse, StreamingDataFormat, StringUtils, VideoDetails, VideoError,
        VideoFormat, VideoOptions, VideoQuality, VideoSearchOptions, YTConfig,
    },
};

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn get_html5player(body: &str) -> Option<String> {
    static HTML5PLAYER_RES: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"<script\s+src="([^"]+)"(?:\s+type="text\\//javascript")?\s+name="player_ias\\//base"\s*>|"jsUrl":"([^"]+)""#).unwrap()
    });

    let caps = HTML5PLAYER_RES.captures(body)?;
    caps.get(2)
        .or_else(|| caps.get(3))
        .map(|cap| cap.as_str().to_string())
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn parse_video_formats(
    info: &PlayerResponse,
    format_functions: Vec<(String, String)>,
) -> Option<Vec<VideoFormat>> {
    if let Some(streaming_data) = info.streaming_data.as_ref() {
        let formats = streaming_data.formats.clone().unwrap_or_default();
        let adaptive_formats = streaming_data.adaptive_formats.clone().unwrap_or_default();

        let mut formats: Vec<StreamingDataFormat> =
            formats.into_iter().chain(adaptive_formats).collect();

        let mut n_transform_cache: HashMap<String, String> = HashMap::new();
        let mut cipher_cache: Option<(String, Context)> = None;

        let well_formated_formats: Vec<VideoFormat> = formats
            .iter_mut()
            .filter(|format| format.mime_type.is_some())
            .map(|format| {
                let mut video_format = VideoFormat::from(format.clone());
                video_format.url = set_download_url(
                    format,
                    format_functions.clone(),
                    &mut n_transform_cache,
                    &mut cipher_cache,
                );
                add_format_meta(&mut video_format);
                video_format
            })
            .collect();

        return Some(well_formated_formats);
    }

    None
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn parse_live_video_formats(unformated_formats: Vec<(String, String)>) -> Vec<VideoFormat> {
    let formats: Vec<VideoFormat> = unformated_formats
        .into_iter()
        .filter_map(|(itag, url)| {
            FORMATS.get(&itag as &str).map(|static_format| {
                let streaming_data_format = StreamingDataFormat {
                    itag: Some(itag.parse::<u64>().unwrap_or(0)),
                    mime_type: Some(static_format.mime_type.clone()),
                    bitrate: static_format.bitrate,
                    quality_label: static_format.quality_label.clone(),
                    audio_bitrate: static_format.audio_bitrate,
                    url: Some(url),
                    ..Default::default()
                };

                let mut format = VideoFormat::from(streaming_data_format);
                add_format_meta(&mut format);
                format
            })
        })
        .collect();

    formats
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn add_format_meta(format: &mut VideoFormat) {
    static REGEX_IS_LIVE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\bsource[/=]yt_live_broadcast\b").unwrap());
    static REGEX_IS_HLS: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"/manifest/hls_(variant|playlist)/").unwrap());
    static REGEX_IS_DASHMPD: Lazy<Regex> = Lazy::new(|| Regex::new(r"/manifest/dash/").unwrap());

    if format.quality_label.is_some() {
        format.has_video = true;
    }

    if format.audio_bitrate.is_some() || format.audio_quality.is_some() {
        format.has_audio = true;
    }

    if REGEX_IS_LIVE.is_match(&format.url) {
        format.is_live = true;
    }

    if REGEX_IS_HLS.is_match(&format.url) {
        format.is_hls = true;
    }

    if REGEX_IS_DASHMPD.is_match(&format.url) {
        format.is_dash_mpd = true;
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn filter_formats(formats: &mut Vec<VideoFormat>, options: &VideoSearchOptions) {
    match options {
        VideoSearchOptions::Audio => {
            formats.retain(|x| (!x.has_video && x.has_audio) || x.is_live);
        }
        VideoSearchOptions::Video => {
            formats.retain(|x| (x.has_video && !x.has_audio) || x.is_live);
        }
        VideoSearchOptions::Custom(func) => {
            formats.retain(|x| func(x) || x.is_live);
        }
        _ => {
            formats.retain(|x| (x.has_video && x.has_audio) || x.is_live);
        }
    }
}

/// Try to get format with [`VideoOptions`] filter
#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn choose_format<'a>(
    formats: &'a [VideoFormat],
    options: &'a VideoOptions,
) -> Result<VideoFormat, VideoError> {
    let filter = &options.filter;
    let mut formats = formats.to_owned();

    filter_formats(&mut formats, filter);

    if formats.iter().any(|x| x.is_hls) {
        formats.retain(|fmt| (fmt.is_hls) || !(fmt.is_live));
    }

    formats.sort_by(sort_formats);
    match &options.quality {
        VideoQuality::Highest => {
            filter_formats(&mut formats, filter);

            let return_format = formats.first().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::Lowest => {
            filter_formats(&mut formats, filter);

            let return_format = formats.last().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::HighestAudio => {
            filter_formats(&mut formats, &VideoSearchOptions::Audio);
            formats.sort_by(sort_formats_by_audio);

            let return_format = formats.first().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::LowestAudio => {
            filter_formats(&mut formats, &VideoSearchOptions::Audio);

            formats.sort_by(sort_formats_by_audio);

            let return_format = formats.last().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::HighestVideo => {
            filter_formats(&mut formats, &VideoSearchOptions::Video);
            formats.sort_by(sort_formats_by_video);

            let return_format = formats.first().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::LowestVideo => {
            filter_formats(&mut formats, &VideoSearchOptions::Video);

            formats.sort_by(sort_formats_by_video);

            let return_format = formats.last().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
        VideoQuality::Custom(filter, func) => {
            filter_formats(&mut formats, filter);

            formats.sort_by(|x, y| func(x, y));

            let return_format = formats.first().ok_or(VideoError::FormatNotFound)?;

            Ok(return_format.clone())
        }
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn sort_formats_by<F>(a: &VideoFormat, b: &VideoFormat, sort_by: &[F]) -> Ordering
where
    F: Fn(&VideoFormat) -> i32,
{
    sort_by
        .iter()
        .map(|func| func(b).cmp(&func(a)))
        .find(|&order| order != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn sort_formats_by_video(a: &VideoFormat, b: &VideoFormat) -> Ordering {
    sort_formats_by(
        a,
        b,
        [
            |form: &VideoFormat| {
                let quality_label = form.quality_label.clone().unwrap_or("".to_string());

                let quality_label = PARSE_INT_REGEX
                    .captures(&quality_label)
                    .and_then(|x| x.get(0))
                    .map(|x| x.as_str())
                    .and_then(|x| x.parse::<i32>().ok())
                    .unwrap_or(0i32);

                quality_label
            },
            |form: &VideoFormat| form.bitrate as i32,
            // getVideoEncodingRank,
            |form: &VideoFormat| {
                let index = VIDEO_ENCODING_RANKS
                    .iter()
                    .position(|enc| form.mime_type.codecs.join(", ").contains(enc))
                    .map(|x| x as i32)
                    .unwrap_or(-1);

                index
            },
        ]
        .as_ref(),
    )
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn sort_formats_by_audio(a: &VideoFormat, b: &VideoFormat) -> Ordering {
    sort_formats_by(
        a,
        b,
        [
            |form: &VideoFormat| form.audio_bitrate.unwrap_or(0) as i32,
            // getAudioEncodingRank,
            |form: &VideoFormat| {
                let index = AUDIO_ENCODING_RANKS
                    .iter()
                    .position(|enc| form.mime_type.codecs.join(", ").contains(enc))
                    .map(|x| x as i32)
                    .unwrap_or(-1);

                index
            },
        ]
        .as_ref(),
    )
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn sort_formats(a: &VideoFormat, b: &VideoFormat) -> Ordering {
    sort_formats_by(
        a,
        b,
        [
            // Formats with both video and audio are ranked highest.
            |form: &VideoFormat| form.is_hls as i32,
            |form: &VideoFormat| form.is_dash_mpd as i32,
            |form: &VideoFormat| (form.has_video && form.has_audio) as i32,
            |form: &VideoFormat| form.has_video as i32,
            |form: &VideoFormat| {
                (form
                    .content_length
                    .clone()
                    .unwrap_or("0".to_string())
                    .parse::<u64>()
                    .unwrap_or(0)
                    > 0) as i32
            },
            |form: &VideoFormat| {
                let quality_label = form.quality_label.clone().unwrap_or("".to_string());

                let quality_label = PARSE_INT_REGEX
                    .captures(&quality_label)
                    .and_then(|x| x.get(0))
                    .map(|x| x.as_str())
                    .and_then(|x| x.parse::<i32>().ok())
                    .unwrap_or(0i32);

                quality_label
            },
            |form: &VideoFormat| form.bitrate as i32,
            |form: &VideoFormat| form.audio_bitrate.unwrap_or(0) as i32,
            // getVideoEncodingRank,
            |form: &VideoFormat| {
                let index = VIDEO_ENCODING_RANKS
                    .iter()
                    .position(|enc| form.mime_type.codecs.join(", ").contains(enc))
                    .map(|x| x as i32)
                    .unwrap_or(-1);

                index
            },
            // getAudioEncodingRank,
            |form: &VideoFormat| {
                let index = AUDIO_ENCODING_RANKS
                    .iter()
                    .position(|enc| form.mime_type.codecs.join(", ").contains(enc))
                    .map(|x| x as i32)
                    .unwrap_or(-1);

                index
            },
        ]
        .as_ref(),
    )
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn set_download_url(
    format: &mut StreamingDataFormat,
    functions: Vec<(String, String)>,
    n_transform_cache: &mut HashMap<String, String>,
    cipher_cache: &mut Option<(String, Context)>,
) -> String {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Query {
        n: String,
        url: String,
        s: String,
        sp: String,
    }

    let empty_script: (&str, &str) = ("", "");
    let decipher_script_string = functions
        .first()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .unwrap_or(empty_script);
    let n_transform_script_string = functions
        .get(1)
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .unwrap_or(empty_script);

    if let Some(url) = format.url.as_ref() {
        format.url = Some(ncode(url, n_transform_script_string, n_transform_cache));
    } else {
        let url = format
            .signature_cipher
            .clone()
            .unwrap_or(format.cipher.clone().unwrap_or_default());

        format.url = Some(ncode(
            decipher(&url, decipher_script_string, cipher_cache).as_str(),
            n_transform_script_string,
            n_transform_cache,
        ));
    }

    format.url.clone().unwrap_or("".to_string())
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
fn decipher(
    url: &str,
    decipher_script_string: (&str, &str),
    cipher_cache: &mut Option<(String, Context)>,
) -> String {
    let args: serde_json::value::Map<String, serde_json::Value> = {
        #[cfg(feature = "performance_analysis")]
        let _guard = flame::start_guard("serde_qs::from_str");
        serde_qs::from_str(url).unwrap()
    };

    let get_url_string = || {
        args.get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(url)
            .to_string()
    };

    if args.get("s").is_none() || decipher_script_string.1.is_empty() {
        return get_url_string();
    }

    let context = match cipher_cache {
        Some((cache_key, context)) if cache_key == decipher_script_string.1 => context,
        _ => {
            #[cfg(feature = "performance_analysis")]
            let _guard = flame::start_guard("build engine");
            let mut context = Context::default();
            if context
                .eval(Source::from_bytes(decipher_script_string.1))
                .is_err()
            {
                return get_url_string();
            }
            *cipher_cache = Some((decipher_script_string.1.to_string(), context));
            &mut cipher_cache.as_mut().unwrap().1
        }
    };

    let result = {
        #[cfg(feature = "performance_analysis")]
        let _guard = flame::start_guard("execute engine");
        context.eval(Source::from_bytes(&format!(
            r#"{func_name}("{args}")"#,
            func_name = decipher_script_string.0,
            args = args
                .get("s")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        )))
    };

    let result = match result {
        Ok(res) => res
            .as_string()
            .and_then(|s| s.to_std_string().ok())
            .unwrap_or_else(get_url_string),
        Err(_) => return get_url_string(),
    };

    let mut return_url = match args
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| Err(url::ParseError::EmptyHost), url::Url::parse)
    {
        Ok(url) => url,
        Err(_) => return get_url_string(),
    };
    let query_name = args
        .get("sp")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("signature");

    let mut query: Vec<(String, String)> = return_url
        .query_pairs()
        .map(|(name, value)| {
            if name == query_name {
                (name.into_owned(), result.clone())
            } else {
                (name.into_owned(), value.into_owned())
            }
        })
        .collect();

    if !return_url.query_pairs().any(|(name, _)| name == query_name) {
        query.push((query_name.to_string(), result));
    }

    return_url.query_pairs_mut().clear().extend_pairs(query);

    return_url.to_string()
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
fn ncode(
    url: &str,
    n_transform_script_string: (&str, &str),
    n_transfrom_cache: &mut HashMap<String, String>,
) -> String {
    let components: serde_json::value::Map<String, serde_json::Value> =
        serde_qs::from_str(&decode(url).unwrap_or(Cow::Borrowed(url))).unwrap_or_default();

    let n_transform_value = match components.get("n").and_then(serde_json::Value::as_str) {
        Some(val) if !n_transform_script_string.1.is_empty() => val,
        _ => return url.to_string(),
    };

    if let Some(result) = n_transfrom_cache.get(n_transform_value) {
        return update_url_with_n(url, result);
    }

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    fn create_transform_script(script: &str) -> Option<Context> {
        let mut context = Context::default();
        context.eval(Source::from_bytes(script)).ok()?;
        Some(context)
    }

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    fn execute_transform_script(
        context: &mut Context,
        func_name: &str,
        n_transform_value: &str,
    ) -> Option<String> {
        context
            .eval(Source::from_bytes(&format!(
                r#"{func_name}("{n_transform_value}")"#,
                func_name = func_name,
                n_transform_value = n_transform_value
            )))
            .ok()
            .and_then(|result| {
                result
                    .as_string()
                    .map(|js_str| js_str.to_std_string().unwrap_or_default())
            })
    }

    let mut context = match create_transform_script(n_transform_script_string.1) {
        Some(res) => res,
        None => return url.to_string(),
    };

    let result = match execute_transform_script(
        &mut context,
        n_transform_script_string.0,
        n_transform_value,
    ) {
        Some(res) => res,
        None => return url.to_string(),
    };

    n_transfrom_cache.insert(n_transform_value.to_owned(), result.clone());

    fn update_url_with_n(url: &str, n_value: &str) -> String {
        let return_url = url::Url::parse(url);
        if let Ok(mut return_url) = return_url {
            let query: Vec<(String, String)> = return_url
                .query_pairs()
                .map(|(name, value)| {
                    if name == "n" {
                        (name.into_owned(), n_value.to_string())
                    } else {
                        (name.into_owned(), value.into_owned())
                    }
                })
                .collect();

            return_url.query_pairs_mut().clear().extend_pairs(query);

            return_url.to_string()
        } else {
            url.to_string()
        }
    }

    update_url_with_n(url, &result)
}

/// Excavate video id from URLs or id with Regex
#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn get_video_id(url: &str) -> Option<String> {
    static URL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^https?://").unwrap());

    if validate_id(url.to_string()) {
        Some(url.to_string())
    } else if URL_REGEX.is_match(url.trim()) {
        get_url_video_id(url)
    } else {
        None
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn validate_id(id: String) -> bool {
    static ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9-_]{11}$").unwrap());
    ID_REGEX.is_match(id.trim())
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
fn get_url_video_id(url: &str) -> Option<String> {
    static VALID_PATH_DOMAINS: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?m)(?:^|\W)(?:youtube(?:-nocookie)?\.com/(?:.*[?&]v=|v/|shorts/|e(?:mbed)?/|[^/]+/.+/)|youtu\.be/)([\w-]+)")
        .unwrap()
    });

    let parsed = url::Url::parse(url.trim()).ok()?;

    if let Some(id) = parsed.query_pairs().find_map(|(key, value)| {
        if key == "v" {
            Some(value.to_string())
        } else {
            None
        }
    }) {
        return Some(id).filter(|id| {
            let return_id = id.substring(0, 11);
            validate_id(return_id.into())
        });
    }

    if VALID_PATH_DOMAINS.is_match(url.trim()) {
        if let Some(captures) = VALID_PATH_DOMAINS.captures(url.trim()) {
            if let Some(id) = captures.get(1).map(|m| m.as_str().to_string()) {
                return Some(id).filter(|id| {
                    let return_id = id.substring(0, 11);
                    validate_id(return_id.into())
                });
            }
        }
    }

    // Check if the host is valid
    if parsed.host_str().is_some()
        && VALID_QUERY_DOMAINS
            .iter()
            .any(|domain| domain == &parsed.host_str().unwrap_or(""))
    {
        return None;
    }

    None
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn get_text(obj: &serde_json::Value) -> &serde_json::Value {
    if !obj["runs"].is_null() {
        &obj["runs"][0]["text"]
    } else {
        &obj["simpleText"]
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_live(player_response: &PlayerResponse) -> bool {
    let video_details = player_response.video_details.as_ref();

    video_details
        .as_ref()
        .and_then(|x| x.is_live_content)
        .unwrap_or(false)
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn clean_video_details(
    initial_response: &serde_json::Value,
    player_response: &PlayerResponse,
    media: serde_json::Value,
    id: String,
) -> VideoDetails {
    let data = player_response
        .micro_format
        .as_ref()
        .and_then(|x| x.player_micro_format_renderer.as_ref());
    let video_details = player_response.video_details.as_ref();

    VideoDetails {
        author: get_author(initial_response, player_response),
        age_restricted: is_age_restricted(&media),

        likes: get_likes(initial_response),
        dislikes: get_dislikes(initial_response),

        video_url: format!("{BASE_URL}{id}"),
        storyboards: get_storyboards(player_response).unwrap_or_default(),
        chapters: get_chapters(initial_response).unwrap_or_default(),

        embed: Embed {
            flash_secure_url: data
                .as_ref()
                .and_then(|x| x.embed.as_ref())
                .and_then(|x| x.flash_secure_url.clone())
                .unwrap_or("".to_string()),
            flash_url: data
                .as_ref()
                .and_then(|x| x.embed.as_ref())
                .and_then(|x| x.flash_url.clone())
                .unwrap_or("".to_string()),
            iframe_url: data
                .as_ref()
                .and_then(|x| x.embed.as_ref())
                .and_then(|x| x.iframe_url.clone())
                .unwrap_or("".to_string()),
            height: data
                .as_ref()
                .and_then(|x| x.embed.as_ref())
                .and_then(|x| x.height)
                .unwrap_or(0i32),
            width: data
                .as_ref()
                .and_then(|x| x.embed.as_ref())
                .and_then(|x| x.width)
                .unwrap_or(0i32),
        },
        title: if let Some(title) = video_details.as_ref().and_then(|x| x.title.clone()) {
            title
        } else {
            data.as_ref()
                .and_then(|x| x.title.as_ref())
                .and_then(|x| x.simple_text.clone())
                .unwrap_or("".to_string())
        },
        description: if let Some(description) = video_details
            .as_ref()
            .and_then(|x| x.short_description.clone())
        {
            description
        } else {
            data.as_ref()
                .and_then(|x| x.description.as_ref())
                .and_then(|x| x.simple_text.clone())
                .unwrap_or("".to_string())
        },
        length_seconds: if let Some(length_seconds) = video_details
            .as_ref()
            .and_then(|x| x.length_seconds.clone())
        {
            length_seconds
        } else {
            data.as_ref()
                .and_then(|x| x.length_seconds.clone())
                .unwrap_or("".to_string())
        },
        owner_profile_url: data
            .as_ref()
            .and_then(|x| x.owner_profile_url.clone())
            .unwrap_or("".to_string()),
        external_channel_id: data
            .as_ref()
            .and_then(|x| x.external_channel_id.clone())
            .unwrap_or("".to_string()),
        is_family_safe: data.as_ref().and_then(|x| x.is_family_safe).unwrap_or(true),
        available_countries: data
            .as_ref()
            .and_then(|x| x.available_countries.clone())
            .unwrap_or_default(),
        is_unlisted: data.as_ref().and_then(|x| x.is_unlisted).unwrap_or(false),
        has_ypc_metadata: data
            .as_ref()
            .and_then(|x| x.has_ypc_metadata)
            .unwrap_or(false),
        view_count: if let Some(view_count) =
            video_details.as_ref().and_then(|x| x.view_count.clone())
        {
            view_count
        } else {
            data.as_ref()
                .and_then(|x| x.view_count.clone())
                .unwrap_or("".to_string())
        },
        category: data
            .as_ref()
            .and_then(|x| x.category.clone())
            .unwrap_or("".to_string()),
        publish_date: data
            .as_ref()
            .and_then(|x| x.publish_date.clone())
            .unwrap_or("".to_string()),
        owner_channel_name: data
            .as_ref()
            .and_then(|x| x.owner_channel_name.clone())
            .unwrap_or("".to_string()),
        upload_date: data
            .as_ref()
            .and_then(|x| x.upload_date.clone())
            .unwrap_or("".to_string()),
        video_id: video_details
            .as_ref()
            .and_then(|x| x.video_id.clone())
            .unwrap_or("".to_string()),
        keywords: video_details
            .as_ref()
            .and_then(|x| x.keywords.clone())
            .unwrap_or_default(),
        channel_id: video_details
            .as_ref()
            .as_ref()
            .and_then(|x| x.channel_id.clone())
            .unwrap_or("".to_string()),
        is_owner_viewing: video_details
            .as_ref()
            .and_then(|x| x.is_owner_viewing)
            .unwrap_or(false),
        is_crawlable: video_details
            .as_ref()
            .and_then(|x| x.is_crawlable)
            .unwrap_or(true),
        allow_ratings: video_details
            .as_ref()
            .and_then(|x| x.allow_ratings)
            .unwrap_or(true),
        is_private: video_details
            .as_ref()
            .and_then(|x| x.is_private)
            .unwrap_or(false),
        is_unplugged_corpus: video_details
            .as_ref()
            .and_then(|x| x.is_unplugged_corpus)
            .unwrap_or(false),
        is_live_content: is_live(player_response),
        thumbnails: [
            video_details
                .as_ref()
                .and_then(|x| x.thumbnail.as_ref())
                .and_then(|x| x.thumbnails.clone())
                .unwrap_or_default(),
            data.as_ref()
                .and_then(|x| x.thumbnail.as_ref())
                .and_then(|x| x.thumbnails.clone())
                .unwrap_or_default(),
        ]
        .concat(),
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_verified(badges: &serde_json::Value) -> bool {
    badges
        .as_array()
        .map(|x| {
            let verified_index = x
                .iter()
                .position(|c| {
                    let json = serde_json::json!(c);
                    json["metadataBadgeRenderer"]["tooltip"] == "Verified"
                })
                .unwrap_or(usize::MAX);

            verified_index < usize::MAX
        })
        .unwrap_or(false)
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_age_restricted(media: &serde_json::Value) -> bool {
    if let Some(media_object) = media.as_object() {
        AGE_RESTRICTED_URLS.iter().any(|url| {
            media_object
                .values()
                .any(|value| value.as_str().map_or(false, |v| v.contains(url)))
        })
    } else {
        false
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_age_restricted_from_html(player_response: &PlayerResponse, html: &str) -> bool {
    if !player_response
        .micro_format
        .as_ref()
        .and_then(|x| x.player_micro_format_renderer.clone())
        .and_then(|x| x.is_family_safe)
        .unwrap_or(true)
    {
        return true;
    }

    let document = Html::parse_document(html);
    let metas_selector = Selector::parse("meta").unwrap();

    // <meta property="og:restrictions:age" content="18+">
    let og_restrictions_age = document
        .select(&metas_selector)
        .filter(|x| {
            x.attr("itemprop")
                .or(x.attr("name"))
                .or(x.attr("property"))
                .or(x.attr("id"))
                .or(x.attr("http-equiv"))
                == Some("og:restrictions:age")
        })
        .map(|x| x.attr("content").unwrap_or("").to_string())
        .next()
        .unwrap_or(String::from(""));

    // <meta itemprop="isFamilyFriendly" content="true">
    let is_family_friendly = document
        .select(&metas_selector)
        .filter(|x| {
            x.attr("itemprop")
                .or(x.attr("name"))
                .or(x.attr("property"))
                .or(x.attr("id"))
                .or(x.attr("http-equiv"))
                == Some("isFamilyFriendly")
        })
        .map(|x| x.attr("content").unwrap_or("").to_string())
        .next()
        .unwrap_or(String::from(""));

    is_family_friendly == "false" || og_restrictions_age == "18+"
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_rental(player_response: &PlayerResponse) -> bool {
    if player_response.playability_status.is_none() {
        return false;
    }

    player_response
        .playability_status
        .as_ref()
        .and_then(|x| x.status.clone())
        .map(|x| x == "UNPLAYABLE")
        .unwrap_or(false)
        && player_response
            .playability_status
            .as_ref()
            .and_then(|x| x.error_screen.clone())
            .and_then(|x| x.player_legacy_desktop_ypc_offer_renderer)
            .is_some()
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_not_yet_broadcasted(player_response: &PlayerResponse) -> bool {
    if player_response.playability_status.is_none() {
        return false;
    }

    player_response
        .playability_status
        .as_ref()
        .and_then(|x| x.status.clone())
        .map(|x| x == "LIVE_STREAM_OFFLINE")
        .unwrap_or(false)
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_play_error(player_response: &PlayerResponse, statuses: Vec<&str>) -> bool {
    let playability_status = player_response
        .playability_status
        .as_ref()
        .and_then(|x| x.status.clone());

    if let Some(playability_some) = playability_status {
        return statuses.contains(&playability_some.as_str());
    }

    false
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_player_response_error(
    player_response: &PlayerResponse,
    reasons: &[&str],
) -> Option<String> {
    if let Some(reason) = player_response
        .playability_status
        .as_ref()
        .and_then(|status| status.reason.as_deref())
    {
        if reasons.contains(&reason) {
            return Some(reason.to_string());
        }
    }

    None
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn is_private_video(player_response: &PlayerResponse) -> bool {
    player_response
        .playability_status
        .as_ref()
        .and_then(|x| x.status.clone())
        .map(|x| x == "LOGIN_REQUIRED")
        .unwrap_or(false)
}

pub fn get_ytconfig(html: &str) -> Result<YTConfig, VideoError> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"ytcfg\.set\((\{.*\})\);"#).unwrap());
    match PATTERN.captures(html) {
        Some(c) => Ok(
            serde_json::from_str::<YTConfig>(c.get(1).map_or("", |m| m.as_str()))
                .map_err(|_x| VideoError::VideoSourceNotFound)?,
        ),
        None => Err(VideoError::VideoSourceNotFound),
    }
}

pub fn get_visitor_data(html: &str) -> Result<String, VideoError> {
    static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#""visitorData":"([^"]+)""#).unwrap());

    match PATTERN.captures(html) {
        Some(c) => Ok(c.get(1).map_or("", |m| m.as_str()).to_string()),
        None => Err(VideoError::VideoSourceNotFound),
    }
}

type CacheFunctions = Lazy<RwLock<Option<(String, Vec<(String, String)>)>>>;
static FUNCTIONS: CacheFunctions = Lazy::new(|| RwLock::new(None));

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub async fn get_functions(
    html5player: impl Into<String>,
    client: &reqwest_middleware::ClientWithMiddleware,
) -> Result<Vec<(String, String)>, VideoError> {
    let mut url = url::Url::parse(BASE_URL).expect("IMPOSSIBLE");
    url.set_path(&html5player.into());
    url.query_pairs_mut().clear();

    let url = url.as_str();

    // println!("html5player url: {}", url);

    {
        // Check if an URL is already cached
        if let Some((cached_url, cached_functions)) = FUNCTIONS.read().await.as_ref() {
            // Check if the cache is the same as the URL
            if cached_url == url {
                return Ok(cached_functions.clone());
            }
        }
    }

    let response = get_html(client, url, None).await?;

    let functions = extract_functions(response);

    // Update the cache
    {
        *FUNCTIONS.write().await = Some((url.to_string(), functions.clone()));
    }

    Ok(functions)
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn extract_functions(body: String) -> Vec<(String, String)> {
    let mut functions: Vec<(String, String)> = vec![];

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    fn extract_manipulations(body: String, caller: &str) -> String {
        let function_name = between(caller, r#"a=a.split("");"#, ".");
        if function_name.is_empty() {
            return String::new();
        }

        let function_start = format!(r#"var {function_name}={{"#);
        let ndx = match body.find(function_start.as_str()) {
            Some(i) => i,
            None => return String::new(),
        };

        let sub_body = body.slice((ndx + function_start.len() - 1)..);

        let cut_after_sub_body = cut_after_js(sub_body).unwrap_or("null");

        let return_formatted_string = format!("var {function_name}={cut_after_sub_body}");

        return_formatted_string
    }

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    fn extract_decipher(body: String, functions: &mut Vec<(String, String)>) {
        let function_name = between(body.as_str(), r#"a.set("alr","yes");c&&(c="#, "(decodeURIC");
        // println!("decipher function name: {}", function_name);
        if !function_name.is_empty() {
            let function_start = format!("{function_name}=function(a)");
            let ndx = body.find(function_start.as_str());

            if let Some(ndx_some) = ndx {
                let sub_body = body.slice((ndx_some + function_start.len())..);

                let cut_after_sub_body = cut_after_js(sub_body).unwrap_or("{}");

                let mut function_body = format!("var {function_start}{cut_after_sub_body}");

                function_body = format!(
                    "{manipulated_body};{function_body};",
                    manipulated_body = extract_manipulations(body.clone(), function_body.as_str(),),
                );

                function_body.retain(|c| c != '\n');

                // println!("decipher function: {}", function_body);

                functions.push((function_name.to_string(), function_body));
            }
        }
    }

    #[cfg_attr(feature = "performance_analysis", flamer::flame)]
    fn extract_ncode(body: String, functions: &mut Vec<(String, String)>) {
        let mut function_name = between(body.as_str(), r#"c=a.get(b))&&(c="#, "(c)");

        let left_name = format!(
            "var {splitted_function_name}=[",
            splitted_function_name = function_name
                .split('[')
                .collect::<Vec<&str>>()
                .first()
                .unwrap_or(&"")
        );

        if function_name.contains('[') {
            function_name = between(body.as_str(), left_name.as_str(), "]");
        }

        if function_name.is_empty() {
            static FUNCTION_REGEX: Lazy<Regex> = Lazy::new(|| {
                Regex::new(
                    r"(?xs);\s*(?P<name>[a-zA-Z0-9_$]+)\s*=\s*function\([a-zA-Z0-9_$]+\)\s*\{",
                )
                .unwrap()
            });

            for caps in FUNCTION_REGEX.captures_iter(body.as_str()) {
                let name = caps.name("name").unwrap().as_str();

                let start_pos = caps.get(0).unwrap().end();
                if let Some(end_pos) = body[start_pos..].find("};") {
                    let function_body = &body[start_pos..start_pos + end_pos];

                    if function_body.contains("enhanced_except_") {
                        function_name = name;
                    }
                }
            }
        }

        // println!("ncode function name: {}", function_name);

        if !function_name.is_empty() {
            let function_start = format!("{function_name}=function(a)");
            let ndx = body.find(function_start.as_str());

            if let Some(ndx_some) = ndx {
                let sub_body = body.slice((ndx_some + function_start.len())..);

                let cut_after_sub_body = cut_after_js(sub_body).unwrap_or("{}");

                let mut function_body = format!("var {function_start}{cut_after_sub_body};");

                function_body.retain(|c| c != '\n');

                // println!("ncode function: {}", function_body);

                functions.push((function_name.to_string(), function_body));
            }
        }
    }

    extract_decipher(body.clone(), &mut functions);
    extract_ncode(body, &mut functions);

    functions
}

pub async fn get_html(
    client: &reqwest_middleware::ClientWithMiddleware,
    url: impl Into<String>,
    headers: Option<&reqwest::header::HeaderMap>,
) -> Result<String, VideoError> {
    let url = url.into();
    #[cfg(feature = "performance_analysis")]
    let _guard = flame::start_guard(format!("get_html {url}"));
    let request = if let Some(some_headers) = headers {
        client.get(url).headers(some_headers.clone())
    } else {
        client.get(url)
    }
    .send()
    .await
    .map_err(VideoError::ReqwestMiddleware)?;

    let response_first = request
        .text()
        .await
        .map_err(|_x| VideoError::BodyCannotParsed)?;

    Ok(response_first)
}

/// Try to generate IPv6 with custom valid block
/// # Example
/// ```ignore
/// let ipv6: std::net::IpAddr = get_random_v6_ip("2001:4::/48")?;
/// ```
#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn get_random_v6_ip(ip: impl Into<String>) -> Result<IpAddr, VideoError> {
    let ipv6_format: String = ip.into();

    if !IPV6_REGEX.is_match(&ipv6_format) {
        return Err(VideoError::InvalidIPv6Format);
    }

    let format_attr = ipv6_format.split('/').collect::<Vec<&str>>();
    let raw_addr = format_attr.first().ok_or(VideoError::InvalidIPv6Format)?;
    let raw_mask = format_attr.get(1).ok_or(VideoError::InvalidIPv6Format)?;

    let mut base_10_mask = raw_mask
        .parse::<u8>()
        .map_err(|_x| VideoError::InvalidIPv6Subnet)?;

    if !(24..=128).contains(&base_10_mask) {
        return Err(VideoError::InvalidIPv6Subnet);
    }

    let base_10_addr = normalize_ip(*raw_addr);
    let mut rng = rand::thread_rng();

    let mut random_addr = [0u16; 8];
    rng.fill(&mut random_addr);

    for (idx, random_item) in random_addr.iter_mut().enumerate() {
        // Calculate the amount of static bits
        let static_bits = min(base_10_mask, 16);
        base_10_mask -= static_bits;
        // Adjust the bitmask with the static_bits
        let mask = (0xffffu32 - ((2_u32.pow((16 - static_bits).into())) - 1)) as u16;
        // Combine base_10_addr and random_item
        let merged = (base_10_addr[idx] & mask) + (*random_item & (mask ^ 0xffff));

        *random_item = merged;
    }

    Ok(IpAddr::from(random_addr))
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn normalize_ip(ip: impl Into<String>) -> Vec<u16> {
    let ip: String = ip.into();
    let parts = ip
        .split("::")
        .map(|x| x.split(':').collect::<Vec<&str>>())
        .collect::<Vec<Vec<&str>>>();

    let empty_array = vec![];
    let part_start = parts.clone().first().unwrap_or(&empty_array).clone();
    let mut part_end = parts.clone().get(1).unwrap_or(&empty_array).clone();

    part_end.reverse();

    let mut full_ip: Vec<u16> = vec![0, 0, 0, 0, 0, 0, 0, 0];

    for i in 0..min(part_start.len(), 8) {
        full_ip[i] = u16::from_str_radix(part_start[i], 16).unwrap_or(0)
    }

    for i in 0..min(part_end.len(), 8) {
        full_ip[7 - i] = u16::from_str_radix(part_end[i], 16).unwrap_or(0)
    }

    full_ip
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn make_absolute_url(base: &str, url: &str) -> Result<url::Url, VideoError> {
    match url::Url::parse(url) {
        Ok(u) => Ok(u),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let base_url = url::Url::parse(base).map_err(VideoError::URLParseError)?;
            Ok(base_url.join(url)?)
        }
        Err(e) => Err(VideoError::URLParseError(e)),
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn time_to_ms(duration: &str) -> usize {
    let mut ms = 0;
    for (i, curr) in duration.split(':').rev().enumerate() {
        ms += curr.parse::<usize>().unwrap_or(0) * (u32::pow(60_u32, i as u32) as usize);
    }
    ms *= 1000;
    ms
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub fn parse_abbreviated_number(time_str: &str) -> usize {
    let replaced_string = time_str.replace(',', ".").replace(' ', "");
    static STRING_MATCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"([\d,.]+)([MK]?)").unwrap());

    if let Some(caps) = STRING_MATCH_REGEX.captures(replaced_string.as_str()) {
        let return_value = if caps.len() > 0 {
            let mut num;

            match caps.get(1) {
                Some(regex_match) => num = regex_match.as_str().parse::<f64>().unwrap_or(0f64),
                None => num = 0f64,
            }

            let multi = match caps.get(2) {
                Some(regex_match) => regex_match.as_str(),
                None => "",
            };

            match multi {
                "M" => num *= 1000000f64,
                "K" => num *= 1000f64,
                _ => {
                    // Do Nothing
                }
            }

            num = num.round();
            num as usize
        } else {
            return 0usize;
        };

        return_value
    } else {
        0usize
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
pub(crate) fn between<'a>(haystack: &'a str, left: &'a str, right: &'a str) -> &'a str {
    let pos: usize;

    if let Some(matched) = haystack.find(left) {
        pos = matched + left.len();
    } else {
        return "";
    }

    let remaining_haystack = &haystack[pos..];

    if let Some(matched) = remaining_haystack.find(right) {
        &haystack[pos..pos + matched]
    } else {
        ""
    }
}

#[cfg_attr(feature = "performance_analysis", flamer::flame)]
// This function uses a state machine architecture and takes around 10µs per request on Ryzen 9 5950XT
// The old function took around 30ms per request on the same CPU
pub fn cut_after_js(mixed_json: &str) -> Option<&str> {
    let bytes = mixed_json.as_bytes();

    // State:
    let mut index = 0;
    let mut nest = 0;
    let mut last_significant: Option<u8> = None;

    // Update function
    while nest > 0 || index == 0 {
        if index >= bytes.len() {
            return None;
        }
        let char = bytes[index];
        match char {
            // Update the nest
            b'{' | b'[' | b'(' => nest += 1,
            b'}' | b']' | b')' => nest -= 1,
            // Skip strings
            b'"' | b'\'' | b'`' => {
                index += 1;
                while bytes[index] != char {
                    if bytes[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
            }
            // Skip comments
            b'/' if bytes[index + 1] == b'*' => {
                index += 2;
                while !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                    index += 1;
                }
                index += 2;
                continue;
            }
            // Skip regexes
            b'/' if last_significant
                .as_ref()
                .map(|x| !x.is_ascii_alphanumeric())
                .unwrap_or(false) =>
            {
                index += 1;
                while bytes[index] != char {
                    if bytes[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
            }
            // Save the last significant character for the regex check
            a if !a.is_ascii_whitespace() => last_significant = Some(a),
            _ => (),
        }
        index += 1;
    }
    if index == 1 {
        return None;
    }
    Some(&mixed_json[0..index])
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cut_after_js() {
        assert_eq!(
            cut_after_js(r#"{"a": 1, "b": 1}"#).unwrap_or(""),
            r#"{"a": 1, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_works_with_simple_json");

        assert_eq!(
            cut_after_js(r#"{"a": 1, "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": 1, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_cut_extra_characters_after_json");

        assert_eq!(
            cut_after_js(r#"{"a": "}1", "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": "}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_double_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": '}1', "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": '}1', "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_single_quoted_string_constants");

        let str = "[-1816574795, '\",;/[;', function asdf() { a = 2/3; return a;}]";
        assert_eq!(
            cut_after_js(format!("{}abcd", str).as_str()).unwrap_or(""),
            str.to_string()
        );
        println!("[PASSED] test_tolerant_to_complex_single_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": `}1`, "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": `}1`, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_back_tick_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": "}1", "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": "}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1}abcd"#).unwrap_or(""),
            r#"{"a": "\"}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_escaped_quoting");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1, "c": /[0-9]}}\/}/}abcd"#).unwrap_or(""),
            r#"{"a": "\"}1", "b": 1, "c": /[0-9]}}\/}/}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_regexes");

        assert_eq!(
            cut_after_js(r#"{"a": [-1929233002,b,/,][}",],()}(\[)/,2070160835,1561177444]}abcd"#)
                .unwrap_or(""),
            r#"{"a": [-1929233002,b,/,][}",],()}(\[)/,2070160835,1561177444]}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_regexes_in_arrays");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1, "c": [4/6, /[0-9]}}\/}/]}abcd"#).unwrap_or(""),
            r#"{"a": "\"}1", "b": 1, "c": [4/6, /[0-9]}}\/}/]}"#.to_string()
        );
        println!("[PASSED] test_does_not_fail_for_division_followed_by_a_regex");

        assert_eq!(
            cut_after_js(r#"{"a": "\"1", "b": 1, "c": {"test": 1}}abcd"#).unwrap_or(""),
            r#"{"a": "\"1", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_nested_objects");

        let test_str = r#"{"a": "\"1", "b": 1, "c": () => { try { /* do sth */ } catch (e) { a = [2+3] }; return 5}}"#;
        assert_eq!(
            cut_after_js(format!("{}abcd", test_str).as_str()).unwrap_or(""),
            test_str.to_string()
        );
        println!("[PASSED] test_works_with_try_catch");

        assert_eq!(
            cut_after_js(r#"{"a": "\"фыва", "b": 1, "c": {"test": 1}}abcd"#).unwrap_or(""),
            r#"{"a": "\"фыва", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_utf");

        assert_eq!(
            cut_after_js(r#"{"a": "\\\\фыва", "b": 1, "c": {"test": 1}}abcd"#).unwrap_or(""),
            r#"{"a": "\\\\фыва", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_backslashes_in_string");

        assert_eq!(
            cut_after_js(r#"{"text": "\\\\"};"#).unwrap_or(""),
            r#"{"text": "\\\\"}"#.to_string()
        );
        println!("[PASSED] test_works_with_backslashes_towards_end_of_string");

        assert_eq!(
            cut_after_js(r#"[{"a": 1}, {"b": 2}]abcd"#).unwrap_or(""),
            r#"[{"a": 1}, {"b": 2}]"#.to_string()
        );
        println!("[PASSED] test_works_with_array_as_start");

        assert!(cut_after_js("abcd]}").is_none());
        println!("[PASSED] test_returns_error_when_not_beginning_with_bracket");

        assert!(cut_after_js(r#"{"a": 1,{ "b": 1}"#).is_none());
        println!("[PASSED] test_returns_error_when_missing_closing_bracket");
    }
}
