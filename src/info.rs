use std::collections::HashMap;
use std::sync::Arc;

use scraper::{Html, Selector};
use xml_oxide::{sax::parser::Parser, sax::Event};

use crate::constants::{BASE_URL, DEFAULT_HEADERS, FORMATS};
use crate::info_extras::{get_media, get_related_videos};
use crate::structs::{VideoError, VideoFormat, VideoInfo, VideoOptions};

use crate::utils::{
    add_format_meta, choose_format, clean_video_details, get_functions, get_html5player,
    get_random_v6_ip, get_video_id, is_not_yet_broadcasted, is_play_error, is_private_video,
    is_rental, parse_video_formats, sort_formats,
};

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "Video({video_id})")]
#[derivative(Debug, PartialEq, Eq)]
pub struct Video {
    video_id: String,
    options: VideoOptions,
    #[derivative(PartialEq = "ignore")]
    client: reqwest::Client,
}

impl Video {
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        let id = get_video_id(&url_or_id.into());

        if id.is_none() {
            return Err(VideoError::VideoNotFound);
        }

        let client = reqwest::Client::builder()
            .build()
            .map_err(|op| VideoError::Reqwest(op))?;

        Ok(Self {
            video_id: id.unwrap(),
            options: VideoOptions::default(),
            client,
        })
    }

    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: VideoOptions,
    ) -> Result<Self, VideoError> {
        let id = get_video_id(&url_or_id.into());

        if id.is_none() {
            return Err(VideoError::VideoNotFound);
        }

        let mut client = reqwest::Client::builder();

        if options.request_options.proxy.is_some() {
            client = client.proxy(options.request_options.proxy.as_ref().unwrap().clone());
        }

        if options.request_options.ipv6_block.is_some() {
            let ipv6 = get_random_v6_ip(options.request_options.ipv6_block.as_ref().unwrap())?;
            client = client.local_address(ipv6);
        }

        if options.request_options.cookies.is_some() {
            let cookie = options.request_options.cookies.as_ref().unwrap();
            let host = "https://youtube.com".parse::<url::Url>().unwrap();

            let jar = reqwest::cookie::Jar::default();
            jar.add_cookie_str(cookie.as_str(), &host);

            client = client.cookie_provider(Arc::new(jar));
        }

        let client = client.build().map_err(|op| VideoError::Reqwest(op))?;

        Ok(Self {
            video_id: id.unwrap(),
            options,
            client,
        })
    }

    pub async fn get_basic_info(&self) -> Result<VideoInfo, VideoError> {
        let client = &self.client;

        let url_parsed =
            url::Url::parse_with_params(self.get_video_url().as_str(), &[("hl", "en")]);
        if url_parsed.is_err() {
            return Err(VideoError::URLParseError(url_parsed.err().unwrap()));
        }

        let request = client.get(url_parsed.unwrap().as_str()).send().await;

        if request.is_err() {
            return Err(VideoError::Reqwest(request.err().unwrap()));
        }

        let response_first = request.unwrap().text().await;

        if response_first.is_err() {
            return Err(VideoError::BodyCannotParsed);
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

        let player_response: serde_json::Value =
            serde_json::from_str(&player_response_string).unwrap();
        let initial_response: serde_json::Value =
            serde_json::from_str(&initial_response_string).unwrap();

        if is_play_error(&player_response, ["ERROR"].to_vec()) {
            return Err(VideoError::VideoNotFound);
        }

        if is_private_video(&player_response) {
            return Err(VideoError::VideoIsPrivate);
        }

        if player_response.get("streamingData").is_none()
            || is_rental(&player_response)
            || is_not_yet_broadcasted(&player_response)
        {
            return Err(VideoError::VideoSourceNotFound);
        }

        let video_details = clean_video_details(
            &initial_response,
            &player_response,
            get_media(&initial_response).unwrap(),
            self.video_id.clone(),
        );

        let dash_manifest_url = player_response
            .get("streamingData")
            .and_then(|x| x.get("dashManifestUrl"))
            .and_then(|x| x.as_str())
            .and_then(|x| Some(x.to_string()));

        let hls_manifest_url = player_response
            .get("streamingData")
            .and_then(|x| x.get("hlsManifestUrl"))
            .and_then(|x| x.as_str())
            .and_then(|x| Some(x.to_string()));

        return Ok(VideoInfo {
            dash_manifest_url,
            hls_manifest_url,
            formats: parse_video_formats(
                &player_response,
                get_functions(get_html5player(response.as_str()).unwrap(), &client).await,
            )
            .unwrap_or(vec![]),
            related_videos: get_related_videos(&initial_response).unwrap_or(vec![]),
            video_details,
        });
    }

    pub async fn get_info(&self) -> Result<VideoInfo, VideoError> {
        let client = &self.client;

        let mut info = self.get_basic_info().await?;

        let has_manifest = info.dash_manifest_url.is_some() || info.hls_manifest_url.is_some();

        if has_manifest && info.dash_manifest_url.is_some() {
            // only support HLS-Formats for livestreams for now so all non-HLS streams will be ignored
            //
            // let url = info.dash_manifest_url.as_ref().unwrap();
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

            //         let format: Result<VideoFormat, serde_json::Error> =
            //             serde_json::from_value(format.clone());
            //         if format.is_err() {
            //             continue;
            //         }
            //         info.formats.insert(info.formats.len(), format.unwrap());
            //     }
            // }
        }

        if has_manifest && info.hls_manifest_url.is_some() {
            let url = info.hls_manifest_url.as_ref().unwrap();
            let unformated_formats = get_m3u8(&url, &client).await;

            let default_formats = FORMATS.as_object().expect("IMPOSSIBLE");
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

                let format: Result<VideoFormat, serde_json::Error> = serde_json::from_value(format);
                if format.is_err() {
                    continue;
                }
                info.formats.insert(info.formats.len(), format.unwrap());
            }
        }

        // Last sort formats
        info.formats.sort_by(sort_formats);
        return Ok(info);
    }

    pub async fn download(&self) -> Result<Vec<u8>, VideoError> {
        let client = &self.client;

        let info = self.get_info().await?;
        let format = choose_format(&info.formats, &self.options)
            .map_err(|_op| VideoError::VideoSourceNotFound)?;

        // Only check for HLS formats
        //
        // Currently not supporting live streams
        if format.is_hls {
            return Ok(vec![]);
        }

        // Normal video
        let link = format.url;

        if link.is_empty() {
            return Err(VideoError::VideoSourceNotFound);
        }

        let dl_chunk_size = if self.options.download_options.dl_chunk_size.is_some() {
            self.options.download_options.dl_chunk_size.unwrap()
        } else {
            1024 * 1024 * 10 as u64 // -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
        };

        async fn get_next_chunk(
            client: &reqwest::Client,
            link: &str,
            content_length: &u64,
            dl_chunk_size: &u64,
            start: &mut u64,
            end: &mut u64,
        ) -> Result<Vec<u8>, VideoError> {
            if *end >= *content_length {
                *end = 0;
            }

            let mut headers = DEFAULT_HEADERS.clone();

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
                return Err(VideoError::Reqwest(response.err().unwrap()));
            }

            let mut response = response.expect("IMPOSSIBLE");

            let mut buf: Vec<u8> = vec![];

            while let Some(chunk) = response.chunk().await.map_err(|e| VideoError::Reqwest(e))? {
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

        let mut content_length = format
            .content_length
            .unwrap_or("0".to_string())
            .parse::<u64>()
            .unwrap_or(0);

        // Get content length from source url if content_length is 0
        if content_length == 0 {
            let content_length_response = client
                .get(&link)
                .send()
                .await
                .map_err(|op| VideoError::Reqwest(op))?
                .content_length();

            if content_length_response.is_none() {
                return Err(VideoError::VideoNotFound);
            }

            content_length = content_length_response.unwrap();
        }

        let mut buf: Vec<u8> = vec![];
        loop {
            let chunk_result = get_next_chunk(
                &client,
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

    pub fn get_video_url(&self) -> String {
        return format!("{}{}", BASE_URL, &self.video_id);
    }

    pub fn get_video_id(&self) -> String {
        return self.video_id.clone();
    }
}

async fn get_dash_manifest(url: &str, client: &reqwest::Client) -> Vec<serde_json::Value> {
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
                                        "audioSampleRate".to_string(),
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

async fn get_m3u8(url: &str, client: &reqwest::Client) -> Vec<(String, String)> {
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
