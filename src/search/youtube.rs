use std::sync::{Arc, RwLock};

use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use urlencoding::encode;

use crate::{
    constants::DEFAULT_HEADERS,
    structs::VideoError,
    utils::{get_html, get_random_v6_ip, time_to_ms},
    Thumbnail,
};

pub use crate::structs::RequestOptions;

const DEFAULT_INNERTUBE_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const DEFAULT_CLIENT_VERSOIN: &str = "2.20230331.00.00";
const SAFE_SEARCH_COOKIE: &str = "PREF=f2=8000000";

static PLAYLIST_ID: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(PL|FL|UU|LL|RD|OL)[a-zA-Z0-9-_]{16,41}").unwrap());

static ALBUM_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(RDC|O)LAK5uy_[a-zA-Z0-9-_]{33}").unwrap());

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "YouTube()")]
#[derivative(Debug, PartialEq, Eq)]
pub struct YouTube {
    #[derivative(PartialEq = "ignore")]
    client: reqwest_middleware::ClientWithMiddleware,
    #[derivative(PartialEq = "ignore")]
    innertube_cache: Arc<RwLock<Option<String>>>,
}

impl YouTube {
    /// Create new YouTube search struct with default [`RequestOptions`]
    pub fn new() -> Result<Self, VideoError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(VideoError::Reqwest)?;

        let client = reqwest_middleware::ClientBuilder::new(client).build();

        Ok(Self {
            client,
            innertube_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Create new YouTube search struct with custom [`RequestOptions`]
    pub fn new_with_options(request_options: &RequestOptions) -> Result<Self, VideoError> {
        let client = if let Some(client) = request_options.client.as_ref() {
            client.clone()
        } else {
            let mut client = reqwest::Client::builder();

            // Assign request options to client

            if let Some(proxy) = request_options.proxy.as_ref() {
                client = client.proxy(proxy.clone());
            }

            if let Some(ipv6_block) = request_options.ipv6_block.as_ref() {
                let ipv6 = get_random_v6_ip(ipv6_block)?;
                client = client.local_address(ipv6);
            }

            if let Some(cookie) = request_options.cookies.as_ref() {
                let host = "https://youtube.com".parse::<url::Url>().unwrap();

                let jar = reqwest::cookie::Jar::default();
                jar.add_cookie_str(cookie, &host);

                client = client.cookie_provider(Arc::new(jar));
            }

            client.build().map_err(VideoError::Reqwest)?
        };

        let client = reqwest_middleware::ClientBuilder::new(client).build();

        Ok(Self {
            client,
            innertube_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Search with spesific `query`. If nothing found, its return empty [`Vec<SearchResult>`]
    /// # Example
    /// ```ignore
    ///     let youtube = YouTube::new().unwrap();
    ///
    ///     let res = youtube.search("i know your ways", None).await;
    ///
    ///     println!("{res:#?}");
    /// ```
    pub async fn search(
        &self,
        query: impl Into<String>,
        search_options: Option<&SearchOptions>,
    ) -> Result<Vec<SearchResult>, VideoError> {
        let default_options = SearchOptions::default();

        // if SearchOptions is None get default
        let options: &SearchOptions = if let Some(some_search_options) = search_options {
            some_search_options
        } else {
            &default_options
        };

        let query: String = query.into();
        let filter = filter_string(&options.search_type);
        let query_regex = Regex::new(r"%20").unwrap();

        // First try with youtube backend
        let res = make_request(
            &self.client,
            self.innertube_key().await,
            "/search",
            options,
            &RequestFuncOptions {
                query: query.clone(),
                filter: if !filter.trim().is_empty() {
                    Some(filter.clone())
                } else {
                    None
                },
                original_url: format!(
                    "https://youtube.com/results?search_query={encoded_query}{filter}",
                    encoded_query = query_regex.replace(&encode(query.trim()), "+")
                ),
            },
        )
        .await;

        // make_request success
        if !res.is_null()
            && !res["contents"]["twoColumnSearchResultsRenderer"]["primaryContents"]
                ["sectionListRenderer"]["contents"][0]["itemSectionRenderer"]["contents"]
                .is_null()
        {
            return Ok(format_search_result(
                &self.client,
                &res["contents"]["twoColumnSearchResultsRenderer"]["primaryContents"]
                    ["sectionListRenderer"]["contents"][0]["itemSectionRenderer"]["contents"],
                options,
            ));
        }

        // get html body if backend return null
        let filter = if options.search_type == SearchType::All {
            "".to_string()
        } else {
            format!("&sp={}", filter_string(&options.search_type))
        };

        let url = format!(
            "https://youtube.com/results?search_query={encoded_query}&hl=en{filter}",
            encoded_query = query_regex.replace(&encode(query.trim()), "+")
        );

        let mut headers = DEFAULT_HEADERS.clone();

        // if search_options.safe_search is true assign safe search cookie to reqwest request
        if options.safe_search {
            headers.insert(
                reqwest::header::COOKIE,
                reqwest::header::HeaderValue::from_str(SAFE_SEARCH_COOKIE)
                    .expect("SAFE_SEARCH_COOKIE contain not ASCII"),
            );
        }

        let body = get_html(&self.client, url, Some(&headers)).await?;

        Ok(parse_search_result(&self.client, body, options))
    }

    /// Classic search function but only get first [`SearchResult`] item. `SearchOptions.limit` not use in request its will be always `1`
    pub async fn search_one(
        &self,
        query: impl Into<String>,
        search_options: Option<&SearchOptions>,
    ) -> Result<Option<SearchResult>, VideoError> {
        let search_options = if let Some(some_search_options) = search_options {
            SearchOptions {
                limit: 1,
                ..some_search_options.clone()
            }
        } else {
            SearchOptions {
                limit: 1,
                ..Default::default()
            }
        };

        let res = self.search(query, Some(&search_options)).await?;

        Ok(res.first().cloned())
    }

    /// Fetch search suggestion with specific `query`
    /// # Example
    /// ```ignore
    /// let youtube = YouTube::new().unwrap();
    /// 
    /// let res = youtube.suggestion("i know ").await;
    /// 
    /// println!("{res:#?}");
    /// ```
    pub async fn suggestion(
        &self,
        query: impl Into<String>,
    ) -> Result<Vec<String>, VideoError> {
        let query: String = query.into();
        let url = format!(
            "https://suggestqueries-clients6.youtube.com/complete/search?client=android&q={query}",
            query = encode(query.trim())
        );

        let body = get_html(&self.client, url, None).await?;

        let serde_value = serde_json::from_str::<serde_json::Value>(&body).unwrap();

        let suggestion = serde_value
            .as_array()
            .unwrap()
            .get(1)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .collect();

        return Ok(suggestion);
    }

    async fn innertube_key(&self) -> String {
        {
            let innertube_cache = self.innertube_cache.read().unwrap();
            if innertube_cache.is_some() {
                return innertube_cache.as_ref().unwrap().to_string();
            }
        }

        self.fetch_inner_tube_key().await
    }

    async fn fetch_inner_tube_key(&self) -> String {
        let response = get_html(
            &self.client,
            "https://www.youtube.com?hl=en",
            Some(&DEFAULT_HEADERS.clone()),
        )
        .await;

        if response.is_err() {
            return DEFAULT_INNERTUBE_KEY.to_string();
        }

        let response = response.unwrap();

        let result = get_api_key(response);

        let mut innertube_cache_data = self.innertube_cache.write().unwrap();

        *innertube_cache_data = Some(result.clone());
        result
    }
}

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[derivative(Debug, PartialEq, Eq)]
pub enum SearchType {
    Video,
    Channel,
    Playlist,
    Film,
    All,
}

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "SearchOptions()")]
#[derivative(Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub limit: u64,
    pub search_type: SearchType,
    pub safe_search: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 100,
            search_type: SearchType::Video,
            safe_search: false,
        }
    }
}

struct RequestFuncOptions {
    query: String,
    filter: Option<String>,
    original_url: String,
}

pub struct PlaylistSearchOptions {
    pub limit: u64,
    pub request_options: Option<RequestOptions>,
    /// Fetch all videos and avoid limit
    pub fetch_all: bool,
}

impl Default for PlaylistSearchOptions {
    fn default() -> Self {
        Self {
            limit: 100,
            request_options: None,
            fetch_all: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchResult {
    Video(Video),
    Playlist(Playlist),
    Channel(Channel),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub id: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub duration: u64,
    pub duration_raw: String,
    pub thumbnails: Vec<Thumbnail>,
    pub channel: Channel,
    pub uploaded_at: Option<String>,
    pub views: u64,
}

impl Video {
    /// Get video embed url with [`EmbedOptions`]
    /// - if [`Video`] id is empty or corrupted, this function return [`None`]
    pub fn get_embed_html(&self, options: Option<&EmbedOptions>) -> Option<String> {
        if self.id.trim().is_empty() {
            return None;
        }

        let default_embed_options = EmbedOptions::default();

        let options = if let Some(some_options) = options {
            drop(default_embed_options);
            some_options
        } else {
            &default_embed_options
        };

        let width = options.width.unwrap_or(600);
        let height = options.height.unwrap_or(480);
        let title = options
            .title
            .clone()
            .unwrap_or("YouTube video player".to_string());

        let id = &self.id;

        let mut url = if options.secure_url {
            let url = url::Url::parse(&format!("https://www.youtube-nocookie.com/embed/{id}"));

            if url.is_err() {
                return None;
            }

            url.unwrap()
        } else {
            let url = url::Url::parse(&format!("https://www.youtube.com/embed/{id}"));

            if url.is_err() {
                return None;
            }

            url.unwrap()
        };

        if !options.controls {
            url.query_pairs_mut().append_pair("controls", "0");
        }

        if options.start_time > 0 {
            url.query_pairs_mut()
                .append_pair("start", &options.start_time.to_string());
        }

        let src = url.to_string();
        Some(format!(
            r#"<iframe width="{width}" height="{height}" src="{src}" title="{title}" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share" allowfullscreen></iframe>"#
        ))
    }

    /// Get YouTube embed URL. If [`Video`] id is empty, this function return [`None`]
    pub fn get_embed_url(&self) -> Option<String> {
        if self.id.trim().is_empty() {
            return None;
        }

        Some(format!("https://www.youtube.com/embed/{}", self.id))
    }
}

#[derive(Clone, derivative::Derivative)]
#[derivative(Debug, PartialEq, Eq)]
pub struct EmbedOptions {
    width: Option<u64>,
    height: Option<u64>,
    title: Option<String>,
    /// Time in seconds. `0` is begin of the video
    start_time: u64,
    /// Will dont send personal cookies
    secure_url: bool,
    /// Enable or disable video controls. Default is `true`
    controls: bool,
}

impl Default for EmbedOptions {
    fn default() -> Self {
        Self {
            width: Some(600),
            height: Some(480),
            title: Some("YouTube video player".to_string()),
            start_time: 0,
            secure_url: false,
            controls: true,
        }
    }
}

#[derive(Clone, derivative::Derivative, Serialize)]
#[derivative(Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub url: String,
    pub channel: Channel,
    pub thumbnails: Vec<Thumbnail>,
    pub views: u64,
    pub videos: Vec<Video>,
    pub last_update: Option<String>,

    #[serde(skip_serializing)]
    #[derivative(PartialEq = "ignore")]
    continuation: Option<Continuation>,
    #[serde(skip_serializing)]
    #[derivative(PartialEq = "ignore")]
    client: reqwest_middleware::ClientWithMiddleware,
}

impl Playlist {
    /// Try to get [`Playlist`] than fetch videos according to the [`PlaylistSearchOptions`]
    pub async fn get(
        url: impl Into<String>,
        options: Option<&PlaylistSearchOptions>,
    ) -> Result<Self, VideoError> {
        let url: String = url.into();
        let default_options = PlaylistSearchOptions::default();
        let options = if let Some(some_options) = options {
            drop(default_options);
            some_options
        } else {
            &default_options
        };

        if !Self::is_playlist(&url) {
            return Err(VideoError::IsNotPlaylist(url.clone()));
        }

        let url_option = Self::get_playlist_url(&url);

        if url_option.is_none() {
            return Err(VideoError::IsNotPlaylist(url.clone()));
        }

        let url = url_option.unwrap();

        // Assign request options to client
        let mut client = reqwest::Client::builder();

        if options
            .request_options
            .as_ref()
            .map(|x| x.proxy.is_some())
            .unwrap_or(false)
        {
            let proxy = options
                .request_options
                .as_ref()
                .unwrap()
                .proxy
                .as_ref()
                .unwrap()
                .clone();
            client = client.proxy(proxy);
        }

        if options
            .request_options
            .as_ref()
            .map(|x| x.ipv6_block.is_some())
            .unwrap_or(false)
        {
            let ipv6 = options
                .request_options
                .as_ref()
                .unwrap()
                .ipv6_block
                .as_ref()
                .unwrap();
            let ipv6 = get_random_v6_ip(ipv6)?;
            client = client.local_address(ipv6);
        }

        if options
            .request_options
            .as_ref()
            .map(|x| x.cookies.is_some())
            .unwrap_or(false)
        {
            let cookie = options
                .request_options
                .as_ref()
                .unwrap()
                .cookies
                .as_ref()
                .unwrap();
            let host = "https://youtube.com".parse::<url::Url>().unwrap();

            let jar = reqwest::cookie::Jar::default();
            jar.add_cookie_str(cookie.as_str(), &host);

            client = client.cookie_provider(Arc::new(jar));
        }

        let client = client.build().map_err(VideoError::Reqwest)?;
        let client = reqwest_middleware::ClientBuilder::new(client).build();

        let html_first = get_html(
            &client,
            format!("{url}&hl=en"),
            Some(&DEFAULT_HEADERS.clone()),
        )
        .await?;

        // Get playlist datas
        let html = {
            let document = Html::parse_document(&html_first);
            let scripts_selector = Selector::parse("script").unwrap();
            let mut initial_response_string = document
                .select(&scripts_selector)
                .filter(|x| x.inner_html().contains("var ytInitialData ="))
                .map(|x| x.inner_html().replace("var ytInitialData =", ""))
                .next()
                .unwrap_or(String::from(""))
                .trim()
                .to_string();

            initial_response_string.pop();

            initial_response_string
        };

        if !html.is_empty() {
            let serde_value = serde_json::from_str::<serde_json::Value>(&html).unwrap();
            let contents = &serde_value["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][0]
                ["tabRenderer"]["content"]["sectionListRenderer"]["contents"][0]
                ["itemSectionRenderer"]["contents"][0]["playlistVideoListRenderer"]["contents"];

            let playlist_primary_data = &serde_value["sidebar"]["playlistSidebarRenderer"]["items"]
                [0]["playlistSidebarPrimaryInfoRenderer"];
            let playlist_secondary_data = &serde_value["sidebar"]["playlistSidebarRenderer"]
                ["items"][1]["playlistSidebarSecondaryInfoRenderer"];

            // if contents found try to format values
            if !contents.is_null() && !playlist_primary_data.is_null() {
                let videos = Self::get_playlist_videos(contents, Some(options.limit));

                let videos_length = videos.len();
                let mut playlist = Playlist {
                    id: playlist_primary_data["title"]["runs"][0]["navigationEndpoint"]
                        ["watchEndpoint"]["playlistId"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    name: playlist_primary_data["title"]["runs"][0]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    url: if playlist_primary_data["title"]["runs"][0]["navigationEndpoint"]
                        ["watchEndpoint"]["playlistId"]
                        .is_string()
                    {
                        format!(
                            "https://www.youtube.com/playlist?list={}",
                            playlist_primary_data["title"]["runs"][0]["navigationEndpoint"]
                                ["watchEndpoint"]["playlistId"]
                                .as_str()
                                .unwrap_or("")
                        )
                    } else {
                        String::from("")
                    },
                    channel: Channel {
                        id: playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]["title"]
                            ["runs"][0]["navigationEndpoint"]["browseEndpoint"]["browseId"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        name: playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]["title"]
                            ["runs"][0]["text"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        url: if playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]
                            ["navigationEndpoint"]["commandMetadata"]["webCommandMetadata"]["url"]
                            .is_string()
                        {
                            playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]
                                ["navigationEndpoint"]["commandMetadata"]["webCommandMetadata"]
                                ["url"]
                                .as_str()
                                .unwrap_or("")
                                .to_string()
                        } else if playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]
                            ["navigationEndpoint"]["browseEndpoint"]["canonicalBaseUrl"]
                            .is_string()
                        {
                            playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]
                                ["navigationEndpoint"]["browseEndpoint"]["canonicalBaseUrl"]
                                .as_str()
                                .unwrap_or("")
                                .to_string()
                        } else {
                            String::from("")
                        },
                        icon: if playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]
                            ["thumbnail"]["thumbnails"]
                            .is_array()
                        {
                            playlist_secondary_data["videoOwner"]["videoOwnerRenderer"]["thumbnail"]
                                ["thumbnails"]
                                .as_array()
                                .unwrap()
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
                                        .unwrap_or(0i64)
                                        as u64,
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
                                        .unwrap_or(0i64)
                                        as u64,
                                    url: x
                                        .get("url")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect::<Vec<Thumbnail>>()
                        } else {
                            vec![]
                        },
                        verified: false,
                        subscribers: 0,
                    },
                    thumbnails: if playlist_primary_data["thumbnailRenderer"]
                        ["playlistVideoThumbnailRenderer"]["thumbnail"]["thumbnails"]
                        .is_array()
                    {
                        playlist_primary_data["thumbnailRenderer"]["playlistVideoThumbnailRenderer"]
                            ["thumbnail"]["thumbnails"]
                            .as_array()
                            .unwrap()
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
                            .collect::<Vec<Thumbnail>>()
                    } else {
                        vec![]
                    },
                    views: if playlist_primary_data["stats"][1]["simpleText"].is_string() {
                        let only_numbers = Regex::new(r"[^0-9]").unwrap();
                        let view_count = only_numbers.replace_all(
                            playlist_primary_data["stats"][1]["simpleText"]
                                .as_str()
                                .unwrap_or(""),
                            "",
                        );

                        view_count.parse::<u64>().unwrap_or(0)
                    } else {
                        0
                    },
                    videos,
                    last_update: if playlist_primary_data["stats"].is_array() {
                        playlist_primary_data["stats"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|x| {
                                if x["runs"].is_array() {
                                    x["runs"].as_array().unwrap().iter().any(|c| {
                                        c["text"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_lowercase()
                                            .contains("last update")
                                    })
                                } else {
                                    false
                                }
                            })
                            .and_then(|x| {
                                if x["runs"].is_array() {
                                    x["runs"]
                                        .as_array()
                                        .unwrap()
                                        .last()
                                        .map(|x| x["text"].as_str().unwrap_or("").to_string())
                                } else {
                                    None
                                }
                            })
                    } else {
                        None
                    },
                    continuation: Some(Continuation {
                        api: Some(get_api_key(&html_first)),
                        token: Self::get_continuation_token(contents),
                        client_version: Some(get_client_version(&html_first)),
                    }),
                    client,
                };

                // we will try to fetch all videos from playlist
                if options.fetch_all {
                    playlist.fetch(None).await;

                // if fetch_all false but limit not reached we can try to reach limit
                } else if options.limit > videos_length as u64 {
                    playlist.fetch(Some(options.limit)).await;
                }

                return Ok(playlist);
            }
        }

        Err(VideoError::PlaylistBodyCannotParsed)
    }

    /// Get next chunk of videos from playlist and return fetched [`Video`] array.
    /// - If limit is [`None`] it will be [`u64::MAX`]
    /// - If [`Playlist`] is coming from [`SearchResult`] this function always return empty [`Vec<Video>`]!
    /// to use this function with [`SearchResult`] follow example
    ///
    /// # Example
    ///
    /// ```ignore
    /// let youtube = YouTube::new().unwrap();
    ///
    /// let res = youtube
    ///    .search(
    ///       "manga",
    ///       Some(&SearchOptions {
    ///           search_type: SearchType::Playlist,
    ///           ..Default::default()
    ///       }),
    /// )
    /// .await;
    ///
    /// for result in res.unwrap() {
    ///    match result {
    ///       SearchResult::Playlist(raw_playlist) => {
    ///            let mut playlist = Playlist::get(raw_playlist.url, None).await;
    ///            playlist.unwrap().next(Some(50)).await.unwrap();
    ///       }
    ///       _ => {}
    ///    }
    /// }
    /// ```
    pub async fn next(&mut self, limit: Option<u64>) -> Result<Vec<Video>, VideoError> {
        let limit = limit.unwrap_or(u64::MAX);

        if self.continuation.is_none()
            || self
                .continuation
                .as_ref()
                .map(|x| x.token.is_none())
                .unwrap_or(true)
        {
            return Ok(vec![]);
        }

        // request body
        let continuation_token = self
            .continuation
            .as_ref()
            .and_then(|x| x.token.clone())
            .unwrap_or("".to_string());
        let mut client_version = self
            .continuation
            .as_ref()
            .and_then(|x| x.client_version.clone())
            .unwrap_or("".to_string());

        if client_version.is_empty() {
            client_version = "".to_string();
        } else {
            client_version = format!(r#""clientVersion": "{client_version}""#);
        }

        let continuation_api = self
            .continuation
            .as_ref()
            .and_then(|x| x.api.clone())
            .unwrap_or("".to_string());

        let format_str = format!(
            r#"{{
                "continuation": "{continuation_token}",
                "context": {{
                    "client": {{
                        "utcOffsetMinutes": 0,
                        "gl": "US",
                        "hl": "en",
                        "clientName": "WEB",
                        {client_version}
                    }},
                    "user": {{}},
                    "request": {{}}
                }}
            }}
            "#
        );

        // Get json object with continuation token
        let body: serde_json::Value = serde_json::from_str(&format_str).unwrap();

        let res = self
            .client
            .post(format!(
                "https://www.youtube.com/youtubei/v1/browse?key={continuation_api}"
            ))
            .json(&body)
            .send()
            .await;

        if res.is_err() {
            return Err(VideoError::ReqwestMiddleware(res.err().unwrap()));
        }

        let res = res.unwrap().json::<serde_json::Value>().await;

        if res.is_err() {
            return Err(VideoError::BodyCannotParsed);
        }
        let res = res.unwrap();

        let contents = res["onResponseReceivedActions"][0]["appendContinuationItemsAction"]
            ["continuationItems"]
            .clone();

        if contents.is_null() {
            return Ok(vec![]);
        }

        let fetched_videos = Self::get_playlist_videos(&contents, Some(limit));

        self.continuation = Some(Continuation {
            token: Self::get_continuation_token(&contents),
            api: self.continuation.as_ref().and_then(|x| x.api.clone()),
            client_version: self
                .continuation
                .as_ref()
                .and_then(|x| x.client_version.clone()),
        });

        self.videos.extend(fetched_videos.clone());

        Ok(fetched_videos)
    }

    /// Try to fetch all playlist videos and return [`Playlist`].
    /// - If limit is [`None`] it will be [`u64::MAX`]
    /// - If [`Playlist`] is coming from [`SearchResult`] this function always return [`Playlist`] with empty [`Vec<Video>`]!
    /// to use this function with [`SearchResult`] follow example
    ///
    /// # Example
    ///
    /// ```ignore
    /// let youtube = YouTube::new().unwrap();
    ///
    /// let res = youtube
    ///    .search(
    ///       "manga",
    ///       Some(&SearchOptions {
    ///           search_type: SearchType::Playlist,
    ///           ..Default::default()
    ///       }),
    /// )
    /// .await;
    ///
    /// for result in res.unwrap() {
    ///    match result {
    ///       SearchResult::Playlist(raw_playlist) => {
    ///            let playlist = Playlist::get(raw_playlist.url, None).await;
    ///            let playlist = playlist.unwrap().fetch(None).await;
    ///       }
    ///       _ => {}
    ///    }
    /// }
    /// ```
    pub async fn fetch(&mut self, limit: Option<u64>) -> &mut Self {
        let limit = limit.unwrap_or(u64::MAX);
        // if continuation token not found return self without fetch videos
        let if_and_while_situation = self.continuation.is_none()
            || self
                .continuation
                .as_ref()
                .and_then(|x| x.token.clone())
                .is_none();

        if if_and_while_situation {
            return self;
        }

        while !(self.continuation.is_none()
            || self
                .continuation
                .as_ref()
                .and_then(|x| x.token.clone())
                .is_none())
        {
            if self.videos.len() as u64 >= limit {
                break;
            }
            let chunk = self.next(Some(limit)).await;

            // if error encountered finish the job
            if chunk.is_err() {
                break;
            }

            // if any not new data finish the job
            if chunk.unwrap().is_empty() {
                break;
            }
        }

        self
    }

    pub fn is_playlist(url_or_id: impl Into<String>) -> bool {
        let url_or_id: String = url_or_id.into();

        if PLAYLIST_ID.is_match(&url_or_id) || ALBUM_REGEX.is_match(&url_or_id) {
            return true;
        }

        false
    }

    pub fn get_playlist_url(url_or_id: impl Into<String>) -> Option<String> {
        let url_or_id: String = url_or_id.into();
        let matched_id = if PLAYLIST_ID.captures(&url_or_id).is_some() {
            PLAYLIST_ID
                .captures(&url_or_id)
                .unwrap()
                .get(0)
                .map(|x| x.as_str())
                .unwrap_or("")
        } else if ALBUM_REGEX.captures(&url_or_id).is_some() {
            ALBUM_REGEX
                .captures(&url_or_id)
                .unwrap()
                .get(0)
                .map(|x| x.as_str())
                .unwrap_or("")
        } else {
            ""
        }
        .trim();

        if matched_id.is_empty() {
            return None;
        }

        // Mixed URLs not allowed
        if matched_id.starts_with("RD") && !ALBUM_REGEX.is_match(matched_id) {
            return None;
        }

        Some(format!(
            "https://www.youtube.com/playlist?list={matched_id}"
        ))
    }

    fn get_playlist_videos(container: &serde_json::Value, limit: Option<u64>) -> Vec<Video> {
        let limit = limit.unwrap_or(u64::MAX);

        let mut videos: Vec<Video> = vec![];

        if !container.is_array() {
            return vec![];
        }

        for info in container.as_array().unwrap() {
            // If limit reached break the loop
            if limit == videos.len() as u64 {
                break;
            }

            let video = &info["playlistVideoRenderer"];
            // video not proper type skip it!
            if video.is_null() || video["shortBylineText"].is_null() {
                continue;
            }

            videos.push(Video {
                id: video["videoId"].as_str().unwrap_or("").to_string(),
                url: if video["videoId"].is_string() {
                    video["videoId"].as_str().unwrap_or("").to_string()
                } else {
                    String::from("")
                },
                title: video["title"]["runs"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                description: "".to_string(),
                duration: if !video["lengthText"]["simpleText"].is_null() {
                    time_to_ms(video["lengthText"]["simpleText"].as_str().unwrap_or("0:00")) as u64
                } else {
                    0
                },
                duration_raw: if !video["lengthText"]["simpleText"].is_null() {
                    video["lengthText"]["simpleText"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                } else {
                    "0:00".to_string()
                },
                thumbnails: if video["thumbnail"]["thumbnails"].is_array() {
                    video["thumbnail"]["thumbnails"]
                        .as_array()
                        .unwrap()
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
                        .collect::<Vec<Thumbnail>>()
                } else {
                    vec![]
                },
                channel: Channel {
                    id: video["shortBylineText"]["runs"][0]["navigationEndpoint"]["browseEndpoint"]
                        ["browseId"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    name: video["shortBylineText"]["runs"][0]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    url: if video["shortBylineText"]["runs"][0]["navigationEndpoint"]
                        ["browseEndpoint"]["canonicalBaseUrl"]
                        .is_string()
                    {
                        format!(
                            "https://www.youtube.com{}",
                            video["shortBylineText"]["runs"][0]["navigationEndpoint"]
                                ["browseEndpoint"]["canonicalBaseUrl"]
                                .as_str()
                                .unwrap_or("")
                        )
                    } else if video["shortBylineText"]["runs"][0]["navigationEndpoint"]
                        ["commandMetadata"]["webCommandMetadata"]["url"]
                        .is_string()
                    {
                        format!(
                            "https://www.youtube.com{}",
                            video["shortBylineText"]["runs"][0]["navigationEndpoint"]
                                ["commandMetadata"]["webCommandMetadata"]["url"]
                                .as_str()
                                .unwrap_or("")
                        )
                    } else {
                        String::from("")
                    },
                    icon: vec![],
                    verified: false,
                    subscribers: 0,
                },
                uploaded_at: None,
                views: 0,
            });
        }

        videos
    }

    fn get_continuation_token(context: &serde_json::Value) -> Option<String> {
        // if context is not array return none
        if !context.is_array() {
            return None;
        }

        let continuation_token = context.as_array().unwrap().iter().find(|x| {
            x.as_object()
                .map(|x| x.contains_key("continuationItemRenderer"))
                .unwrap_or(false)
        });

        if let Some(token) = continuation_token {
            let continuation_token = &token["continuationItemRenderer"]["continuationEndpoint"]
                ["continuationCommand"]["token"];

            if continuation_token.is_string() {
                return Some(continuation_token.as_str().unwrap_or("").to_string());
            }

            None
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continuation {
    api: Option<String>,
    token: Option<String>,
    client_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub url: String,
    pub icon: Vec<Thumbnail>,
    pub verified: bool,
    pub subscribers: u64,
}

fn filter_string(filter: &SearchType) -> String {
    match filter {
        SearchType::Video => "EgIQAQ%253D%253D".to_string(),
        SearchType::Channel => "EgIQAg%253D%253D".to_string(),
        SearchType::Playlist => "EgIQAw%253D%253D".to_string(),
        SearchType::Film => "EgIQBA%253D%253D".to_string(),
        SearchType::All => "".to_string(),
    }
}

fn get_client_version(html: impl Into<String>) -> String {
    let html: String = html.into();
    let first_collect_for_client_version = html
        .split(r#""INNERTUBE_CONTEXT_CLIENT_VERSION":""#)
        .collect::<Vec<&str>>();

    return match first_collect_for_client_version.get(1) {
        Some(x) => {
            let second_collect = x.split('"').collect::<Vec<&str>>();
            if !second_collect.is_empty() {
                let inner_tube = second_collect.first().unwrap().to_string();
                // println!("INNERTUBE_CONTEXT_CLIENT_VERSION => {inner_tube}");

                inner_tube
            } else {
                let third_collect = html
                    .split(r#""innertube_context_client_version":""#)
                    .collect::<Vec<&str>>();

                match third_collect.get(1) {
                    Some(c) => {
                        let forth_collect = c.split('"').collect::<Vec<&str>>();
                        if !forth_collect.is_empty() {
                            let inner_tube = forth_collect.first().unwrap().to_string();
                            // println!("innertube_context_client_version => {inner_tube}");
                            inner_tube
                        } else {
                            DEFAULT_CLIENT_VERSOIN.to_string()
                        }
                    }
                    None => DEFAULT_CLIENT_VERSOIN.to_string(),
                }
            }
        }
        None => {
            let third_collect = html
                .split(r#""innertube_context_client_version":""#)
                .collect::<Vec<&str>>();

            match third_collect.get(1) {
                Some(c) => {
                    let forth_collect = c.split('"').collect::<Vec<&str>>();
                    if !forth_collect.is_empty() {
                        let inner_tube = forth_collect.first().unwrap().to_string();
                        // println!("innertube_context_client_version => {inner_tube}");
                        inner_tube
                    } else {
                        DEFAULT_CLIENT_VERSOIN.to_string()
                    }
                }
                None => DEFAULT_CLIENT_VERSOIN.to_string(),
            }
        }
    };
}

fn get_api_key(html: impl Into<String>) -> String {
    let html: String = html.into();

    let first_collect = html
        .split(r#""INNERTUBE_API_KEY":""#)
        .collect::<Vec<&str>>();

    return match first_collect.get(1) {
        Some(x) => {
            let second_collect = x.split('"').collect::<Vec<&str>>();
            if !second_collect.is_empty() {
                let inner_tube = second_collect.first().unwrap().to_string();
                // println!("INNERTUBE_API_KEY => {inner_tube}");
                inner_tube
            } else {
                let third_collect = html.split(r#""innertubeApiKey":""#).collect::<Vec<&str>>();

                match third_collect.get(1) {
                    Some(c) => {
                        let forth_collect = c.split('"').collect::<Vec<&str>>();
                        if !forth_collect.is_empty() {
                            let inner_tube = forth_collect.first().unwrap().to_string();
                            // println!("innertubeApiKey => {inner_tube}");

                            inner_tube
                        } else {
                            DEFAULT_INNERTUBE_KEY.to_string()
                        }
                    }
                    None => DEFAULT_INNERTUBE_KEY.to_string(),
                }
            }
        }
        None => {
            let third_collect = html.split(r#""innertubeApiKey":""#).collect::<Vec<&str>>();

            match third_collect.get(1) {
                Some(c) => {
                    let forth_collect = c.split('"').collect::<Vec<&str>>();
                    if !forth_collect.is_empty() {
                        let inner_tube = forth_collect.first().unwrap().to_string();
                        // println!("innertubeApiKey => {inner_tube}");
                        inner_tube
                    } else {
                        DEFAULT_INNERTUBE_KEY.to_string()
                    }
                }
                None => DEFAULT_INNERTUBE_KEY.to_string(),
            }
        }
    };
}

async fn make_request(
    client: &reqwest_middleware::ClientWithMiddleware,
    key: impl Into<String>,
    url: impl Into<String>,
    search_options: &SearchOptions,
    request_options: &RequestFuncOptions,
) -> serde_json::Value {
    let key: String = key.into();
    let url: String = url.into();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(
        reqwest::header::HOST,
        reqwest::header::HeaderValue::from_str("www.youtube.com").unwrap(),
    );
    headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_str("https://www.youtube.com").unwrap(),
    );

    // if search_options.safe_search is true assign safe search cookie to reqwest request
    if search_options.safe_search {
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(SAFE_SEARCH_COOKIE)
                .expect("SAFE_SEARCH_COOKIE contain not ASCII"),
        );
    }

    let original_url = &request_options.original_url;
    let query = &request_options.query;
    let filter = if request_options.filter.is_some() {
        format!(
            r#""params": "{}","#,
            request_options.filter.as_ref().unwrap()
        )
    } else {
        "".to_string()
    };

    let format_str = format!(
        r#"{{
            "query": "{query}",
            {filter}
            "context": {{
                "client": {{
                    "utcOffsetMinutes": 0,
                    "gl": "US",
                    "hl": "en",
                    "clientName": "WEB",
                    "clientVersion": "1.20220406.00.00",
                    "originalUrl": "{original_url}"
                }}
            }}
        }}
        "#
    );

    let body: serde_json::Value = serde_json::from_str(&format_str).unwrap();

    let res = client
        .post(format!("https://youtube.com/youtubei/v1${url}?key=${key}"))
        .headers(headers)
        .json(&body)
        .send()
        .await;

    if res.is_err() {
        return serde_json::Value::Null;
    }

    let res = res.unwrap().json::<serde_json::Value>().await;

    if res.is_err() {
        return serde_json::Value::Null;
    }

    res.unwrap()
}

fn parse_search_result(
    client: &reqwest_middleware::ClientWithMiddleware,
    html: impl Into<String>,
    options: &SearchOptions,
) -> Vec<SearchResult> {
    let mut html: String = html.into();

    html = {
        let document = Html::parse_document(&html);
        let scripts_selector = Selector::parse("script").unwrap();
        let mut initial_response_string = document
            .select(&scripts_selector)
            .filter(|x| x.inner_html().contains("var ytInitialData ="))
            .map(|x| x.inner_html().replace("var ytInitialData =", ""))
            .next()
            .unwrap_or(String::from(""))
            .trim()
            .to_string();

        initial_response_string.pop();

        initial_response_string
    };

    // check if html is not empty
    if !html.is_empty() {
        let serde_value = serde_json::from_str::<serde_json::Value>(&html).unwrap();
        let contents = &serde_value["contents"]["twoColumnSearchResultsRenderer"]
            ["primaryContents"]["sectionListRenderer"]["contents"][0]["itemSectionRenderer"]
            ["contents"];

        // if contents found try to format values
        if !contents.is_null() {
            return format_search_result(client, contents, options);
        }
    }

    // if cannot fetch initial data return empty array
    vec![]
}

fn format_search_result(
    client: &reqwest_middleware::ClientWithMiddleware,
    value: &serde_json::Value,
    options: &SearchOptions,
) -> Vec<SearchResult> {
    let mut res: Vec<SearchResult> = vec![];
    let only_numbers_regex = Regex::new(r"[^0-9]").unwrap();
    // Not array we dont care
    if value.is_array() {
        let details = value.as_array().unwrap();

        for data in details {
            // if limit reached break to loop
            if options.limit > 0 && res.len() >= (options.limit) as usize {
                break;
            }

            let match_statemant = if options.search_type == SearchType::All {
                if data
                    .as_object()
                    .map(|x| x.contains_key("videoRenderer"))
                    .unwrap_or(false)
                {
                    &SearchType::Video
                } else if data
                    .as_object()
                    .map(|x| x.contains_key("channelRenderer"))
                    .unwrap_or(false)
                {
                    &SearchType::Channel
                } else if data
                    .as_object()
                    .map(|x| x.contains_key("playlistRenderer"))
                    .unwrap_or(false)
                {
                    &SearchType::Playlist
                } else {
                    &SearchType::All
                }
            } else {
                &options.search_type
            };

            match match_statemant {
                SearchType::Video | SearchType::Film => {
                    // cannot resolve continue
                    if data.is_null() || data["videoRenderer"].is_null() {
                        continue;
                    }

                    let badge = if !data["videoRenderer"]["ownerBadges"].is_array() {
                        &data["videoRenderer"]["ownerBadges"]
                    } else {
                        &data["videoRenderer"]["ownerBadges"][0]
                    };

                    let video = Video {
                        id: data["videoRenderer"]["videoId"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        url: format!(
                            "https://www.youtube.com/watch?v={}",
                            data["videoRenderer"]["videoId"].as_str().unwrap_or("")
                        ),
                        title: data["videoRenderer"]["title"]["runs"][0]["text"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        description: if !data["videoRenderer"]["descriptionSnippet"]["runs"]
                            .is_null()
                        {
                            data["videoRenderer"]["descriptionSnippet"]["runs"]
                                .as_array()
                                .map(|x| {
                                    x.iter()
                                        .map(|c| c["text"].as_str().unwrap_or(""))
                                        .collect::<Vec<&str>>()
                                        .join("")
                                })
                                .unwrap_or("".to_string())
                        } else if !data["videoRenderer"]["detailedMetadataSnippets"][0]
                            ["snippetText"]["runs"]
                            .is_null()
                        {
                            data["videoRenderer"]["detailedMetadataSnippets"][0]["snippetText"]
                                ["runs"]
                                .as_array()
                                .map(|x| {
                                    x.iter()
                                        .map(|c| c["text"].as_str().unwrap_or(""))
                                        .collect::<Vec<&str>>()
                                        .join("")
                                })
                                .unwrap_or("".to_string())
                        } else {
                            String::from("")
                        },
                        duration: if !data["videoRenderer"]["lengthText"].is_null() {
                            time_to_ms(
                                data["videoRenderer"]["lengthText"]["simpleText"]
                                    .as_str()
                                    .unwrap_or("0:00"),
                            ) as u64
                        } else {
                            0u64
                        },
                        duration_raw: if !data["videoRenderer"]["lengthText"]["simpleText"]
                            .is_null()
                        {
                            data["videoRenderer"]["lengthText"]["simpleText"]
                                .as_str()
                                .unwrap_or("0:00")
                                .to_string()
                        } else {
                            String::from("0:00")
                        },
                        thumbnails: if data["videoRenderer"]["thumbnail"]["thumbnails"].is_array() {
                            data["videoRenderer"]["thumbnail"]["thumbnails"]
                                .as_array()
                                .unwrap()
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
                                        .unwrap_or(0i64)
                                        as u64,
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
                                        .unwrap_or(0i64)
                                        as u64,
                                    url: x
                                        .get("url")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect::<Vec<Thumbnail>>()
                        } else {
                            vec![]
                        },
                        channel: Channel {
                            id: data["videoRenderer"]["ownerText"]["runs"][0]["navigationEndpoint"]
                                ["browseEndpoint"]["browseId"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            name: data["videoRenderer"]["ownerText"]["runs"][0]["text"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            url: if !data["videoRenderer"]["ownerText"]["runs"][0]
                                ["navigationEndpoint"]["browseEndpoint"]["canonicalBaseUrl"]
                                .is_null()
                            {
                                format!(
                                    "https://www.youtube.com{}",
                                    data["videoRenderer"]["ownerText"]["runs"][0]
                                        ["navigationEndpoint"]["browseEndpoint"]
                                        ["canonicalBaseUrl"]
                                        .as_str()
                                        .unwrap_or("")
                                )
                            } else if !data["videoRenderer"]["ownerText"]["runs"][0]
                                ["navigationEndpoint"]["commandMetadata"]["webCommandMetadata"]
                                ["url"]
                                .is_null()
                            {
                                format!(
                                    "https://www.youtube.com{}",
                                    data["videoRenderer"]["ownerText"]["runs"][0]
                                        ["navigationEndpoint"]["commandMetadata"]
                                        ["webCommandMetadata"]["url"]
                                        .as_str()
                                        .unwrap_or("")
                                )
                            } else {
                                String::from("")
                            },
                            icon: if data["videoRenderer"]["channelThumbnail"]["thumbnails"]
                                .is_array()
                            {
                                data["videoRenderer"]["channelThumbnail"]["thumbnails"]
                                    .as_array()
                                    .unwrap()
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
                                            .unwrap_or(0i64)
                                            as u64,
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
                                            .unwrap_or(0i64)
                                            as u64,
                                        url: x
                                            .get("url")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    })
                                    .collect::<Vec<Thumbnail>>()
                            } else if data["videoRenderer"]["channelThumbnailSupportedRenderers"]
                                ["channelThumbnailWithLinkRenderer"]["thumbnail"]["thumbnails"]
                                .is_array()
                            {
                                data["videoRenderer"]["channelThumbnailSupportedRenderers"]
                                    ["channelThumbnailWithLinkRenderer"]["thumbnail"]["thumbnails"]
                                    .as_array()
                                    .unwrap()
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
                                            .unwrap_or(0i64)
                                            as u64,
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
                                            .unwrap_or(0i64)
                                            as u64,
                                        url: x
                                            .get("url")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    })
                                    .collect::<Vec<Thumbnail>>()
                            } else {
                                vec![]
                            },
                            verified: if badge["metadataBadgeRenderer"]["style"].is_string() {
                                badge["metadataBadgeRenderer"]["style"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains("verified")
                            } else {
                                false
                            },
                            subscribers: 0,
                        },
                        uploaded_at: if data["videoRenderer"]["publishedTimeText"]["simpleText"]
                            .is_string()
                        {
                            Some(
                                data["videoRenderer"]["publishedTimeText"]["simpleText"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                            )
                        } else {
                            None
                        },
                        views: if data["videoRenderer"]["viewCountText"]["simpleText"].is_string() {
                            let view_count = only_numbers_regex.replace_all(
                                data["videoRenderer"]["viewCountText"]["simpleText"]
                                    .as_str()
                                    .unwrap_or("0"),
                                "",
                            );

                            view_count.parse::<u64>().unwrap_or(0)
                        } else {
                            0u64
                        },
                    };

                    res.push(SearchResult::Video(video));
                }
                SearchType::Channel => {
                    // cannot resolve continue
                    if data.is_null() || data["channelRenderer"].is_null() {
                        continue;
                    }

                    let badges = &data["channelRenderer"]["ownerBadges"];

                    let channel = Channel {
                        id: data["channelRenderer"]["channelId"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        name: data["channelRenderer"]["title"]["simpleText"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        url: if !data["channelRenderer"]["navigationEndpoint"]["browseEndpoint"]
                            ["canonicalBaseUrl"]
                            .is_null()
                        {
                            format!(
                                "https://www.youtube.com{}",
                                data["channelRenderer"]["navigationEndpoint"]["browseEndpoint"]
                                    ["canonicalBaseUrl"]
                                    .as_str()
                                    .unwrap_or("")
                            )
                        } else if !data["channelRenderer"]["navigationEndpoint"]["commandMetadata"]
                            ["webCommandMetadata"]["url"]
                            .is_null()
                        {
                            format!(
                                "https://www.youtube.com{}",
                                data["channelRenderer"]["navigationEndpoint"]["commandMetadata"]
                                    ["webCommandMetadata"]["url"]
                                    .as_str()
                                    .unwrap_or("")
                            )
                        } else {
                            String::from("")
                        },
                        icon: if data["channelRenderer"]["thumbnail"]["thumbnails"].is_array() {
                            data["channelRenderer"]["thumbnail"]["thumbnails"]
                                .as_array()
                                .unwrap()
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
                                        .unwrap_or(0i64)
                                        as u64,
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
                                        .unwrap_or(0i64)
                                        as u64,
                                    url: x
                                        .get("url")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect::<Vec<Thumbnail>>()
                        } else {
                            vec![]
                        },
                        verified: if badges.is_array() {
                            badges.as_array().unwrap().iter().any(|badge| {
                                !badge["verifiedBadge"].is_null()
                                    || badge["metadataBadgeRenderer"]["style"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_lowercase()
                                        .contains("verified")
                            })
                        } else {
                            false
                        },
                        subscribers: if !data["channelRenderer"]["subscriberCountText"]
                            ["simpleText"]
                            .is_null()
                        {
                            let sub_count = only_numbers_regex.replace_all(
                                data["channelRenderer"]["subscriberCountText"]["simpleText"]
                                    .as_str()
                                    .unwrap_or("0"),
                                "",
                            );

                            sub_count.parse::<u64>().unwrap_or(0)
                        } else {
                            0
                        },
                    };

                    res.push(SearchResult::Channel(channel));
                }
                SearchType::Playlist => {
                    // cannot resolve continue
                    if data.is_null() || data["playlistRenderer"].is_null() {
                        continue;
                    }

                    let playlist = Playlist {
                        id: data["playlistRenderer"]["playlistId"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        name: data["playlistRenderer"]["title"]["simpleText"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        url: if data["playlistRenderer"]["playlistId"].is_string() {
                            format!(
                                "https://www.youtube.com/playlist?list={}",
                                data["playlistRenderer"]["playlistId"]
                                    .as_str()
                                    .unwrap_or("")
                            )
                        } else {
                            String::from("")
                        },
                        channel: Channel {
                            id: data["playlistRenderer"]["shortBylineText"]["runs"][0]
                                ["navigationEndpoint"]["browseEndpoint"]["browseId"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            name: data["playlistRenderer"]["shortBylineText"]["runs"][0]["text"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            url: if data["playlistRenderer"]["shortBylineText"]["runs"][0]
                                ["navigationEndpoint"]["browseEndpoint"]["canonicalBaseUrl"]
                                .is_string()
                            {
                                format!(
                                    "https://www.youtube.com{}",
                                    data["playlistRenderer"]["shortBylineText"]["runs"][0]
                                        ["navigationEndpoint"]["browseEndpoint"]
                                        ["canonicalBaseUrl"]
                                        .as_str()
                                        .unwrap_or("")
                                )
                            } else if data["playlistRenderer"]["shortBylineText"]["runs"][0]
                                ["navigationEndpoint"]["commandMetadata"]["webCommandMetadata"]
                                ["url"]
                                .is_string()
                            {
                                format!(
                                    "https://www.youtube.com{}",
                                    data["playlistRenderer"]["shortBylineText"]["runs"][0]
                                        ["navigationEndpoint"]["commandMetadata"]
                                        ["webCommandMetadata"]["url"]
                                        .as_str()
                                        .unwrap_or("")
                                )
                            } else {
                                String::from("")
                            },
                            icon: vec![],
                            verified: if data["playlistRenderer"]["ownerBadges"].is_array() {
                                data["playlistRenderer"]["ownerBadges"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|badge| {
                                        !badge["verifiedBadge"].is_null()
                                            || badge["metadataBadgeRenderer"]["style"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_lowercase()
                                                .contains("verified")
                                    })
                            } else {
                                false
                            },
                            subscribers: 0,
                        },
                        thumbnails: if data["playlistRenderer"]["thumbnails"][0]["thumbnails"]
                            .is_array()
                        {
                            data["playlistRenderer"]["thumbnails"][0]["thumbnails"]
                                .as_array()
                                .unwrap()
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
                                        .unwrap_or(0i64)
                                        as u64,
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
                                        .unwrap_or(0i64)
                                        as u64,
                                    url: x
                                        .get("url")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect::<Vec<Thumbnail>>()
                        } else if data["playlistRenderer"]["thumbnailRenderer"]
                            ["playlistVideoThumbnailRenderer"]["thumbnail"]["thumbnails"]
                            .is_array()
                        {
                            data["playlistRenderer"]["thumbnailRenderer"]
                                ["playlistVideoThumbnailRenderer"]["thumbnail"]["thumbnails"]
                                .as_array()
                                .unwrap()
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
                                        .unwrap_or(0i64)
                                        as u64,
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
                                        .unwrap_or(0i64)
                                        as u64,
                                    url: x
                                        .get("url")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect::<Vec<Thumbnail>>()
                        } else {
                            vec![]
                        },
                        // we cannot get videos, views and last_update from search we need to send request to playlist url
                        views: 0,
                        videos: vec![],
                        last_update: None,
                        // continuation not available in search
                        continuation: None,
                        client: client.clone(),
                    };

                    res.push(SearchResult::Playlist(playlist));
                }
                // Not proper type! skip it
                _ => continue,
            }
        }
    }

    // return results array
    res
}
