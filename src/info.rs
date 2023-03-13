use std::collections::HashMap;

use scraper::{Html, Selector};
use xml_oxide::{sax::parser::Parser, sax::Event};

use crate::constants::{BASE_URL, FORMATS};
#[allow(unused_imports)]
use crate::info_extras::{get_media, get_related_videos};
use crate::structs::{VideoInfo, VideoInfoError};

#[allow(unused_imports)]
use crate::utils::{
    add_format_meta, clean_video_details, get_cver, get_functions, get_html5player, get_video_id,
    is_not_yet_broadcasted, is_play_error, is_private_video, is_rental, parse_video_formats,
    sort_formats,
};

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
        related_videos: get_related_videos(&initial_response_clone).unwrap(),
        video_details,
    };

    return Ok(a);
}

pub async fn get_info(link: &str) -> Result<VideoInfo, VideoInfoError> {
    let client = reqwest::Client::builder().build().unwrap();
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
