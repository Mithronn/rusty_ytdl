use bytes::Bytes;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::{io::AsyncWriteExt, process::Command};
use unicode_segmentation::UnicodeSegmentation;
use urlencoding::decode;

use crate::constants::{
    AGE_RESTRICTED_URLS, AUDIO_ENCODING_RANKS, BASE_URL, ESCAPING_SEQUENZES, IPV6_REGEX,
    PARSE_INT_REGEX, VALID_QUERY_DOMAINS, VIDEO_ENCODING_RANKS,
};
use crate::info_extras::{get_author, get_chapters, get_dislikes, get_likes, get_storyboards};
use crate::structs::{
    Embed, EscapeSequence, StringUtils, Thumbnail, VideoDetails, VideoError, VideoFormat,
    VideoOptions, VideoQuality, VideoSearchOptions,
};

#[cfg(feature = "ffmpeg")]
pub async fn ffmpeg_cmd_run(args: &Vec<String>, data: Bytes) -> Result<Bytes, VideoError> {
    use tokio::io::AsyncReadExt;

    let mut cmd = Command::new("ffmpeg");
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true);

    let mut process = cmd.spawn().map_err(|x| VideoError::FFmpeg(x.to_string()))?;
    let mut stdin = process
        .stdin
        .take()
        .ok_or(VideoError::FFmpeg("Failed to open stdin".to_string()))?;

    tokio::spawn(async move { stdin.write_all(&data).await });

    let output = process
        .wait_with_output()
        .await
        .map_err(|x| VideoError::FFmpeg(x.to_string()))?;

    Ok(Bytes::from(output.stdout))
}

#[allow(dead_code)]
pub fn get_cver(info: &serde_json::Value) -> &str {
    info.get("responseContext")
        .and_then(|x| x.get("serviceTrackingParams"))
        .unwrap()
        .as_array()
        .and_then(|x| {
            let index = x
                .iter()
                .position(|r| {
                    r.as_object()
                        .map(|c| c.get("service").unwrap().as_str().unwrap() == "CSI")
                        .unwrap_or(false)
                })
                .unwrap();
            x.get(index)
                .unwrap()
                .as_object()
                .and_then(|x| {
                    let second_array = x.get("params").unwrap().as_array().unwrap();
                    let second_index = second_array
                        .iter()
                        .position(|r| {
                            r.as_object()
                                .map(|c| c.get("key").unwrap().as_str().unwrap() == "cver")
                                .unwrap_or(false)
                        })
                        .unwrap();
                    second_array
                        .get(second_index)
                        .unwrap()
                        .as_object()
                        .unwrap()
                        .get("value")
                })
                .unwrap()
                .as_str()
        })
        .unwrap()
}

pub fn get_html5player(body: &str) -> Option<String> {
    let html5player_res = Regex::new(r#"<script\s+src="([^"]+)"(?:\s+type="text\\//javascript")?\s+name="player_ias\\//base"\s*>|"jsUrl":"([^"]+)""#).unwrap();
    let caps = html5player_res.captures(body).unwrap();
    match caps.get(2) {
        Some(caps) => Some(caps.as_str().to_string()),
        None => match caps.get(3) {
            Some(caps) => Some(caps.as_str().to_string()),
            None => Some(String::from("")),
        },
    }
}

pub fn parse_video_formats(
    info: &serde_json::Value,
    format_functions: Vec<(String, String)>,
) -> Option<Vec<VideoFormat>> {
    if info.as_object()?.contains_key("streamingData") {
        let formats = info
            .as_object()?
            .get("streamingData")
            .and_then(|x| x.get("formats"))?
            .as_array()?;
        let adaptive_formats = info
            .as_object()?
            .get("streamingData")
            .and_then(|x| x.get("adaptiveFormats"))?
            .as_array()?;
        let mut formats = [&formats[..], &adaptive_formats[..]].concat();

        let mut n_transform_cache: HashMap<String, String> = HashMap::new();

        for format in &mut formats {
            format.as_object_mut().map(|x| {
                let new_url = set_download_url(
                    &mut serde_json::json!(x),
                    format_functions.clone(),
                    &mut n_transform_cache,
                );

                // Delete unnecessary cipher, signatureCipher
                x.remove("signatureCipher");
                x.remove("cipher");

                x.insert("url".to_string(), new_url);

                // Add Video metaData
                add_format_meta(x);

                x
            });
        }

        let mut well_formated_formats: Vec<VideoFormat> = vec![];

        // Change formats type serde_json::Value to VideoFormat
        for format in formats.iter() {
            let well_formated_format: Result<VideoFormat, serde_json::Error> =
                serde_json::from_value(format.clone());

            if well_formated_format.is_err() {
                continue;
            }

            well_formated_formats
                .insert(well_formated_formats.len(), well_formated_format.unwrap());
        }

        Some(well_formated_formats)
    } else {
        None
    }
}

pub fn add_format_meta(format: &mut serde_json::Map<String, serde_json::Value>) {
    if format.contains_key("qualityLabel") {
        format.insert("hasVideo".to_owned(), serde_json::Value::Bool(true));
    } else {
        format.insert("hasVideo".to_owned(), serde_json::Value::Bool(false));
    }

    if format.contains_key("audioBitrate") || format.contains_key("audioQuality") {
        format.insert("hasAudio".to_owned(), serde_json::Value::Bool(true));
    } else {
        format.insert("hasAudio".to_owned(), serde_json::Value::Bool(false));
    }

    let regex_is_live = Regex::new(r"\bsource[/=]yt_live_broadcast\b").unwrap();
    let regex_is_hls = Regex::new(r"/manifest/hls_(variant|playlist)/").unwrap();
    let regex_is_dashmpd = Regex::new(r"/manifest/dash/").unwrap();

    format.insert(
        "isLive".to_string(),
        serde_json::Value::Bool(
            regex_is_live.is_match(format.get("url").and_then(|x| x.as_str()).unwrap_or("")),
        ),
    );

    format.insert(
        "isHLS".to_string(),
        serde_json::Value::Bool(
            regex_is_hls.is_match(format.get("url").and_then(|x| x.as_str()).unwrap_or("")),
        ),
    );

    format.insert(
        "isDashMPD".to_string(),
        serde_json::Value::Bool(
            regex_is_dashmpd.is_match(format.get("url").and_then(|x| x.as_str()).unwrap_or("")),
        ),
    );
}

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

pub fn sort_formats_by<F>(a: &VideoFormat, b: &VideoFormat, sort_by: Vec<F>) -> std::cmp::Ordering
where
    F: Fn(&VideoFormat) -> i32,
{
    let mut res = std::cmp::Ordering::Equal;

    for func in sort_by {
        res = func(b).cmp(&func(a));

        // Is not equal return order
        if res != std::cmp::Ordering::Equal {
            break;
        }
    }

    res
}

pub fn sort_formats_by_video(a: &VideoFormat, b: &VideoFormat) -> std::cmp::Ordering {
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
        .to_vec(),
    )
}

pub fn sort_formats_by_audio(a: &VideoFormat, b: &VideoFormat) -> std::cmp::Ordering {
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
        .to_vec(),
    )
}

pub fn sort_formats(a: &VideoFormat, b: &VideoFormat) -> std::cmp::Ordering {
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
        .to_vec(),
    )
}

pub fn set_download_url(
    format: &mut serde_json::Value,
    functions: Vec<(String, String)>,
    n_transform_cache: &mut HashMap<String, String>,
) -> serde_json::Value {
    let empty_string_serde_value = serde_json::json!("");
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Query {
        n: String,
        url: String,
        s: String,
        sp: String,
    }

    let empty_script = ("".to_string(), "".to_string());
    let decipher_script_string = functions.first().unwrap_or(&empty_script);
    let n_transform_script_string = functions.get(1).unwrap_or(&empty_script);

    // println!(
    //     "{:?}\n\n\n\n\n{:?}",
    //     decipher_script_string, n_transform_script_string
    // );

    fn decipher(url: &str, decipher_script_string: &(String, String)) -> String {
        let args: serde_json::value::Map<String, serde_json::Value> =
            serde_qs::from_str(url).unwrap();

        if args.get("s").is_none() || decipher_script_string.1.is_empty() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let mut context = boa_engine::Context::default();
        let decipher_script = context.eval(boa_engine::Source::from_bytes(
            decipher_script_string.1.as_str(),
        ));

        if decipher_script.is_err() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let result = context.eval(boa_engine::Source::from_bytes(&format!(
            r#"{func_name}("{args}")"#,
            func_name = decipher_script_string.0.as_str(),
            args = args.get("s").and_then(|x| x.as_str()).unwrap_or("")
        )));

        if result.is_err() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let is_result_string = result.as_ref().unwrap().as_string();

        if is_result_string.is_none() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let convert_result_to_rust_string = is_result_string.unwrap().to_std_string();

        if convert_result_to_rust_string.is_err() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let result = convert_result_to_rust_string.unwrap();

        // println!(
        //     "Decipher: {:?} {:?}",
        //     args.get("s").and_then(|x| x.as_str()).unwrap_or(""),
        //     result
        // );

        let return_url = url::Url::parse(args.get("url").and_then(|x| x.as_str()).unwrap_or(""));

        if return_url.is_err() {
            if args.get("url").is_none() {
                return url.to_string();
            } else {
                let args_url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                return args_url.to_string();
            }
        }

        let mut return_url = return_url.unwrap();

        let query_name = if args.get("sp").is_some() {
            args.get("sp")
                .and_then(|x| x.as_str())
                .unwrap_or("signature")
        } else {
            "signature"
        };

        let mut query = return_url
            .query_pairs()
            .map(|(name, value)| {
                if name == query_name {
                    (name.into_owned(), result.to_string())
                } else {
                    (name.into_owned(), value.into_owned())
                }
            })
            .collect::<Vec<(String, String)>>();

        if !return_url.query_pairs().any(|(name, _)| name == query_name) {
            query.push((query_name.to_string(), result));
        }

        return_url.query_pairs_mut().clear().extend_pairs(&query);

        return_url.to_string()
    }

    fn ncode(
        url: &str,
        n_transform_script_string: &(String, String),
        n_transfrom_cache: &mut HashMap<String, String>,
    ) -> String {
        let components: serde_json::value::Map<String, serde_json::Value> =
            serde_qs::from_str(&decode(url).unwrap_or(std::borrow::Cow::Borrowed(url))).unwrap();

        if components.get("n").is_none()
            || components.get("n").and_then(|x| x.as_str()).is_none()
            || n_transform_script_string.1.is_empty()
        {
            return url.to_string();
        }

        let n_transform_result;

        let n_transform_value = components.get("n").and_then(|x| x.as_str()).unwrap_or("");

        // println!("{:?}", n_transfrom_cache);

        if let Some(&result) = n_transfrom_cache.get(n_transform_value).as_ref() {
            n_transform_result = result.clone();
        } else {
            let mut context = boa_engine::Context::default();
            let n_transform_script = context.eval(boa_engine::Source::from_bytes(
                n_transform_script_string.1.as_str(),
            ));

            if n_transform_script.is_err() {
                return url.to_string();
            }

            let result = context.eval(boa_engine::Source::from_bytes(&format!(
                r#"{func_name}("{args}")"#,
                func_name = n_transform_script_string.0.as_str(),
                args = n_transform_value
            )));

            if result.is_err() {
                return url.to_string();
            }

            let is_result_string = result.as_ref().unwrap().as_string();

            if is_result_string.is_none() {
                return url.to_string();
            }

            let convert_result_to_rust_string = is_result_string.unwrap().to_std_string();

            if convert_result_to_rust_string.is_err() {
                return url.to_string();
            }

            let result = convert_result_to_rust_string.unwrap();

            // println!("N Transform: {:?} {:?}", n_transform_value, result);

            n_transfrom_cache.insert(n_transform_value.to_owned(), result.clone());
            n_transform_result = result;
        }

        let return_url = url::Url::parse(url);

        if return_url.is_err() {
            return url.to_string();
        }

        let mut return_url = return_url.unwrap();

        let query = return_url
            .query_pairs()
            .map(|(name, value)| {
                if name == "n" {
                    (name.into_owned(), n_transform_result.to_string())
                } else {
                    (name.into_owned(), value.into_owned())
                }
            })
            .collect::<Vec<(String, String)>>();

        return_url.query_pairs_mut().clear().extend_pairs(&query);

        return_url.to_string()
    }

    let return_format = format.as_object_mut().unwrap();

    let cipher = return_format.get("url").is_none();
    let url = return_format
        .get("url")
        .unwrap_or(
            return_format.get("signatureCipher").unwrap_or(
                return_format
                    .get("cipher")
                    .unwrap_or(&empty_string_serde_value),
            ),
        )
        .as_str()
        .unwrap_or("");

    if cipher {
        return_format.insert(
            "url".to_string(),
            serde_json::json!(&ncode(
                decipher(url, decipher_script_string).as_str(),
                n_transform_script_string,
                n_transform_cache
            )),
        );
    } else {
        return_format.insert(
            "url".to_string(),
            serde_json::json!(&ncode(url, n_transform_script_string, n_transform_cache)),
        );
    }

    // Delete unnecessary cipher, signatureCipher
    return_format.remove("signatureCipher");
    return_format.remove("cipher");

    let return_url = url::Url::parse(
        return_format
            .get("url")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
    )
    .unwrap();

    serde_json::json!(return_url.to_string())
}

/// Excavate video id from URLs or id with Regex
pub fn get_video_id(url: &str) -> Option<String> {
    let url_regex = Regex::new(r"^https?://").unwrap();

    if validate_id(url.to_string()) {
        Some(url.to_string())
    } else if url_regex.is_match(url.trim()) {
        get_url_video_id(url)
    } else {
        None
    }
}

pub fn validate_id(id: String) -> bool {
    let id_regex = Regex::new(r"^[a-zA-Z0-9-_]{11}$").unwrap();

    id_regex.is_match(id.trim())
}

fn get_url_video_id(url: &str) -> Option<String> {
    let valid_path_domains =
        // Regex::new(r"^https?:\\//\\//(youtu\.be\\//|(www\.)?youtube\.com\\//(embed|v|shorts)\\//)")
        //     .unwrap();
        Regex::new(r"(?m)(?:^|\W)(?:youtube(?:-nocookie)?\.com/(?:.*[?&]v=|v/|shorts/|e(?:mbed)?/|[^/]+/.+/)|youtu\.be/)([\w-]+)")
        .unwrap();

    let parsed_result = url::Url::parse(url.trim());

    if parsed_result.is_err() {
        return None;
    }

    let parsed = url::Url::parse(url.trim()).unwrap();

    let mut id: Option<String> = None;

    for value in parsed.query_pairs() {
        if value.0.to_string().as_str() == "v" {
            id = Some(value.1.to_string());
        }
    }

    if valid_path_domains.is_match(url.trim()) && id.is_none() {
        let captures = valid_path_domains.captures(url.trim());
        // println!("{:#?}", captures);
        if let Some(captures_some) = captures {
            let id_group = captures_some.get(1);
            if let Some(id_group_some) = id_group {
                id = Some(id_group_some.as_str().to_string());
            }
        }
    } else if url::Url::parse(url.trim()).unwrap().host_str().is_some()
        && !VALID_QUERY_DOMAINS
            .iter()
            .any(|domain| domain == &parsed.host_str().unwrap_or(""))
    {
        return None;
    }

    if let Some(id_some) = id {
        id = Some(id_some.substring(0, 11).to_string());

        if !validate_id(id.clone().unwrap()) {
            return None;
        }

        id
    } else {
        None
    }
}

pub fn get_text(obj: &serde_json::Value) -> &serde_json::Value {
    let null_referance = &serde_json::Value::Null;
    obj.as_object()
        .and_then(|x| {
            if x.contains_key("runs") {
                x.get("runs").and_then(|c| {
                    c.as_array()
                        .unwrap()
                        .first()
                        .and_then(|d| d.as_object().and_then(|f| f.get("text")))
                })
            } else {
                x.get("simpleText")
            }
        })
        .unwrap_or(null_referance)
}

pub fn clean_video_details(
    initial_response: &serde_json::Value,
    player_response: &serde_json::Value,
    media: serde_json::Value,
    id: String,
) -> VideoDetails {
    let empty_serde_object = serde_json::json!({});
    let empty_serde_vec: Vec<serde_json::Value> = vec![];
    let empty_serde_map = serde_json::Map::new();

    let mut data = player_response
        .get("microformat")
        .and_then(|x| x.get("playerMicroformatRenderer"))
        .unwrap_or(&empty_serde_object)
        .clone();
    let player_response_video_details = player_response
        .get("videoDetails")
        .unwrap_or(&empty_serde_object)
        .clone();

    // merge two json objects
    merge(&mut data, &player_response_video_details);

    let embed_object = data
        .get("embed")
        .and_then(|x| x.as_object())
        .unwrap_or(&empty_serde_map);
    VideoDetails {
        author: get_author(initial_response, player_response),
        age_restricted: is_age_restricted(&media),

        likes: get_likes(initial_response),
        dislikes: get_dislikes(initial_response),

        video_url: format!("{BASE_URL}{id}"),
        storyboards: get_storyboards(player_response).unwrap_or_default(),
        chapters: get_chapters(initial_response).unwrap_or_default(),

        embed: Embed {
            flash_secure_url: embed_object
                .get("flashSecureUrl")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            flash_url: embed_object
                .get("flashUrl")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            iframe_url: embed_object
                .get("iframeUrl")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            height: embed_object
                .get("height")
                .and_then(|x| {
                    if x.is_string() {
                        x.as_str().map(|x| match x.parse::<i64>() {
                            Ok(a) => a,
                            Err(_err) => 0i64,
                        })
                    } else {
                        x.as_i64()
                    }
                })
                .unwrap_or(0i64) as i32,
            width: embed_object
                .get("width")
                .and_then(|x| {
                    if x.is_string() {
                        x.as_str().map(|x| match x.parse::<i64>() {
                            Ok(a) => a,
                            Err(_err) => 0i64,
                        })
                    } else {
                        x.as_i64()
                    }
                })
                .unwrap_or(0i64) as i32,
        },
        title: data
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        description: if data.get("shortDescription").is_some() {
            data.get("shortDescription")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            get_text(data.get("description").unwrap_or(&empty_serde_object))
                .as_str()
                .unwrap_or("")
                .to_string()
        },
        length_seconds: data
            .get("lengthSeconds")
            .and_then(|x| x.as_str())
            .unwrap_or("0")
            .to_string(),
        owner_profile_url: data
            .get("ownerProfileUrl")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        external_channel_id: data
            .get("externalChannelId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        is_family_safe: data
            .get("isFamilySafe")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        available_countries: data
            .get("availableCountries")
            .and_then(|x| x.as_array())
            .unwrap_or(&empty_serde_vec)
            .iter()
            .map(|x| x.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>(),
        is_unlisted: data
            .get("isUnlisted")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        has_ypc_metadata: data
            .get("hasYpcMetadata")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        view_count: data
            .get("viewCount")
            .and_then(|x| x.as_str())
            .unwrap_or("0")
            .to_string(),
        category: data
            .get("category")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        publish_date: data
            .get("publishDate")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        owner_channel_name: data
            .get("ownerChannelName")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        upload_date: data
            .get("uploadDate")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        video_id: data
            .get("videoId")
            .and_then(|x| x.as_str())
            .unwrap_or("0")
            .to_string(),
        keywords: data
            .get("keywords")
            .and_then(|x| x.as_array())
            .unwrap_or(&empty_serde_vec)
            .iter()
            .map(|x| x.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>(),
        channel_id: data
            .get("channelId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        is_owner_viewing: data
            .get("isOwnerViewing")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        is_crawlable: data
            .get("isCrawlable")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        allow_ratings: data
            .get("allowRatings")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        is_private: data
            .get("isPrivate")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        is_unplugged_corpus: data
            .get("isUnpluggedCorpus")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        is_live_content: data
            .get("isLiveContent")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        thumbnails: data
            .get("thumbnail")
            .and_then(|x| x.get("thumbnails"))
            .and_then(|x| x.as_array())
            .unwrap_or(&empty_serde_vec)
            .iter()
            .map(|x| Thumbnail {
                width: x
                    .get("width")
                    .and_then(|x| {
                        if x.is_string() {
                            x.as_str().map(|x| match x.parse::<i64>() {
                                Ok(a) => a,
                                Err(_err) => 0i64,
                            })
                        } else {
                            x.as_i64()
                        }
                    })
                    .unwrap_or(0i64) as u64,
                height: x
                    .get("height")
                    .and_then(|x| {
                        if x.is_string() {
                            x.as_str().map(|x| match x.parse::<i64>() {
                                Ok(a) => a,
                                Err(_err) => 0i64,
                            })
                        } else {
                            x.as_i64()
                        }
                    })
                    .unwrap_or(0i64) as u64,
                url: x
                    .get("url")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect::<Vec<Thumbnail>>(),
    }
}

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

pub fn is_age_restricted(media: &serde_json::Value) -> bool {
    let mut age_restricted = false;
    if media.is_object() && media.as_object().is_some() {
        age_restricted = AGE_RESTRICTED_URLS.iter().any(|url| {
            media
                .as_object()
                .map(|x| {
                    let mut bool_vec: Vec<bool> = vec![];

                    for (_key, value) in x {
                        if let Some(value_some) = value.as_str() {
                            bool_vec.push(value_some.contains(url))
                        } else {
                            bool_vec.push(false);
                        }
                    }

                    bool_vec.iter().any(|v| v == &true)
                })
                .unwrap_or(false)
        })
    }

    age_restricted
}

pub fn is_rental(player_response: &serde_json::Value) -> bool {
    let playability = player_response.get("playabilityStatus");

    if playability.is_none() {
        return false;
    }

    playability
        .and_then(|x| x.get("status"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        == "UNPLAYABLE"
        && playability
            .and_then(|x| x.get("errorScreen"))
            .and_then(|x| x.get("playerLegacyDesktopYpcOfferRenderer"))
            .is_some()
}

pub fn is_not_yet_broadcasted(player_response: &serde_json::Value) -> bool {
    let playability = player_response.get("playabilityStatus");

    if playability.is_none() {
        return false;
    }

    playability
        .and_then(|x| x.get("status"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        == "LIVE_STREAM_OFFLINE"
}

pub fn is_play_error(player_response: &serde_json::Value, statuses: Vec<&str>) -> bool {
    let playability = player_response
        .get("playabilityStatus")
        .and_then(|x| x.get("status").and_then(|x| x.as_str()));

    if let Some(playability_some) = playability {
        return statuses.contains(&playability_some);
    }

    false
}

pub fn is_private_video(player_response: &serde_json::Value) -> bool {
    if player_response
        .get("playabilityStatus")
        .and_then(|x| x.get("status"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        == "LOGIN_REQUIRED"
    {
        return true;
    }

    false
}

pub async fn get_functions(
    html5player: impl Into<String>,
    client: &reqwest_middleware::ClientWithMiddleware,
) -> Result<Vec<(String, String)>, VideoError> {
    let mut url = url::Url::parse(BASE_URL).expect("IMPOSSIBLE");
    url.set_path(&html5player.into());
    url.query_pairs_mut().clear();

    let url = url.as_str();

    let response = get_html(client, url, None).await?;

    Ok(extract_functions(response))
}

pub fn extract_functions(body: String) -> Vec<(String, String)> {
    let mut functions: Vec<(String, String)> = vec![];

    // let mut cut_after_js_script =
    //     js_sandbox::Script::from_string(CUT_AFTER_JS).expect("cut_after_js function error");

    fn extract_manipulations(
        body: String,
        caller: &str,
        // cut_after_js_script: &mut js_sandbox::Script,
    ) -> String {
        let function_name = between(caller, r#"a=a.split("");"#, ".");
        if function_name.is_empty() {
            return String::new();
        }

        let function_start = format!(r#"var {function_name}={{"#);
        let ndx = body.find(function_start.as_str());

        if ndx.is_none() {
            return String::new();
        }

        let sub_body = body.slice((ndx.unwrap() + function_start.len() - 1)..);

        // let cut_after_sub_body = cut_after_js_script.call("cutAfterJS", (&sub_body,));
        // let cut_after_sub_body: String = cut_after_sub_body.unwrap_or(String::from("null"));

        let cut_after_sub_body = cut_after_js(sub_body).unwrap_or(String::from("null"));

        let return_formatted_string = format!("var {function_name}={cut_after_sub_body}");

        return_formatted_string
    }

    fn extract_decipher(
        body: String,
        functions: &mut Vec<(String, String)>,
        // cut_after_js_script: &mut js_sandbox::Script,
    ) {
        let function_name = between(body.as_str(), r#"a.set("alr","yes");c&&(c="#, "(decodeURIC");
        // println!("decipher function name: {}", function_name);
        if !function_name.is_empty() {
            let function_start = format!("{function_name}=function(a)");
            let ndx = body.find(function_start.as_str());

            if let Some(ndx_some) = ndx {
                let sub_body = body.slice((ndx_some + function_start.len())..);

                // let cut_after_sub_body = cut_after_js_script.call("cutAfterJS", (&sub_body,));
                // let cut_after_sub_body: String = cut_after_sub_body.unwrap_or(String::from("{}"));

                let cut_after_sub_body = cut_after_js(sub_body).unwrap_or(String::from("{}"));

                let mut function_body = format!("var {function_start}{cut_after_sub_body}");

                function_body = format!(
                    "{manipulated_body};{function_body};",
                    manipulated_body = extract_manipulations(
                        body.clone(),
                        function_body.as_str(),
                        // cut_after_js_script
                    ),
                );

                function_body.retain(|c| c != '\n');

                functions.push((function_name.to_string(), function_body));
            }
        }
    }

    fn extract_ncode(
        body: String,
        functions: &mut Vec<(String, String)>,
        // cut_after_js_script: &mut js_sandbox::Script,
    ) {
        let mut function_name = between(body.as_str(), r#"&&(b=a.get("n"))&&(b="#, "(b)");

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

        // println!("ncode function name: {}", function_name);

        if !function_name.is_empty() {
            let function_start = format!("{function_name}=function(a)");
            let ndx = body.find(function_start.as_str());

            if let Some(ndx_some) = ndx {
                let sub_body = body.slice((ndx_some + function_start.len())..);

                // let cut_after_sub_body = cut_after_js_script.call("cutAfterJS", (&sub_body,));
                // let cut_after_sub_body: String = cut_after_sub_body.unwrap_or(String::from("{}"));

                let cut_after_sub_body = cut_after_js(sub_body).unwrap_or(String::from("{}"));

                let mut function_body = format!("var {function_start}{cut_after_sub_body};");

                function_body.retain(|c| c != '\n');

                functions.push((function_name.to_string(), function_body));
            }
        }
    }

    extract_decipher(
        body.clone(),
        &mut functions, /*&mut cut_after_js_script*/
    );
    extract_ncode(body, &mut functions /*&mut cut_after_js_script*/);

    // println!("{:#?} {}", functions, functions.len());
    functions
}

pub async fn get_html(
    client: &reqwest_middleware::ClientWithMiddleware,
    url: impl Into<String>,
    headers: Option<&reqwest::header::HeaderMap>,
) -> Result<String, VideoError> {
    let request = if let Some(some_headers) = headers {
        client.get(url.into()).headers(some_headers.clone())
    } else {
        client.get(url.into())
    }
    .send()
    .await;

    if request.is_err() {
        return Err(VideoError::ReqwestMiddleware(request.err().unwrap()));
    }

    let response_first = request.unwrap().text().await;

    if response_first.is_err() {
        return Err(VideoError::BodyCannotParsed);
    }

    Ok(response_first.unwrap())
}

/// Try to generate IPv6 with custom valid block
/// # Example
/// ```ignore
/// let ipv6: std::net::IpAddr = get_random_v6_ip("2001:4::/48")?;
/// ```
pub fn get_random_v6_ip(ip: impl Into<String>) -> Result<std::net::IpAddr, VideoError> {
    let ipv6_format: String = ip.into();

    if !IPV6_REGEX.is_match(&ipv6_format) {
        return Err(VideoError::InvalidIPv6Format);
    }

    let format_attr = ipv6_format.split('/').collect::<Vec<&str>>();
    let raw_addr = format_attr.first();
    let raw_mask = format_attr.get(1);

    if raw_addr.is_none() || raw_mask.is_none() {
        return Err(VideoError::InvalidIPv6Format);
    }

    let raw_addr = raw_addr.unwrap();
    let raw_mask = raw_mask.unwrap();

    let base_10_mask = raw_mask.parse::<u8>();
    if base_10_mask.is_err() {
        return Err(VideoError::InvalidIPv6Subnet);
    }

    let mut base_10_mask = base_10_mask.unwrap();

    if !(24..=128).contains(&base_10_mask) {
        return Err(VideoError::InvalidIPv6Subnet);
    }

    let base_10_addr = normalize_ip(*raw_addr);
    let mut rng = rand::thread_rng();

    let mut random_addr = [0u16; 8];
    rng.fill(&mut random_addr);

    for (idx, random_item) in random_addr.iter_mut().enumerate() {
        // Calculate the amount of static bits
        let static_bits = std::cmp::min(base_10_mask, 16);
        base_10_mask -= static_bits;
        // Adjust the bitmask with the static_bits
        let mask = (0xffffu32 - ((2_u32.pow((16 - static_bits).into())) - 1)) as u16;
        // Combine base_10_addr and random_item
        let merged = (base_10_addr[idx] & mask) + (*random_item & (mask ^ 0xffff));

        *random_item = merged;
    }

    Ok(std::net::IpAddr::from(random_addr))
}

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

    for i in 0..std::cmp::min(part_start.len(), 8) {
        full_ip[i] = u16::from_str_radix(part_start[i], 16).unwrap_or(0)
    }

    for i in 0..std::cmp::min(part_end.len(), 8) {
        full_ip[7 - i] = u16::from_str_radix(part_end[i], 16).unwrap_or(0)
    }

    full_ip
}

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

pub fn time_to_ms(duration: &str) -> usize {
    let mut ms = 0;
    for (i, curr) in duration.split(':').rev().enumerate() {
        ms += curr.parse::<usize>().unwrap_or(0) * (u32::pow(60_u32, i as u32) as usize);
    }
    ms *= 1000;
    ms
}

pub fn parse_abbreviated_number(time_str: &str) -> usize {
    let replaced_string = time_str.replace(',', ".").replace(' ', "");
    let string_match_regex = Regex::new(r"([\d,.]+)([MK]?)").unwrap();
    // let mut return_value = 0usize;

    let caps = string_match_regex
        .captures(replaced_string.as_str())
        .unwrap();

    let return_value = if caps.len() > 0 {
        let mut num;

        match caps.get(1) {
            Some(regex_match) => num = regex_match.as_str().parse::<f32>().unwrap_or(0f32),
            None => num = 0f32,
        }

        let multi = match caps.get(2) {
            Some(regex_match) => regex_match.as_str(),
            None => "",
        };

        match multi {
            "M" => num *= 1000000f32,
            "K" => num *= 1000f32,
            _ => {
                // Do Nothing
            }
        }

        num = num.round();
        num as usize
    } else {
        0usize
    };

    return_value
}

pub fn merge(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (&mut serde_json::Value::Object(ref mut a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => {
            *a = b.clone();
        }
    }
}

// pub fn between<'a>(haystack: &'a str, left: &'a str, right: &'a str) -> &'a str {
//     let left_index = haystack.find(left);
//     if left_index.is_none() {
//         return "";
//     }

//     let mut pos = left_index.unwrap();
//     pos += left.len();

//     let mut return_str = haystack.slice(pos..);
//     let right_index = return_str.find(right);
//     if right_index.is_none() {
//         return "";
//     }

//     let second_pos = right_index.unwrap();

//     return_str = return_str.substring(0, second_pos);
//     return_str
// }

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

pub fn cut_after_js(mixed_json: &str) -> Option<String> {
    let (open, close) = match mixed_json.slice(0..=0) {
        "[" => ("[", "]"),
        "{" => ("{", "}"),
        _ => {
            return None;
        }
    };

    let mut is_escaped_object: Option<EscapeSequence> = None;

    // States if the current character is escaped or not
    let mut is_escaped = false;

    // Current open brackets to be closed
    let mut counter = 0;

    let mixed_json_unicode = mixed_json.graphemes(true).collect::<Vec<&str>>();
    for (i, value) in mixed_json_unicode.iter().enumerate() {
        let value = <&str>::clone(value);

        if !is_escaped
            && is_escaped_object.as_ref().is_some()
            && value
                == is_escaped_object
                    .as_ref()
                    .map(|x| x.end.as_str())
                    .unwrap_or("57")
        {
            is_escaped_object = None;
            continue;
        }

        if !is_escaped && is_escaped_object.is_none() {
            for escaped in ESCAPING_SEQUENZES.iter() {
                if value != escaped.start.as_str() {
                    continue;
                }

                let substring_start_number = if i <= 10 { 0usize } else { i - 10 };

                // println!(
                //     "regex test str: {}\nregex test str length: {}\ntest result: {}\nindex: {}\nindex - 10: {}\n",
                //     mixed_json.substring(substring_start_number, i),
                //     mixed_json.substring(substring_start_number, i).len(),
                //     escaped
                //         .start_prefix
                //         .as_ref()
                //         .map(|x| x.is_match(mixed_json.substring(substring_start_number, i)))
                //         .unwrap_or(false),
                //     i,
                //     (i as i32 - 10)
                // );

                if escaped.start_prefix.is_none()
                    || escaped
                        .start_prefix
                        .as_ref()
                        .map(|x| x.is_match(mixed_json.substring(substring_start_number, i)))
                        .unwrap_or(false)
                {
                    is_escaped_object = Some(escaped.clone());
                    break;
                }
            }

            if is_escaped_object.is_some() {
                continue;
            }
        }

        is_escaped = value == "\\" && !is_escaped;

        if is_escaped_object.is_some() {
            continue;
        }

        if value == open {
            counter += 1;
        } else if value == close {
            counter -= 1;
        }

        if counter == 0 {
            return Some(mixed_json.substring(0, i + 1).to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cut_after_js() {
        assert_eq!(
            cut_after_js(r#"{"a": 1, "b": 1}"#).unwrap_or("".to_string()),
            r#"{"a": 1, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_works_with_simple_json");

        assert_eq!(
            cut_after_js(r#"{"a": 1, "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": 1, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_cut_extra_characters_after_json");

        assert_eq!(
            cut_after_js(r#"{"a": "}1", "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": "}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_double_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": '}1', "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": '}1', "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_single_quoted_string_constants");

        let str = "[-1816574795, '\",;/[;', function asdf() { a = 2/3; return a;}]";
        assert_eq!(
            cut_after_js(format!("{}abcd", str).as_str()).unwrap_or("".to_string()),
            str.to_string()
        );
        println!("[PASSED] test_tolerant_to_complex_single_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": `}1`, "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": `}1`, "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_back_tick_quoted_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": "}1", "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": "}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_constants");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1}abcd"#).unwrap_or("".to_string()),
            r#"{"a": "\"}1", "b": 1}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_escaped_quoting");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1, "c": /[0-9]}}\/}/}abcd"#)
                .unwrap_or("".to_string()),
            r#"{"a": "\"}1", "b": 1, "c": /[0-9]}}\/}/}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_regexes");

        assert_eq!(
            cut_after_js(r#"{"a": [-1929233002,b,/,][}",],()}(\[)/,2070160835,1561177444]}abcd"#)
                .unwrap_or("".to_string()),
            r#"{"a": [-1929233002,b,/,][}",],()}(\[)/,2070160835,1561177444]}"#.to_string()
        );
        println!("[PASSED] test_tolerant_to_string_with_regexes_in_arrays");

        assert_eq!(
            cut_after_js(r#"{"a": "\"}1", "b": 1, "c": [4/6, /[0-9]}}\/}/]}abcd"#)
                .unwrap_or("".to_string()),
            r#"{"a": "\"}1", "b": 1, "c": [4/6, /[0-9]}}\/}/]}"#.to_string()
        );
        println!("[PASSED] test_does_not_fail_for_division_followed_by_a_regex");

        assert_eq!(
            cut_after_js(r#"{"a": "\"1", "b": 1, "c": {"test": 1}}abcd"#).unwrap_or("".to_string()),
            r#"{"a": "\"1", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_nested_objects");

        let test_str = r#"{"a": "\"1", "b": 1, "c": () => { try { /* do sth */ } catch (e) { a = [2+3] }; return 5}}"#;
        assert_eq!(
            cut_after_js(format!("{}abcd", test_str).as_str()).unwrap_or("".to_string()),
            test_str.to_string()
        );
        println!("[PASSED] test_works_with_try_catch");

        assert_eq!(
            cut_after_js(r#"{"a": "\"фыва", "b": 1, "c": {"test": 1}}abcd"#)
                .unwrap_or("".to_string()),
            r#"{"a": "\"фыва", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_utf");

        assert_eq!(
            cut_after_js(r#"{"a": "\\\\фыва", "b": 1, "c": {"test": 1}}abcd"#)
                .unwrap_or("".to_string()),
            r#"{"a": "\\\\фыва", "b": 1, "c": {"test": 1}}"#.to_string()
        );
        println!("[PASSED] test_works_with_backslashes_in_string");

        assert_eq!(
            cut_after_js(r#"{"text": "\\\\"};"#).unwrap_or("".to_string()),
            r#"{"text": "\\\\"}"#.to_string()
        );
        println!("[PASSED] test_works_with_backslashes_towards_end_of_string");

        assert_eq!(
            cut_after_js(r#"[{"a": 1}, {"b": 2}]abcd"#).unwrap_or("".to_string()),
            r#"[{"a": 1}, {"b": 2}]"#.to_string()
        );
        println!("[PASSED] test_works_with_array_as_start");

        assert!(cut_after_js("abcd]}").is_none());
        println!("[PASSED] test_returns_error_when_not_beginning_with_bracket");

        assert!(cut_after_js(r#"{"a": 1,{ "b": 1}"#).is_none());
        println!("[PASSED] test_returns_error_when_missing_closing_bracket");
    }
}
