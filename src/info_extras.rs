use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, map::Map, Value};

use crate::{
    constants::BASE_URL,
    structs::{Author, Chapter, PlayerResponse, RelatedVideo, StoryBoard, Thumbnail},
    utils::{get_text, is_verified, parse_abbreviated_number, time_to_ms},
};

pub fn get_related_videos(info: &Value) -> Option<Vec<RelatedVideo>> {
    let mut rvs_params: Vec<&str> = vec![];
    let mut secondary_results: Vec<Value> = vec![];

    let mut rvs_params_closure = || -> Result<(), &str> {
        rvs_params = info["webWatchNextResponseExtensionData"]["relatedVideoArgs"]
            .as_str()
            .map(|c| c.split(',').collect::<Vec<&str>>())
            .unwrap_or_default();
        Ok(())
    };

    if let Err(_err) = rvs_params_closure() {
        rvs_params = vec![];
    }

    let mut secondary_results_closure = || -> Result<(), &str> {
        secondary_results = info["contents"]["twoColumnWatchNextResults"]["secondaryResults"]
            ["secondaryResults"]["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .to_vec();
        Ok(())
    };

    if let Err(_err) = secondary_results_closure() {
        secondary_results = vec![];
    }

    let mut videos: Vec<RelatedVideo> = vec![];
    for result in secondary_results {
        let details = result.as_object().and_then(|x| {
            x.get("compactVideoRenderer")
                .map(|c| c.as_object().cloned().unwrap_or_default())
        });

        if let Some(details) = details {
            if let Some(video_some) = parse_related_video(&details, &rvs_params) {
                videos.push(video_some)
            }
        } else if let Some(autoplay) = result.as_object().and_then(|x| {
            x.get("compactAutoplayRenderer")
                .map(|c| c.as_object().cloned().unwrap_or_default())
        }) {
            let contents = autoplay
                .get("contents")
                .map(|x| x.as_array().cloned().unwrap_or_default());

            if let Some(contents) = contents {
                for content in contents {
                    let content_details = content
                        .get("compactVideoRenderer")
                        .map(|x| x.as_object().cloned().unwrap_or_default());

                    if let Some(content_details) = content_details {
                        let video = parse_related_video(&content_details, &rvs_params);
                        if let Some(video_some) = video {
                            videos.push(video_some)
                        }
                    }
                }
            }
        }
    }

    Some(videos)
}

pub fn parse_related_video(
    details: &Map<String, Value>,
    rvs_params: &[&str],
) -> Option<RelatedVideo> {
    #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
    struct QueryParams {
        id: String,
        short_view_count_text: String,
        length_seconds: String,
    }
    let mut view_count = if let Some(view_count_text) = details.get("viewCountText") {
        get_text(view_count_text).as_str().unwrap_or("").to_string()
    } else {
        "0".to_string()
    };

    let mut short_view_count =
        if let Some(short_view_count_text) = details.get("shortViewCountText") {
            get_text(short_view_count_text)
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            "0".to_string()
        };

    let first = |string: &str| {
        string
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
    };

    if !first(&short_view_count) {
        let rvs_details_index = rvs_params
            .iter()
            .map(|x| serde_qs::from_str::<QueryParams>(x).unwrap())
            .position(|r| {
                r.id == *details
                    .get("videoId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("0")
            });

        if let Some(rvs_details_index_some) = rvs_details_index {
            let rvs_params_to_short_view_count = rvs_params
                .get(rvs_details_index_some)
                .cloned()
                .unwrap_or("");

            short_view_count = serde_qs::from_str::<QueryParams>(rvs_params_to_short_view_count)
                .map(|x| x.short_view_count_text)
                .unwrap_or("0".to_string());
        }
    }

    view_count = if first(&view_count) {
        view_count
            .split(' ')
            .collect::<Vec<&str>>()
            .first()
            .cloned()
            .unwrap_or("")
            .to_string()
    } else {
        short_view_count
            .split(' ')
            .collect::<Vec<&str>>()
            .first()
            .cloned()
            .unwrap_or("")
            .to_string()
    };

    let is_live = details
        .get("badges")
        .map(|c| {
            c.as_array()
                .map(|x| {
                    x.iter()
                        .filter(|x| x["metadataBadgeRenderer"]["label"] == "LIVE NOW")
                        .count()
                        > 0
                })
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let browse_end_point = &details
        .get("shortBylineText")
        .map(|x| x["runs"][0]["navigationEndpoint"]["browseEndpoint"].clone())
        .unwrap_or_default();
    let channel_id = &browse_end_point["browseId"];
    let author_user = browse_end_point["canonicalBaseUrl"]
        .as_str()
        .map(|c| {
            c.split('/')
                .collect::<Vec<&str>>()
                .last()
                .cloned()
                .unwrap_or("")
        })
        .unwrap_or("");

    static VIEW_COUNT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r",").unwrap());

    let video = RelatedVideo {
        id: details
            .get("videoId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        title: if let Some(title) = details.get("title") {
            get_text(title).as_str().unwrap_or("").to_string()
        } else {
            String::from("")
        },
        url: if let Some(video_id) = details.get("videoId") {
            let id = video_id.as_str().unwrap_or("").to_string();
            if !id.is_empty() {
                format!("{}{}", BASE_URL, id)
            } else {
                String::from("")
            }
        } else {
            String::from("")
        },
        published: if let Some(published_time_text) = details.get("publishedTimeText") {
            get_text(published_time_text)
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            String::from("")
        },
        author: if !browse_end_point.is_null() {
            Some(Author {
                id: channel_id.as_str().unwrap_or("").to_string(),
                name: if let Some(text) = details.get("shortBylineText") {
                    get_text(text).as_str().unwrap_or("").to_string()
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
                    if author_user.starts_with('@') {
                        format!("https://www.youtube.com/{user}", user = author_user)
                    } else {
                        String::from("")
                    }
                } else {
                    String::from("")
                },
                thumbnails: if let Some(thumbnails) = details
                    .get("channelThumbnail")
                    .and_then(|x| x.get("thumbnails"))
                {
                    thumbnails
                        .as_array()
                        .map(|f| {
                            f.iter()
                                .map(|x| Thumbnail {
                                    width: x
                                        .get("width")
                                        .and_then(|x| {
                                            if x.is_string() {
                                                x.as_str()
                                                    .map(|x| x.parse::<i64>().unwrap_or_default())
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
                                                x.as_str()
                                                    .map(|x| x.parse::<i64>().unwrap_or_default())
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
                                .collect::<Vec<Thumbnail>>()
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                },
                verified: if let Some(badges) = details.get("ownerBadges") {
                    is_verified(badges)
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
            .first()
            .unwrap_or(&"")
            .to_string(),
        view_count: VIEW_COUNT_REGEX.replace_all(&view_count, "").to_string(),
        length_seconds: if let Some(length_text) = details.get("lengthText") {
            let time = (time_to_ms(get_text(length_text).as_str().unwrap_or("0")) / 1000) as f32;
            time.floor().to_string()
        } else {
            "0".to_string()
        },
        thumbnails: if let Some(thumbnails) =
            details.get("thumbnail").and_then(|x| x.get("thumbnails"))
        {
            thumbnails
                .as_array()
                .map(|f| {
                    f.iter()
                        .map(|x| Thumbnail {
                            width: x
                                .get("width")
                                .and_then(|x| {
                                    if x.is_string() {
                                        x.as_str().map(|x| x.parse::<i64>().unwrap_or_default())
                                    } else {
                                        x.as_i64()
                                    }
                                })
                                .unwrap_or(0i64) as u64,
                            height: x
                                .get("height")
                                .and_then(|x| {
                                    if x.is_string() {
                                        x.as_str().map(|x| x.parse::<i64>().unwrap_or_default())
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
                        .collect::<Vec<Thumbnail>>()
                })
                .unwrap_or_default()
        } else {
            vec![]
        },
        is_live,
    };

    Some(video)
}

pub fn get_media(info: &Value) -> Option<Value> {
    let results = info["contents"]["twoColumnWatchNextResults"]["results"]["results"]["contents"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let result_option = results
        .iter()
        .find(|x| !x["videoSecondaryInfoRenderer"].is_null());

    let json_result = if let Some(result) = result_option {
        let metadata_rows = if let Some(result) = result.get("metadataRowContainer") {
            result["metadataRowContainerRenderer"]["rows"].clone()
        } else if let Some(result) = result.get("videoSecondaryInfoRenderer") {
            result["metadataRowContainer"]["metadataRowContainerRenderer"]["rows"].clone()
        } else {
            serde_json::Value::Null
        };

        let mut return_object: Option<Value> = None;

        for row in metadata_rows.as_array().cloned().unwrap_or_default() {
            if let Some(row) = row.get("metadataRowRenderer") {
                let title = get_text(&row["title"]).as_str().unwrap_or("title");
                let contents_value = row["contents"].as_array().cloned().unwrap_or_default();

                let contents = contents_value.first().cloned().unwrap_or_default();

                let runs = contents["runs"].as_array().cloned().unwrap_or_default();
                let title_url = runs
                    .first()
                    .map(|x| {
                        x["navigationEndpoint"]["commandMetadata"]["webCommandMetadata"]["url"]
                            .clone()
                    })
                    .unwrap_or_default();

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
                    title_content = get_text(&contents).as_str().unwrap_or(""),
                );

                return_object = from_str(data.as_str()).unwrap_or_default();
            } else if let Some(row) = row.get("richMetadataRowRenderer") {
                let contents = row["contents"].as_array().cloned().unwrap_or_default();

                let box_art = contents.iter().filter(|x| {
                    x["richMetadataRenderer"]["style"].as_str().unwrap_or("")
                        == "RICH_METADATA_RENDERER_STYLE_BOX_ART"
                });

                let mut media_year = "";
                let mut media_type = "type";
                let mut media_type_title = "";
                let mut media_type_url = "";
                let mut media_thumbnails = json!([]);

                for box_art_value in box_art {
                    media_year = get_text(&box_art_value["richMetadataRenderer"]["subtitle"])
                        .as_str()
                        .unwrap_or("");

                    media_type = get_text(&box_art_value["richMetadataRenderer"]["callToAction"])
                        .as_str()
                        .unwrap_or("type")
                        .split(' ')
                        .collect::<Vec<&str>>()
                        .get(1)
                        .unwrap_or(&"type");

                    media_type_title = get_text(&box_art_value["richMetadataRenderer"]["title"])
                        .as_str()
                        .unwrap_or("");

                    media_type_url = box_art_value["richMetadataRenderer"]["endpoint"]
                        ["commandMetadata"]["webCommandMetadata"]["url"]
                        .as_str()
                        .unwrap_or("");

                    media_thumbnails =
                        box_art_value["richMetadataRenderer"]["thumbnail"]["thumbnails"].clone()
                }

                let topic = contents.iter().filter(|x| {
                    x["richMetadataRenderer"]["style"].as_str().unwrap_or("")
                        == "RICH_METADATA_RENDERER_STYLE_TOPIC"
                });

                let mut category = "";
                let mut category_url = "";

                for topic_value in topic {
                    category = get_text(&topic_value["richMetadataRenderer"]["title"])
                        .as_str()
                        .unwrap_or("");

                    category_url = topic_value["richMetadataRenderer"]["endpoint"]
                        ["commandMetadata"]["webCommandMetadata"]["url"]
                        .as_str()
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
                );

                return_object = from_str(data.as_str()).unwrap_or_default();
            }
        }

        return_object
    } else {
        None
    };

    json_result
}

pub fn get_author(initial_response: &Value, player_response: &PlayerResponse) -> Option<Author> {
    let mut results: Vec<Value> = vec![];

    let mut results_closure = || -> Result<(), &str> {
        results = initial_response["contents"]["twoColumnWatchNextResults"]["results"]["results"]
            ["contents"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .to_vec();

        Ok(())
    };

    if let Err(_err) = results_closure() {
        results = vec![];
    }

    let v_position = results
        .iter()
        .position(|x| !x["videoSecondaryInfoRenderer"]["owner"]["videoOwnerRenderer"].is_null())
        .unwrap_or(usize::MAX);

    // match v_position
    let v = results.get(v_position).cloned().unwrap_or_default();

    let video_ownder_renderer =
        v["videoSecondaryInfoRenderer"]["owner"]["videoOwnerRenderer"].clone();

    let channel_id = video_ownder_renderer["navigationEndpoint"]["browseEndpoint"]["browseId"]
        .as_str()
        .unwrap_or("");
    let thumbnails = video_ownder_renderer["thumbnail"]["thumbnails"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|x| Thumbnail {
            width: x
                .get("width")
                .and_then(|x| {
                    if x.is_string() {
                        x.as_str().map(|x| x.parse::<i64>().unwrap_or_default())
                    } else {
                        x.as_i64()
                    }
                })
                .unwrap_or(0i64) as u64,
            height: x
                .get("height")
                .and_then(|x| {
                    if x.is_string() {
                        x.as_str().map(|x| x.parse::<i64>().unwrap_or_default())
                    } else {
                        x.as_i64()
                    }
                })
                .unwrap_or(0i64) as u64,
            url: x["url"].as_str().unwrap_or("").to_string(),
        })
        .collect::<Vec<Thumbnail>>();
    let subscriber_count = parse_abbreviated_number(
        get_text(&video_ownder_renderer["subscriberCountText"])
            .as_str()
            .unwrap_or("0"),
    );
    let verified = is_verified(&video_ownder_renderer["badges"]);
    let video_details = player_response
        .micro_format
        .as_ref()
        .and_then(|x| x.player_micro_format_renderer.clone());

    let id = if !channel_id.is_empty() {
        channel_id.to_string()
    } else {
        player_response
            .video_details
            .as_ref()
            .and_then(|x| x.channel_id.clone())
            .unwrap_or("".to_string())
    };

    let user = if let Some(owner_profile_url) = video_details
        .as_ref()
        .and_then(|x| x.owner_profile_url.clone())
    {
        owner_profile_url
            .trim()
            .split('/')
            .collect::<Vec<&str>>()
            .last()
            .unwrap_or(&"")
            .to_string()
    } else {
        String::from("")
    };

    Some(Author {
        id: id.clone(),
        name: if let Some(owner_channel_name) = video_details
            .as_ref()
            .and_then(|x| x.owner_channel_name.clone())
        {
            owner_channel_name
        } else {
            player_response
                .video_details
                .as_ref()
                .and_then(|x| x.author.clone())
                .unwrap_or("".to_string())
        },
        user: user.clone(),
        channel_url: format!("https://www.youtube.com/channel/{id}", id = id),
        external_channel_url: if let Some(external_channel_id) = video_details
            .as_ref()
            .and_then(|x| x.external_channel_id.clone())
        {
            let mut return_string = String::from("");
            if !external_channel_id.is_empty() {
                return_string = format!("https://www.youtube.com/channel/{}", external_channel_id);
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
        subscriber_count: subscriber_count as u64,
    })
}

pub fn get_likes(info: &Value) -> u64 {
    let contents =
        info["contents"]["twoColumnWatchNextResults"]["results"]["results"]["contents"].clone();

    let video = contents
        .as_array()
        .map(|x| {
            let info_renderer_position = x
                .iter()
                .position(|c| c.get("videoPrimaryInfoRenderer").is_some())
                .unwrap_or(usize::MAX);

            contents
                .as_array()
                .and_then(|c| c.get(info_renderer_position).cloned())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let buttons = video["videoPrimaryInfoRenderer"]["videoActions"]["menuRenderer"]
        ["topLevelButtons"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let like_index = buttons
        .iter()
        .position(|x| x.get("segmentedLikeDislikeButtonViewModel").is_some())
        .unwrap_or(usize::MAX);

    let like = buttons.get(like_index).cloned().unwrap_or_default();

    let count = like["segmentedLikeDislikeButtonViewModel"]["likeButtonViewModel"]
        ["likeButtonViewModel"]["toggleButtonViewModel"]["toggleButtonViewModel"]
        ["defaultButtonViewModel"]["buttonViewModel"]["title"]
        .as_str()
        .unwrap_or("0");

    parse_abbreviated_number(count) as u64
}

pub fn get_dislikes(info: &Value) -> u64 {
    let contents =
        info["contents"]["twoColumnWatchNextResults"]["results"]["results"]["contents"].clone();

    let video = contents
        .as_array()
        .map(|x| {
            let info_renderer_position = x
                .iter()
                .position(|c| c.get("videoPrimaryInfoRenderer").is_some())
                .unwrap_or(usize::MAX);

            contents
                .as_array()
                .and_then(|c| c.get(info_renderer_position).cloned())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let buttons = video["videoPrimaryInfoRenderer"]["videoActions"]["menuRenderer"]
        ["topLevelButtons"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let dislike_index = buttons
        .iter()
        .position(|x| x.get("segmentedLikeDislikeButtonViewModel").is_some())
        .unwrap_or(usize::MAX);

    let dislike = buttons.get(dislike_index).cloned().unwrap_or_default();

    let count = dislike["segmentedLikeDislikeButtonViewModel"]["dislikeButtonViewModel"]
        ["dislikeButtonViewModel"]["toggleButtonViewModel"]["toggleButtonViewModel"]
        ["defaultButtonViewModel"]["buttonViewModel"]["title"]
        .as_str()
        .unwrap_or("0");

    parse_abbreviated_number(count) as u64
}

pub fn get_storyboards(info: &PlayerResponse) -> Option<Vec<StoryBoard>> {
    let parts = info
        .storyboards
        .as_ref()
        .and_then(|x| x.player_storyboard_spec_renderer.as_ref())
        .and_then(|x| x.spec.clone());

    // If storyboard spec is absent, return an empty vector
    if let Some(parts_binding) = parts {
        let mut parts = parts_binding.split('|').collect::<Vec<&str>>();

        let mut url = url::Url::parse(parts.remove(0))
            .unwrap_or_else(|_| url::Url::parse("https://i.ytimg.com/").unwrap());

        let storyboards = parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                let part_split = part.split('#').collect::<Vec<&str>>();

                let thumbnail_width = part_split
                    .first()
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let thumbnail_height = part_split
                    .get(1)
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let thumbnail_count = part_split
                    .get(2)
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let columns = part_split
                    .get(3)
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let rows = part_split
                    .get(4)
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let interval = part_split
                    .get(5)
                    .unwrap_or(&"0")
                    .parse::<i32>()
                    .unwrap_or(0);
                let name_replacement = part_split.get(6).unwrap_or(&"0");
                let sigh = part_split.get(7).unwrap_or(&"0");

                url.query_pairs_mut().append_pair("sigh", sigh);

                let storyboard_count_ceiled = thumbnail_count / (columns * rows);

                let template_url = url
                    .as_str()
                    .replace("$L", &i.to_string())
                    .replace("$N", name_replacement);

                StoryBoard {
                    template_url: template_url.to_string(),
                    thumbnail_width,
                    thumbnail_height,
                    thumbnail_count,
                    interval,
                    columns,
                    rows,
                    storyboard_count: storyboard_count_ceiled,
                }
            })
            .collect::<Vec<StoryBoard>>();

        Some(storyboards)
    } else {
        Some(Vec::new())
    }
}

pub fn get_chapters(info: &Value) -> Option<Vec<Chapter>> {
    let markers_map = info["playerOverlays"]["playerOverlayRenderer"]["decoratedPlayerBarRenderer"]
        ["decoratedPlayerBarRenderer"]["playerBar"]["multiMarkersPlayerBarRenderer"]["markersMap"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let marker_index = markers_map
        .iter()
        .position(|x| {
            x.get("value")
                .map(|c| c.get("chapters").map(|d| d.is_array()).unwrap_or(false))
                .unwrap_or(false)
        })
        .unwrap_or(usize::MAX);

    let marker = markers_map
        .get(marker_index)
        .map(|x| x.as_object().cloned().unwrap_or_default())
        .unwrap_or_default();

    if marker.is_empty() {
        return Some(vec![]);
    }

    let chapters = marker
        .get("value")
        .and_then(|x| x.get("chapters"))
        .and_then(|x| x.as_array().cloned())
        .unwrap_or_default();

    Some(
        chapters
            .iter()
            .map(|x| Chapter {
                title: get_text(&x["chapterRenderer"]["title"])
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                start_time: (x["chapterRenderer"]["timeRangeStartMillis"]
                    .as_f64()
                    .unwrap_or_default()
                    / 1000f64) as i32,
            })
            .collect::<Vec<Chapter>>(),
    )
}
