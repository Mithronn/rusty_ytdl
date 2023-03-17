use rusty_ytdl;

#[tokio::test]
async fn formats_str_to_json() {
    use rusty_ytdl::constants::FORMATS;

    let itag_91 = FORMATS.as_object().and_then(|x| {
        x.get("91")
            .and_then(|c| c.as_object().and_then(|x| x.get("mimeType")))
    });

    let itag_13 = FORMATS.as_object().and_then(|x| {
        x.get("13")
            .and_then(|c| c.as_object().and_then(|x| x.get("qualityLabel")))
    });

    // println!("{:#?}", itag_91);

    assert_eq!(
        Some(&serde_json::Value::String(
            "video/ts; codecs=\"H.264, aac\"".to_string()
        )),
        itag_91
    );

    assert!(itag_13.unwrap().is_null());
}
