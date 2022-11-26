use scraper::{Html, Selector};

use crate::VideoInfo;

use crate::info_extras::{get_media, get_related_videos};
use crate::utils::{
    clean_video_details, get_cver, get_functions, get_html5player, get_video_id,
    is_not_yet_broadcasted, is_play_error, is_private_video, is_rental, parse_video_formats,
};

#[derive(Debug)]
pub enum VideoInfoError {
    VideoNotFound,
    VideoSourceNotFound,
    VideoIsPrivate,
    HttpRequestError,
    BodyCannotParsed,
}

pub async fn get_basic_info(link: &str) -> Result<VideoInfo, VideoInfoError> {
    let mut cver = "2.20210622.10.00";

    let id = get_video_id(link);

    if id.is_none() {
        return Err(VideoInfoError::VideoNotFound);
    }

    let client = reqwest::Client::new();

    let url_parsed = url::Url::parse_with_params(link, &[("hl", "en")])
        .unwrap_or_else(|_x| url::Url::parse("https://a.xyz").unwrap());

    let request = client.get(url_parsed.as_str()).send().await;

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

    cver = get_cver(&player_response_clone);

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

    let a = VideoInfo {
        player_response,
        initial_response,
        html5player: get_html5player(response.as_str()).unwrap(),
        formats: parse_video_formats(
            &player_response_clone,
            get_functions(get_html5player(response.as_str()).unwrap().as_str()).await,
        )
        .unwrap(),
        related_videos: get_related_videos(&initial_response_clone).unwrap(),
        video_details,
    };

    return Ok(a);
}

pub async fn get_info(link: &str) -> VideoInfo {
    let info = get_basic_info(link).await.unwrap();

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
    let mut funcs: Vec<String> = vec![];

    if info.formats.len() > 0 {}

    if has_manifest
        && info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("dashManifestUrl"))
            .is_some()
    {
        let url = info
            .player_response
            .get("streamingData")
            .and_then(|x| x.get("dashManifestUrl"))
            .and_then(|x| x.as_str())
            .unwrap_or_else(|| "");
        funcs.push(url.to_string());
        get_m3u8(url).await;
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
        funcs.push(url.to_string());
        get_dash_manifest(url).await;
    }

    return info;
}

pub async fn get_dash_manifest(url: &str) {
    todo!();
}

pub async fn get_m3u8(url: &str) {
    let client = reqwest::Client::new();

    let body = client.get(url).send().await.unwrap().text().await.unwrap();

    let http_regex = regex::Regex::new(r"^https?://").unwrap();
    let itag_regex = regex::Regex::new(r"/itag/(\d+)/").unwrap();

    body.split("\n")
        .filter(|x| http_regex.is_match(x))
        .for_each(|line| {
            let itag = itag_regex
                .captures(line)
                .unwrap()
                .get(1)
                .and_then(|x| Some(x.as_str()))
                .unwrap_or_else(|| "");

            println!("itag: {}, url: {}", itag, line);
        });
}
