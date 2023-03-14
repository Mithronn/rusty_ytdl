use std::collections::HashMap;

use scraper::{Html, Selector};
use xml_oxide::{sax::parser::Parser, sax::Event};

use crate::constants::{BASE_URL, FORMATS};
#[allow(unused_imports)]
use crate::info_extras::{get_media, get_related_videos};
use crate::structs::{DownloadError, DownloadOptions, VideoInfo, VideoInfoError};

#[allow(unused_imports)]
use crate::utils::{
    add_format_meta, choose_format, clean_video_details, get_cver, get_functions, get_html5player,
    get_video_id, is_not_yet_broadcasted, is_play_error, is_private_video, is_rental,
    parse_video_formats, sort_formats,
};

pub async fn download(
    url_or_id: impl Into<String>,
    client: Option<&reqwest::Client>,
    options: DownloadOptions,
) -> Result<Vec<u8>, DownloadError> {
    let client = client
        .and_then(|x| Some(x.clone()))
        .unwrap_or(reqwest::Client::builder().build().unwrap());

    let link: String = url_or_id.into();

    let info = get_info(&link, Some(&client))
        .await
        .map_err(|_op| DownloadError::VideoNotFound)?;
    let format = choose_format(&info.formats, &options.video_options)
        .map_err(|_op| DownloadError::VideoNotFound)?;

    // Only check for HLS formats
    let is_live_hls = format
        .as_object()
        .and_then(|x| x.get("isHLS"))
        .and_then(|x| x.as_bool());

    if is_live_hls.unwrap_or(false) {
        // Currently not supporting live streams
        return Ok(vec![]);
    }

    let link = format
        .as_object()
        .and_then(|x| {
            Some(
                x.get("url")
                    .and_then(|x| Some(x.as_str().unwrap_or("").to_string()))
                    .unwrap_or("".to_string()),
            )
        })
        .unwrap_or("".to_string());

    let dl_chunk_size = if options.dl_chunk_size.is_some() {
        options.dl_chunk_size.unwrap()
    } else {
        1024 * 1024 * 10 as u64 // -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
    };

    async fn get_next_chunk(
        client: &reqwest::Client,
        headers: &reqwest::header::HeaderMap,
        link: &str,
        content_length: &u64,
        dl_chunk_size: &u64,
        start: &mut u64,
        end: &mut u64,
    ) -> Result<Vec<u8>, DownloadError> {
        if *end >= *content_length {
            *end = 0;
        }

        let mut headers = headers.clone();

        let range_end = if *end == 0 {
            "".to_string()
        } else {
            end.to_string()
        };
        headers.insert(
            reqwest::header::RANGE,
            format!("bytes={}-{}", start, range_end).parse().unwrap(),
        );

        let response = client.get(link).headers(headers).send().await;

        if response.is_err() {
            return Err(DownloadError::VideoNotFound);
        }

        let mut response = response.expect("IMPOSSIBLE");

        let mut buf: Vec<u8> = vec![];

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_e| DownloadError::VideoNotFound)?
        {
            let chunk = chunk.to_vec();
            buf.extend(chunk.iter());
            // println!("Chunk recieved: {:?}", chunk.len());
        }

        if *end != 0 {
            *start = *end + 1;
            *end += *dl_chunk_size;
            // println!("Chunking new: Length {:?}", start);
        }

        Ok(buf)
    }

    let mut start = 0;
    let mut end = start + dl_chunk_size;

    let content_length = format.as_object().and_then(|x| {
        x.get("contentLength")
            .and_then(|x| Some(x.as_str().unwrap_or("").to_string()))
    });
    let mut content_length = content_length
        .unwrap_or("0".to_string())
        .parse::<u64>()
        .unwrap_or(0);

    // Get content length from source url if content_length is 0
    if content_length == 0 {
        let content_length_response = client
            .get(&link)
            .send()
            .await
            .map_err(|_op| DownloadError::VideoNotFound)?
            .content_length();

        if content_length_response.is_none() {
            return Err(DownloadError::VideoNotFound);
        }

        content_length = content_length_response.unwrap();
    }

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.101 Safari/537.36".parse().unwrap());

    let mut buf: Vec<u8> = vec![];
    loop {
        let chunk_result = get_next_chunk(
            &client,
            &headers,
            &link,
            &content_length,
            &dl_chunk_size,
            &mut start,
            &mut end,
        )
        .await;

        if chunk_result.is_err() {
            break;
        }

        buf.extend(chunk_result.unwrap().iter());

        if end == 0 {
            // Nothing else remain, break to loop!
            break;
        }
    }

    Ok(buf)
}

pub async fn get_basic_info(
    link: &str,
    client: Option<&reqwest::Client>,
) -> Result<VideoInfo, VideoInfoError> {
    // let mut cver = "2.20210622.10.00";
    let client = client
        .and_then(|x| Some(x.clone()))
        .unwrap_or(reqwest::Client::builder().build().unwrap());

    let id = get_video_id(link);

    if id.is_none() {
        return Err(VideoInfoError::VideoNotFound);
    }

    let link = format!("{}{}", BASE_URL, id.clone().unwrap());

    let url_parsed = url::Url::parse_with_params(link.as_str(), &[("hl", "en")]);

    if url_parsed.is_err() {
        return Err(VideoInfoError::URLParseError);
    }

    let request = client.get(url_parsed.unwrap().as_str()).send().await;

    if request.is_err() {
        return Err(VideoInfoError::HttpRequestError);
    }

    let response_first = request.unwrap().text().await;

    if response_first.is_err() {
        return Err(VideoInfoError::BodyCannotParsed);
    }

    let response = response_first.unwrap();

    let document = Html::parse_document(&response);
    let scripts_selector = Selector::parse("script").unwrap();
    let mut player_response_string = document
        .select(&scripts_selector)
        .filter(|x| x.inner_html().contains("var ytInitialPlayerResponse ="))
        .map(|x| x.inner_html().replace("var ytInitialPlayerResponse =", ""))
        .into_iter()
        .nth(0)
        .unwrap()
        .trim()
        .to_string();
    let mut initial_response_string = document
        .select(&scripts_selector)
        .filter(|x| x.inner_html().contains("var ytInitialData ="))
        .map(|x| x.inner_html().replace("var ytInitialData =", ""))
        .into_iter()
        .nth(0)
        .unwrap()
        .trim()
        .to_string();

    // remove json objects' last element (;)
    player_response_string.pop();
    initial_response_string.pop();

    let player_response: serde_json::Value = serde_json::from_str(&player_response_string).unwrap();
    let initial_response: serde_json::Value =
        serde_json::from_str(&initial_response_string).unwrap();

    let player_response_clone = &player_response.clone();
    let initial_response_clone = &initial_response.clone();

    // cver = get_cver(&player_response_clone);

    if is_play_error(&player_response_clone, ["ERROR"].to_vec()) {
        return Err(VideoInfoError::VideoNotFound);
    }

    if is_private_video(&player_response_clone) {
        return Err(VideoInfoError::VideoIsPrivate);
    }

    if player_response.get("streamingData").is_none()
        || is_rental(&player_response_clone)
        || is_not_yet_broadcasted(&player_response_clone)
    {
        return Err(VideoInfoError::VideoSourceNotFound);
    }

    let media = get_media(&initial_response_clone).unwrap();

    let video_details = clean_video_details(
        &initial_response_clone,
        &player_response_clone,
        media,
        id.unwrap(),
    );

    let html5player = get_html5player(response.as_str()).unwrap();

    let a = VideoInfo {
        player_response,
        initial_response,
        html5player: html5player.clone(),
        formats: parse_video_formats(
            &player_response_clone,
            get_functions(html5player, &client).await,
        )
        .unwrap_or(vec![]),
        related_videos: get_related_videos(&initial_response_clone).unwrap_or(vec![]),
        video_details,
    };

    return Ok(a);
}

pub async fn get_info(
    link: &str,
    client: Option<&reqwest::Client>,
) -> Result<VideoInfo, VideoInfoError> {
    let client = client
        .and_then(|x| Some(x.clone()))
        .unwrap_or(reqwest::Client::builder().build().unwrap());

    let mut info = get_basic_info(link, Some(&client)).await?;

    let has_manifest = info
        .player_response
        .get("streamingData")
        .and_then(|x| x.get("dashManifestUrl"))
        .is_some()
        || info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("hlsManifestUrl"))
            .is_some();

    if has_manifest
        && info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("dashManifestUrl"))
            .is_some()
    {
        // only support HLS-Formats for livestreams for now so all non-HLS streams will be ignored
        //
        // let url = info
        //     .player_response
        //     .get("streamingData")
        //     .and_then(|x| x.get("dashManifestUrl"))
        //     .and_then(|x| x.as_str())
        //     .unwrap_or_else(|| "");
        // let mut dash_manifest_formats = get_dash_manifest(url, &client).await;

        // for format in dash_manifest_formats.iter_mut() {
        //     let format_as_object = format.as_object_mut();
        //     if format_as_object.is_some() {
        //         let format_as_object = format_as_object.unwrap();

        //         // Insert url
        //         format_as_object.insert(
        //             "url".to_string(),
        //             serde_json::Value::String(url.to_string()),
        //         );

        //         // Add other metadatas to format map
        //         add_format_meta(format_as_object);
        //         info.formats.insert(info.formats.len(), format.clone());
        //     }
        // }
    }

    if has_manifest
        && info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("hlsManifestUrl"))
            .is_some()
    {
        let url = info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("hlsManifestUrl"))
            .and_then(|x| x.as_str())
            .unwrap_or_else(|| "");
        let unformated_formats = get_m3u8(url, &client).await;

        let default_formats = (*FORMATS).as_object().expect("IMPOSSIBLE");
        // Push formated infos to formats
        for (itag, url) in unformated_formats {
            let static_format = default_formats.get(&itag);
            if static_format.is_none() {
                continue;
            }
            let static_format = static_format.unwrap();

            let mut format = serde_json::json!({
                "itag": itag.parse::<i32>().unwrap_or(0),
                "mimeType": static_format.get("mimeType").expect("IMPOSSIBLE"),
            });

            let format_as_object_mut = format.as_object_mut().unwrap();

            if !static_format.get("qualityLabel").unwrap().is_null() {
                format_as_object_mut.insert(
                    "qualityLabel".to_string(),
                    static_format.get("qualityLabel").unwrap().clone(),
                );
            }

            if !static_format.get("bitrate").unwrap().is_null() {
                format_as_object_mut.insert(
                    "bitrate".to_string(),
                    static_format.get("bitrate").unwrap().clone(),
                );
            }

            if !static_format.get("audioBitrate").unwrap().is_null() {
                format_as_object_mut.insert(
                    "audioBitrate".to_string(),
                    static_format.get("audioBitrate").unwrap().clone(),
                );
            }

            // Insert stream url to format map
            format_as_object_mut.insert("url".to_string(), serde_json::Value::String(url));

            // Add other metadatas to format map
            add_format_meta(format_as_object_mut);

            info.formats.insert(info.formats.len(), format);
        }
    }

    // Last sort formats
    info.formats.sort_by(sort_formats);
    return Ok(info);
}

pub async fn get_dash_manifest(url: &str, client: &reqwest::Client) -> Vec<serde_json::Value> {
    let base_url = url::Url::parse(BASE_URL).expect("BASE_URL corrapt");
    let base_url_host = base_url.host_str().expect("BASE_URL host corrapt");

    let url = url::Url::parse(url)
        .and_then(|mut x| {
            let set_host_result = x.set_host(Some(base_url_host));
            if set_host_result.is_err() {
                return Err(set_host_result.err().expect("How can be possible"));
            }
            Ok(x)
        })
        .and_then(|x| Ok(x.as_str().to_string()))
        .unwrap_or("".to_string());

    let body = client.get(url).send().await.unwrap().text().await.unwrap();

    let mut parser = Parser::from_reader(body.as_bytes());

    let mut formats: Vec<serde_json::Value> = vec![];
    let mut adaptation_set: HashMap<String, String> = HashMap::default();

    loop {
        let res = parser.read_event();

        match res {
            Ok(event) => match event {
                Event::EndDocument => {
                    break;
                }
                Event::StartElement(node) => {
                    if node.name.to_lowercase() == "ADAPTATIONSET".to_lowercase() {
                        adaptation_set = node
                            .attributes()
                            .map(|x| (x.name.to_lowercase(), x.value.to_lowercase()))
                            .collect();
                    } else if node.name.to_lowercase() == "REPRESENTATION".to_lowercase() {
                        let representation: HashMap<String, String> = node
                            .attributes()
                            .map(|x| (x.name.to_lowercase(), x.value.to_lowercase()))
                            .collect();
                        if representation.contains_key("id") {
                            let itag = representation.get("id").unwrap();
                            let itag = itag.parse::<i32>();
                            if itag.is_ok() {
                                let itag = itag.unwrap();
                                let mut format = serde_json::json!({
                                    "itag": itag,
                                    "bitrate": representation.get("bandwith").and_then(|x| x.parse::<i32>().ok()).unwrap_or(0),
                                    "mimeType": format!(r#"{}; codecs="{}""#,adaptation_set.get("mimetype").and_then(|x| Some(x.as_str())).unwrap_or(""),representation.get("codecs").and_then(|x| Some(x.as_str())).unwrap_or(""))
                                });
                                let format_as_object_mut =
                                    format.as_object_mut().expect("IMPOSSIBLE");
                                if representation.contains_key("height") {
                                    format_as_object_mut.insert(
                                        "width".to_string(),
                                        serde_json::Value::Number(
                                            representation
                                                .get("width")
                                                .and_then(|x| x.parse::<i32>().ok())
                                                .unwrap_or(0)
                                                .into(),
                                        ),
                                    );
                                    format_as_object_mut.insert(
                                        "height".to_string(),
                                        serde_json::Value::Number(
                                            representation
                                                .get("height")
                                                .and_then(|x| x.parse::<i32>().ok())
                                                .unwrap_or(0)
                                                .into(),
                                        ),
                                    );
                                    format_as_object_mut.insert(
                                        "fps".to_string(),
                                        serde_json::Value::Number(
                                            representation
                                                .get("framerate")
                                                .and_then(|x| x.parse::<i32>().ok())
                                                .unwrap_or(0)
                                                .into(),
                                        ),
                                    );
                                } else {
                                    format_as_object_mut.insert(
                                        "audioSamplingRate".to_string(),
                                        serde_json::Value::Number(
                                            representation
                                                .get("audiosamplingrate")
                                                .and_then(|x| x.parse::<i32>().ok())
                                                .unwrap_or(0)
                                                .into(),
                                        ),
                                    );
                                }

                                formats.insert(formats.len(), format);
                            }
                        }
                    }
                }
                _ => {}
            },
            Err(err) => {
                println!("{}", err);
                break;
            }
        }
    }

    return formats;
}

pub async fn get_m3u8(url: &str, client: &reqwest::Client) -> Vec<(String, String)> {
    let base_url = url::Url::parse(BASE_URL).expect("BASE_URL corrapt");
    let base_url_host = base_url.host_str().expect("BASE_URL host corrapt");

    let url = url::Url::parse(url)
        .and_then(|mut x| {
            let set_host_result = x.set_host(Some(base_url_host));
            if set_host_result.is_err() {
                return Err(set_host_result.err().expect("How can be posible"));
            }
            Ok(x)
        })
        .and_then(|x| Ok(x.as_str().to_string()))
        .unwrap_or("".to_string());

    let body = client.get(url).send().await.unwrap().text().await.unwrap();

    let http_regex = regex::Regex::new(r"^https?://").unwrap();
    let itag_regex = regex::Regex::new(r"/itag/(\d+)/").unwrap();

    let itag_and_url = body
        .split("\n")
        .filter(|x| http_regex.is_match(x) && itag_regex.is_match(x));

    let itag_and_url: Vec<(String, String)> = itag_and_url
        .map(|line| {
            let itag = itag_regex
                .captures(line)
                .expect("IMPOSSIBLE")
                .get(1)
                .and_then(|x| Some(x.as_str()))
                .unwrap_or_else(|| "");

            // println!("itag: {}, url: {}", itag, line);
            (itag.to_string(), line.to_string())
        })
        .collect();

    itag_and_url
}
