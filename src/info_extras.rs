use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::constants::BASE_URL;
use crate::structs::{Author, Chapter, RelatedVideo, StoryBoard, Thumbnail};
use crate::utils::{get_text, is_verified, parse_abbreviated_number, time_to_ms};

pub fn get_related_videos(info: &serde_json::Value) -> Option<Vec<RelatedVideo>> {
    let mut rvs_params: Vec<&str> = vec![];
    let mut secondary_results: Vec<serde_json::Value> = vec![];

    let mut rvs_params_closure = || -> Result<(), &str> {
        rvs_params = info
            .as_object()
            .and_then(|x| x.get("webWatchNextResponseExtensionData"))
            .and_then(|x| x.get("relatedVideoArgs"))
            .and_then(|x| {
                x.as_str()
                    .and_then(|c| Some(c.split(",").collect::<Vec<&str>>()))
            })
            .unwrap_or_default();
        Ok(())
    };

    if let Err(_err) = rvs_params_closure() {
        rvs_params = vec![];
    }

    let mut secondary_results_closure = || -> Result<(), &str> {
        secondary_results = info
            .as_object()
            .and_then(|x| x.get("contents"))
            .and_then(|x| x.get("twoColumnWatchNextResults"))
            .and_then(|x| x.get("secondaryResults"))
            .and_then(|x| x.get("secondaryResults"))
            .and_then(|x| x.get("results"))
            .and_then(|x| Some(x.as_array()?.to_vec()))
            .unwrap_or_default();
        Ok(())
    };

    if let Err(_err) = secondary_results_closure() {
        secondary_results = vec![];
    }

    let contents_fallback: Vec<serde_json::Value> = vec![];
    let fallback_value = serde_json::map::Map::new();

    let mut videos: Vec<RelatedVideo> = vec![];
    for result in secondary_results {
        let details = result
            .as_object()
            .and_then(|x| {
                x.get("compactVideoRenderer")
                    .and_then(|c| Some(c.as_object().unwrap()))
            })
            .unwrap_or(&fallback_value);

        if !details.is_empty() {
            let video = parse_related_video(details, &rvs_params);
            if !video.is_none() {
                videos.push(video.unwrap())
            }
        } else {
            let autoplay = result
                .as_object()
                .and_then(|x| x.get("compactAutoplayRenderer").and_then(|c| c.as_object()))
                .unwrap_or(&fallback_value);

            if !autoplay.contains_key("contents") {
                continue;
            };

            let contents = autoplay
                .get("contents")
                .and_then(|x| x.as_array())
                .unwrap_or(&contents_fallback);

            for content in contents {
                let content_details = content
                    .get("compactVideoRenderer")
                    .and_then(|x| x.as_object())
                    .unwrap_or(&fallback_value);
                if content_details.is_empty() {
                    continue;
                }

                let video = parse_related_video(content_details, &rvs_params);
                if !video.is_none() {
                    videos.push(video.unwrap())
                }
            }
        }
    }

    Some(videos)
}

pub fn parse_related_video(
    details: &serde_json::map::Map<String, serde_json::Value>,
    rvs_params: &Vec<&str>,
) -> Option<RelatedVideo> {
    #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
    struct QueryParams {
        id: String,
        short_view_count_text: String,
        length_seconds: String,
    }
    let mut view_count = if details.contains_key("viewCountText") {
        get_text(&details["viewCountText"])
            .as_str()
            .unwrap_or("".into())
    } else {
        "0"
    };

    let mut short_view_count = if details.contains_key("shortViewCountText") {
        get_text(&details["shortViewCountText"])
            .as_str()
            .unwrap_or("".into())
            .to_string()
    } else {
        "0".to_string()
    };

    let regex = Regex::new(r"^\d").unwrap();

    if !regex.is_match(&short_view_count) {
        let rvs_details_index = rvs_params
            .iter()
            .map(|x| serde_qs::from_str::<QueryParams>(x).unwrap())
            .position(|r| r.id == String::from(details["videoId"].as_str().unwrap_or("0".into())));

        if rvs_details_index.is_some() {
            let rvs_details_index = rvs_details_index.unwrap();
            let rvs_params_to_short_view_count = rvs_params
                .get(rvs_details_index)
                .and_then(|x| Some(*x))
                .unwrap_or("");

            short_view_count = serde_qs::from_str::<QueryParams>(rvs_params_to_short_view_count)
                .and_then(|x| Ok(x.short_view_count_text))
                .unwrap_or("0".to_string())
        }
    }

    view_count = if regex.is_match(view_count) {
        view_count
            .split(' ')
            .collect::<Vec<&str>>()
            .get(0)
            .and_then(|x| Some(*x))
            .unwrap_or("")
    } else {
        short_view_count
            .split(' ')
            .collect::<Vec<&str>>()
            .get(0)
            .and_then(|x| Some(*x))
            .unwrap_or("")
    };

    let is_live = details
        .get("badges")
        .and_then(|c| {
            Some(
                c.as_array()
                    .and_then(|x| {
                        Some(
                            x.iter()
                                .filter(|x| {
                                    let json = serde_json::json!(x);
                                    json["metadataBadgeRenderer"]["label"] == "LIVE NOW"
                                })
                                .count()
                                > 0,
                        )
                    })
                    .unwrap_or(false),
            )
        })
        .unwrap_or(false);

    let browse_end_point =
        &details["shortBylineText"]["runs"][0]["navigationEndpoint"]["browseEndpoint"];
    let channel_id = &browse_end_point["browseId"];
    let author_user = browse_end_point
        .get("canonicalBaseUrl")
        .and_then(|x| {
            Some(
                x.as_str()
                    .and_then(|c| {
                        Some(
                            c.split("/")
                                .collect::<Vec<&str>>()
                                .last()
                                .and_then(|c| Some(*c))
                                .unwrap_or(""),
                        )
                    })
                    .unwrap_or(""),
            )
        })
        .unwrap_or("");

    let view_count_regex = Regex::new(r",").unwrap();

    let video = RelatedVideo {
        id: details
            .get("videoId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        title: if details.contains_key("title") {
            get_text(&details["title"])
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            String::from("")
        },
        url: if details.contains_key("videoId") {
            let id = details
                .get("videoId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() {
                format!("{}{}", BASE_URL, id)
            } else {
                String::from("")
            }
        } else {
            String::from("")
        },
        published: if details.contains_key("publishedTimeText") {
            get_text(&details["publishedTimeText"])
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            String::from("")
        },
        author: if !browse_end_point.is_null() {
            Some(Author {
                id: channel_id.as_str().unwrap_or("").to_string(),
                name: if details.contains_key("shortBylineText") {
                    get_text(&details["shortBylineText"])
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::from("")
                },
                user: author_user.to_string(),
                channel_url: if !channel_id.as_str().unwrap_or("").to_string().is_empty() {
                    format!(
                        "https://www.youtube.com/channel/{channel_id}",
                        channel_id = channel_id.as_str().unwrap_or("")
                    )
                } else {
                    String::from("")
                },
                external_channel_url: if !channel_id.as_str().unwrap_or("").to_string().is_empty() {
                    format!(
                        "https://www.youtube.com/channel/{channel_id}",
                        channel_id = channel_id.as_str().unwrap_or("")
                    )
                } else {
                    String::from("")
                },
                user_url: if !author_user.is_empty() {
                    if author_user.starts_with("@") {
                        format!("https://www.youtube.com/{user}", user = author_user)
                    } else {
                        String::from("")
                    }
                } else {
                    String::from("")
                },
                thumbnails: if !details["channelThumbnail"]["thumbnails"].is_null() {
                    details["channelThumbnail"]["thumbnails"]
                        .as_array()
                        .and_then(|f| {
                            Some(
                                f.iter()
                                    .map(|x| Thumbnail {
                                        width: x
                                            .get("width")
                                            .and_then(|x| {
                                                if x.is_string() {
                                                    x.as_str().and_then(|x| {
                                                        match x.parse::<i64>() {
                                                            Ok(a) => Some(a),
                                                            Err(_err) => Some(0i64),
                                                        }
                                                    })
                                                } else {
                                                    x.as_i64()
                                                }
                                            })
                                            .unwrap_or(0i64)
                                            as u64,
                                        height: x
                                            .get("height")
                                            .and_then(|x| {
                                                if x.is_string() {
                                                    x.as_str().and_then(|x| {
                                                        match x.parse::<i64>() {
                                                            Ok(a) => Some(a),
                                                            Err(_err) => Some(0i64),
                                                        }
                                                    })
                                                } else {
                                                    x.as_i64()
                                                }
                                            })
                                            .unwrap_or(0i64)
                                            as u64,
                                        url: x
                                            .get("url")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    })
                                    .collect::<Vec<Thumbnail>>(),
                            )
                        })
                        .unwrap_or(vec![])
                } else {
                    vec![]
                },
                verified: if details.contains_key("ownerBadges") {
                    is_verified(&details["ownerBadges"])
                } else {
                    false
                },
                subscriber_count: 0,
            })
        } else {
            None
        },
        short_view_count_text: short_view_count
            .split(' ')
            .collect::<Vec<&str>>()
            .get(0)
            .unwrap_or(&"")
            .to_string(),
        view_count: view_count_regex.replace_all(view_count, "").to_string(),
        length_seconds: if details.contains_key("lengthText") {
            let time = (time_to_ms(get_text(&details["lengthText"]).as_str().unwrap_or("0")) / 1000)
                as f32;
            time.floor().to_string()
        } else {
            "0".to_string()
        },
        thumbnails: if !details["thumbnail"]["thumbnails"].is_null() {
            details["thumbnail"]["thumbnails"]
                .as_array()
                .and_then(|f| {
                    Some(
                        f.iter()
                            .map(|x| Thumbnail {
                                width: x
                                    .get("width")
                                    .and_then(|x| {
                                        if x.is_string() {
                                            x.as_str().and_then(|x| match x.parse::<i64>() {
                                                Ok(a) => Some(a),
                                                Err(_err) => Some(0i64),
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
                                            x.as_str().and_then(|x| match x.parse::<i64>() {
                                                Ok(a) => Some(a),
                                                Err(_err) => Some(0i64),
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
                    )
                })
                .unwrap_or(vec![])
        } else {
            vec![]
        },
        is_live,
    };

    Some(video)
}

pub fn get_media(info: &serde_json::Value) -> Option<serde_json::Value> {
    let empty_serde_array = serde_json::json!([]);
    let empty_serde_object_array = vec![serde_json::json!({})];
    let empty_serde_object = serde_json::json!({});

    let results = info
        .as_object()
        .and_then(|x| x.get("contents"))
        .and_then(|x| x.get("twoColumnWatchNextResults"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("contents"))
        .unwrap_or(&empty_serde_array)
        .as_array()
        .unwrap_or(&empty_serde_object_array);

    let result_option = results
        .iter()
        .find(|x| x.get("videoSecondaryInfoRenderer").is_some());

    let json_result = if result_option.is_none() {
        Some(serde_json::json!({}))
    } else {
        let result = result_option.unwrap();

        let metadata_rows = if result.get("metadataRowContainer").is_some() {
            result
                .get("metadataRowContainer")
                .and_then(|x| x.get("metadataRowContainerRenderer"))
                .and_then(|x| x.get("rows"))
                .unwrap_or(&empty_serde_object)
        } else if result.get("videoSecondaryInfoRenderer").is_some()
            && result
                .get("videoSecondaryInfoRenderer")
                .and_then(|x| x.get("metadataRowContainer"))
                .is_some()
        {
            result
                .get("videoSecondaryInfoRenderer")
                .and_then(|x| x.get("metadataRowContainer"))
                .and_then(|x| x.get("metadataRowContainerRenderer"))
                .and_then(|x| x.get("rows"))
                .unwrap_or(&empty_serde_object)
        } else {
            &empty_serde_object
        }
        .as_array()
        .unwrap_or(&empty_serde_object_array);

        let mut return_object = serde_json::json!({});

        for row in metadata_rows {
            // println!("{}", serde_json::to_string_pretty(row).unwrap());
            if row.get("metadataRowRenderer").is_some() {
                let title = get_text(
                    &row.get("metadataRowRenderer")
                        .and_then(|x| x.get("title"))
                        .unwrap_or(&empty_serde_object),
                )
                .as_str()
                .unwrap_or("title");
                let contents = row
                    .get("metadataRowRenderer")
                    .and_then(|x| x.get("contents"))
                    .and_then(|x| x.as_array())
                    .unwrap_or(&empty_serde_object_array)
                    .get(0)
                    .unwrap_or(&empty_serde_object);

                let runs = contents.get("runs");

                let mut title_url = "";

                if runs.is_some()
                    && runs.unwrap_or(&empty_serde_object).is_array()
                    && runs
                        .unwrap_or(&empty_serde_object)
                        .as_array()
                        .and_then(|x| x.get(0))
                        .and_then(|x| x.get("navigationEndpoint"))
                        .is_some()
                {
                    title_url = runs
                        .unwrap_or(&empty_serde_array)
                        .as_array()
                        .unwrap_or(&empty_serde_object_array)
                        .get(0)
                        .and_then(|x| x.get("navigationEndpoint"))
                        .and_then(|x| x.get("commandMetadata"))
                        .and_then(|x| x.get("webCommandMetadata"))
                        .and_then(|x| x.get("url"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                }

                let mut category = "";
                let mut category_url = "";

                if title == "song" {
                    category = "Music";
                    category_url = "https://music.youtube.com/"
                }

                let data = format!(
                    r#"
                "{title}": {title_content},
                "{title}_url": {title_url},
                "category: {category},
                "category_url": {category_url},
                "#,
                    title = title,
                    title_content = get_text(contents).as_str().unwrap_or(""),
                    title_url = title_url,
                    category = category,
                    category_url = category_url,
                );

                return_object =
                    serde_json::from_str(data.as_str()).unwrap_or(serde_json::json!({}));
            } else if row.get("richMetadataRowRenderer").is_some() {
                let contents = row
                    .get("richMetadataRowRenderer")
                    .and_then(|x| x.get("contents"))
                    .and_then(|x| x.as_array())
                    .unwrap_or(&empty_serde_object_array);

                let box_art = contents.iter().filter(|x| {
                    x.get("richMetadataRenderer")
                        .and_then(|c| c.get("style"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        == "RICH_METADATA_RENDERER_STYLE_BOX_ART"
                });

                let mut media_year = "";
                let mut media_type = "type";
                let mut media_type_title = "";
                let mut media_type_url = "";
                let mut media_thumbnails = &empty_serde_array;

                for box_art_value in box_art {
                    let meta = box_art_value
                        .get("richMetadataRenderer")
                        .unwrap_or(&empty_serde_object);

                    media_year = get_text(meta.get("subtitle").unwrap_or(&empty_serde_object))
                        .as_str()
                        .unwrap_or("");

                    media_type = get_text(meta.get("callToAction").unwrap_or(&empty_serde_object))
                        .as_str()
                        .unwrap_or("type")
                        .split(' ')
                        .collect::<Vec<&str>>()
                        .get(1)
                        .unwrap_or(&"type");

                    media_type_title = get_text(meta.get("title").unwrap_or(&empty_serde_object))
                        .as_str()
                        .unwrap_or("");

                    media_type_url = meta
                        .get("endpoint")
                        .and_then(|x| x.get("commandMetadata"))
                        .and_then(|x| x.get("webCommandMetadata"))
                        .and_then(|x| x.get("url"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    media_thumbnails = meta
                        .get("thumbnail")
                        .and_then(|x| x.get("thumbnails"))
                        .unwrap_or(&empty_serde_array);
                }

                let topic = contents.iter().filter(|x| {
                    x.get("richMetadataRenderer")
                        .and_then(|x| x.get("style"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        == "RICH_METADATA_RENDERER_STYLE_TOPIC"
                });

                let mut category = "";
                let mut category_url = "";

                for topic_value in topic {
                    let meta = topic_value
                        .get("richMetadataRenderer")
                        .unwrap_or(&empty_serde_object);

                    category = get_text(meta.get("title").unwrap_or(&empty_serde_object))
                        .as_str()
                        .unwrap_or("");

                    category_url = meta
                        .get("endpoint")
                        .and_then(|x| x.get("commandMetadata"))
                        .and_then(|x| x.get("webCommandMetadata"))
                        .and_then(|x| x.get("url"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                }

                let data = format!(
                    r#"
                    "year": {media_year},
                    "{media_type}": {media_type_title},
                    "{media_type}_url": {media_type_url},
                    "thumbnails: {media_thumbnails},
                    "category: {category},
                    "category_url": {category_url},
                    "#,
                    media_year = media_year,
                    media_type = media_type,
                    media_type_title = media_type_title,
                    media_type_url = media_type_url,
                    media_thumbnails = media_thumbnails,
                    category = category,
                    category_url = category_url,
                );

                return_object =
                    serde_json::from_str(data.as_str()).unwrap_or(serde_json::json!({}));
            }
        }

        Some(return_object)
    };

    json_result
}

pub fn get_author(
    initial_response: &serde_json::Value,
    player_response: &serde_json::Value,
) -> Option<Author> {
    let serde_empty_object = serde_json::json!({});
    let empty_serde_object_array: Vec<serde_json::Value> = vec![];

    let mut results: Vec<serde_json::Value> = vec![];

    let mut results_closure = || -> Result<(), &str> {
        results = initial_response
            .as_object()
            .and_then(|x| x.get("contents"))
            .and_then(|x| x.get("twoColumnWatchNextResults"))
            .and_then(|x| x.get("results"))
            .and_then(|x| x.get("results"))
            .and_then(|x| x.get("contents"))
            .and_then(|x| Some(x.as_array()?.to_vec()))
            .unwrap_or_default();
        Ok(())
    };

    if let Err(_err) = results_closure() {
        results = vec![];
    }

    let v_position = results
        .iter()
        .position(|x| {
            let video_owner_renderer_index = x
                .as_object()
                .and_then(|x| x.get("videoSecondaryInfoRenderer"))
                .and_then(|x| x.get("owner"))
                .and_then(|x| x.get("videoOwnerRenderer"));
            video_owner_renderer_index.unwrap_or(&serde_json::Value::Null)
                != &serde_json::Value::Null
        })
        .unwrap_or(usize::MAX);

    // match v_position
    let v = results.get(v_position).unwrap_or(&serde_empty_object);

    let video_ownder_renderer = v
        .get("videoSecondaryInfoRenderer")
        .and_then(|x| x.get("owner"))
        .and_then(|x| x.get("videoOwnerRenderer"))
        .unwrap_or(&serde_empty_object);

    let channel_id = video_ownder_renderer
        .get("navigationEndpoint")
        .and_then(|x| x.get("browseEndpoint"))
        .and_then(|x| x.get("browseId"))
        .and_then(|x| x.as_str())
        .unwrap_or(&"");
    let thumbnails = video_ownder_renderer
        .get("thumbnail")
        .and_then(|x| x.get("thumbnails"))
        .and_then(|x| x.as_array())
        .unwrap_or(&empty_serde_object_array)
        .clone()
        .iter()
        .map(|x| Thumbnail {
            width: x
                .get("width")
                .and_then(|x| {
                    if x.is_string() {
                        x.as_str().and_then(|x| match x.parse::<i64>() {
                            Ok(a) => Some(a),
                            Err(_err) => Some(0i64),
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
                        x.as_str().and_then(|x| match x.parse::<i64>() {
                            Ok(a) => Some(a),
                            Err(_err) => Some(0i64),
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
        .collect::<Vec<Thumbnail>>();
    let zero_viewer = serde_json::json!("0");
    let subscriber_count = parse_abbreviated_number(
        get_text(
            video_ownder_renderer
                .get("subscriberCountText")
                .unwrap_or(&&zero_viewer),
        )
        .as_str()
        .unwrap_or("0"),
    );
    let verified = is_verified(
        video_ownder_renderer
            .get("badges")
            .unwrap_or(&serde_empty_object),
    );
    let video_details = player_response
        .get("microformat")
        .and_then(|x| x.get("playerMicroformatRenderer"))
        .unwrap_or(&serde_empty_object);

    let id = if serde_json::json!(video_details).is_object()
        && video_details.get("channelId").is_some()
    {
        video_details
            .get("channelId")
            .and_then(|x| x.as_str())
            .unwrap_or({
                if channel_id != "" {
                    channel_id
                } else {
                    player_response
                        .get("videoDetails")
                        .and_then(|x| x.get("channelId"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                }
            })
    } else if channel_id != "" {
        channel_id
    } else {
        player_response
            .get("videoDetails")
            .and_then(|x| x.get("channelId"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
    };

    let user = if video_details
        .as_object()
        .and_then(|x| Some(!x.is_empty()))
        .unwrap_or(false)
    {
        video_details
            .get("ownerProfileUrl")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .split("/")
            .collect::<Vec<&str>>()
            .last()
            .unwrap_or(&"")
            .to_string()
    } else {
        String::from("")
    };

    Some(Author {
        id: id.to_string(),
        name: if video_details
            .as_object()
            .and_then(|x| Some(!x.is_empty()))
            .unwrap_or(false)
        {
            video_details
                .get("ownerChannelName")
                .and_then(|x| x.as_str())
                .unwrap_or({
                    player_response
                        .get("videoDetails")
                        .and_then(|x| x.get("author"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                })
                .to_string()
        } else {
            player_response
                .get("videoDetails")
                .and_then(|x| x.get("author"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        },
        user: user.clone(),
        channel_url: format!("https://www.youtube.com/channel/{id}", id = id).to_string(),
        external_channel_url: if video_details
            .as_object()
            .and_then(|x| Some(!x.is_empty()))
            .unwrap_or(false)
        {
            let external_channel_id = video_details
                .get("externalChannelId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            let mut return_string = String::from("");
            if external_channel_id != "" {
                return_string =
                    format!("https://www.youtube.com/channel/{}", external_channel_id).to_string();
            }
            return_string
        } else {
            String::from("")
        },
        user_url: if !user.trim().is_empty() {
            format!("https://www.youtube.com/{}", user)
        } else {
            String::from("")
        },
        thumbnails,
        verified,
        subscriber_count: subscriber_count as i32,
    })
}

pub fn get_likes(info: &serde_json::Value) -> i32 {
    let serde_empty_object = serde_json::json!({});
    let empty_serde_object_array = vec![serde_json::json!({})];

    let contents = info
        .get("contents")
        .and_then(|x| x.get("twoColumnWatchNextResults"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("contents"))
        .unwrap_or(&serde_empty_object);

    let video = contents
        .as_array()
        .and_then(|x| {
            let info_renderer_position = x
                .iter()
                .position(|c| c.get("videoPrimaryInfoRenderer").is_some())
                .unwrap_or(usize::MAX);

            contents
                .as_array()
                .and_then(|c| Some(c.get(info_renderer_position)))
                .unwrap_or(Some(&serde_empty_object))
        })
        .unwrap_or(&serde_empty_object);

    let buttons = video
        .get("videoPrimaryInfoRenderer")
        .and_then(|x| x.get("videoActions"))
        .and_then(|x| x.get("menuRenderer"))
        .and_then(|x| x.get("topLevelButtons"))
        .and_then(|x| x.as_array())
        .unwrap_or(&empty_serde_object_array);

    let like_index = buttons
        .iter()
        .position(|x| {
            let icon_type = x
                .get("toggleButtonRenderer")
                .and_then(|c| c.get("defaultIcon"))
                .and_then(|c| c.get("iconType"))
                .and_then(|c| c.as_str())
                .unwrap_or("");

            icon_type == "LIKE"
        })
        .unwrap_or(usize::MAX);

    let like = buttons.get(like_index).unwrap_or(&serde_empty_object);

    let count = like
        .get("toggleButtonRenderer")
        .and_then(|x| x.get("defaultText"))
        .and_then(|x| x.get("accessibility"))
        .and_then(|x| x.get("accessibilityData"))
        .and_then(|x| x.get("label"))
        .and_then(|x| x.as_str())
        .unwrap_or("0");

    let count_regex = regex::Regex::new(r"\D+").unwrap();

    let count_final = count_regex.replace_all(count, "");

    count_final.parse::<i32>().unwrap_or(0i32)
}

pub fn get_dislikes(info: &serde_json::Value) -> i32 {
    let serde_empty_object = serde_json::json!({});
    let empty_serde_object_array = vec![serde_json::json!({})];

    let contents = info
        .get("contents")
        .and_then(|x| x.get("twoColumnWatchNextResults"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("results"))
        .and_then(|x| x.get("contents"))
        .unwrap_or(&serde_empty_object);

    let video = contents
        .as_array()
        .and_then(|x| {
            let info_renderer_position = x
                .iter()
                .position(|c| c.get("videoPrimaryInfoRenderer").is_some())
                .unwrap_or(usize::MAX);

            contents
                .as_array()
                .and_then(|c| Some(c.get(info_renderer_position)))
                .unwrap_or(Some(&serde_empty_object))
        })
        .unwrap_or(&serde_empty_object);

    let buttons = video
        .get("videoPrimaryInfoRenderer")
        .and_then(|x| x.get("videoActions"))
        .and_then(|x| x.get("menuRenderer"))
        .and_then(|x| x.get("topLevelButtons"))
        .and_then(|x| x.as_array())
        .unwrap_or(&empty_serde_object_array);

    let like_index = buttons
        .iter()
        .position(|x| {
            let icon_type = x
                .get("toggleButtonRenderer")
                .and_then(|c| c.get("defaultIcon"))
                .and_then(|c| c.get("iconType"))
                .and_then(|c| c.as_str())
                .unwrap_or("");

            icon_type == "DISLIKE"
        })
        .unwrap_or(usize::MAX);

    let like = buttons.get(like_index).unwrap_or(&serde_empty_object);

    let count = like
        .get("toggleButtonRenderer")
        .and_then(|x| x.get("defaultText"))
        .and_then(|x| x.get("accessibility"))
        .and_then(|x| x.get("accessibilityData"))
        .and_then(|x| x.get("label"))
        .and_then(|x| x.as_str())
        .unwrap_or("0");

    let count_regex = regex::Regex::new(r"\D+").unwrap();

    let count_final = count_regex.replace_all(count, "");

    count_final.parse::<i32>().unwrap_or(0i32)
}

pub fn get_storyboards(info: &serde_json::Value) -> Option<Vec<StoryBoard>> {
    let parts = info
        .get("storyboards")
        .and_then(|x| x.get("playerStoryboardSpecRenderer"))
        .and_then(|x| x.get("spec"))
        .and_then(|x| x.as_str());

    if parts.is_none() {
        return Some(vec![]);
    };

    let mut parts = parts.unwrap_or("").split("|").collect::<Vec<&str>>();

    let mut url = url::Url::parse(parts.remove(0))
        .unwrap_or(url::Url::parse("https://i.ytimg.com/").unwrap());
    Some(
        parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                let part_split_vec = part.split('#').collect::<Vec<&str>>();
                let thumbnail_width = part_split_vec.get(0).unwrap_or(&"0");
                let thumbnail_height = part_split_vec.get(1).unwrap_or(&"0");
                let thumbnail_count = part_split_vec.get(2).unwrap_or(&"0");
                let columns = part_split_vec.get(3).unwrap_or(&"0");
                let rows = part_split_vec.get(4).unwrap_or(&"0");
                let interval = part_split_vec.get(5).unwrap_or(&"0");
                let name_replacement = part_split_vec.get(6).unwrap_or(&"0");
                let sigh = part_split_vec.get(7).unwrap_or(&"0");

                url.query_pairs_mut().append_pair("sigh", sigh);

                let thumbnail_count_parsed =
                    i32::from_str_radix(thumbnail_count, 10).unwrap_or(0i32);
                let columns_parsed = i32::from_str_radix(columns, 10).unwrap_or(0i32);
                let rows_parsed = i32::from_str_radix(rows, 10).unwrap_or(0i32);

                let storyboard_count_ceiled =
                    thumbnail_count_parsed / (columns_parsed * rows_parsed);

                let template_url = String::from(
                    url.as_str()
                        .replace("$L", i.to_string().as_str())
                        .replace("$N", name_replacement),
                );

                StoryBoard {
                    template_url,
                    thumbnail_width: i32::from_str_radix(thumbnail_width, 10).unwrap_or(0i32),
                    thumbnail_height: i32::from_str_radix(thumbnail_height, 10).unwrap_or(0i32),
                    thumbnail_count: thumbnail_count_parsed,
                    interval: i32::from_str_radix(interval, 10).unwrap_or(0i32),
                    columns: columns_parsed,
                    rows: rows_parsed,
                    storyboard_count: storyboard_count_ceiled,
                }
            })
            .collect::<Vec<StoryBoard>>(),
    )
}

pub fn get_chapters(info: &serde_json::Value) -> Option<Vec<Chapter>> {
    let serde_empty_object = serde_json::json!({});
    let empty_serde_object_array = vec![serde_json::json!({})];

    let player_overlay_renderer = info
        .get("playerOverlays")
        .and_then(|x| x.get("playerOverlayRenderer"))
        .unwrap_or(&serde_empty_object);

    let player_bar = player_overlay_renderer
        .get("decoratedPlayerBarRenderer")
        .and_then(|x| x.get("decoratedPlayerBarRenderer"))
        .and_then(|x| x.get("playerBar"))
        .unwrap_or(&serde_empty_object);

    let markers_map = player_bar
        .get("multiMarkersPlayerBarRenderer")
        .and_then(|x| x.get("markersMap"))
        .and_then(|x| x.as_array())
        .unwrap_or(&empty_serde_object_array);

    let marker_index = markers_map
        .iter()
        .position(|x| {
            x.get("value").is_some()
                && x.get("value")
                    .and_then(|c| {
                        Some(
                            c.get("chapters")
                                .and_then(|d| Some(d.is_array()))
                                .unwrap_or(false),
                        )
                    })
                    .unwrap_or(false)
        })
        .unwrap_or(usize::MAX);

    let marker = markers_map
        .get(marker_index)
        .and_then(|x| x.as_object())
        .unwrap_or(&serde_empty_object.as_object().unwrap());

    if marker.is_empty() {
        return Some(vec![]);
    }

    let chapters = marker
        .get("value")
        .and_then(|x| x.get("chapters"))
        .and_then(|x| x.as_array())
        .unwrap_or(&empty_serde_object_array);

    Some(
        chapters
            .iter()
            .map(|x| Chapter {
                title: get_text(
                    x.get("chapterRenderer")
                        .and_then(|x| x.get("title"))
                        .unwrap_or(&serde_empty_object),
                )
                .as_str()
                .unwrap_or("")
                .to_string(),
                start_time: (x
                    .get("chapterRenderer")
                    .and_then(|x| x.get("timeRangeStartMillis"))
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0f64)
                    / 1000f64) as i32,
            })
            .collect::<Vec<Chapter>>(),
    )
}
