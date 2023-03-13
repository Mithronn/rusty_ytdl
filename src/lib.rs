#![recursion_limit = "256"]
pub mod constants;
pub mod info;
pub mod info_extras;
pub mod structs;
pub mod utils;

#[cfg(test)]
mod tests {
    use crate::utils;

    #[tokio::test]
    async fn get_video_info() {
        use crate::{info::get_info, structs::VideoOptions};
        let start_time = std::time::Instant::now();
        let video_info = get_info("https://www.youtube.com/watch?v=FZ8BxMU3BYc")
            // let video_info = get_info("https://www.youtube.com/watch?v=0ThMultL4PY")
            .await
            .unwrap();
        let video_options = VideoOptions::default();
        let format = utils::choose_format(&video_info.formats, &video_options);
        println!("Formats: {:#?}", video_info.formats);
        println!("Formats: {:#?}", format);
        println!("Time elapsed: {}", start_time.elapsed().as_secs_f64());
    }

    #[tokio::test]
    async fn is_valid_id_or_link() {
        use crate::utils::get_video_id;

        assert_eq!(Some("FZ8BxMU3BYc".to_string()), get_video_id("FZ8BxMU3BYc"));
        assert_eq!(
            Some("FZ8BxMU3BYc".to_string()),
            get_video_id("https://www.youtube.com/watch?v=FZ8BxMU3BYc")
        );
        assert_eq!(
            Some("FZ8BxMU3BYc".to_string()),
            get_video_id("https://music.youtube.com/watch?v=FZ8BxMU3BYc&feature=share")
        );
        assert_eq!(
            Some("FZ8BxMU3BYc".to_string()),
            get_video_id("https://youtu.be/FZ8BxMU3BYc")
        );
        assert_eq!(
            Some("FZ8BxMU3BYc".to_string()),
            get_video_id("https://www.youtube.com/shorts/FZ8BxMU3BYc")
        );
        assert_eq!(
            Some("FZ8BxMU3BYc".to_string()),
            get_video_id("https://www.youtube.com/embed/FZ8BxMU3BYc")
        );

        // Not valid video id
        assert_eq!(None, get_video_id("FZ8BxU3BYc"));
    }

    #[tokio::test]
    async fn formats_str_to_json() {
        use crate::constants::FORMATS;

        let itag_91 = (*FORMATS).as_object().and_then(|x| {
            x.get("91")
                .and_then(|c| c.as_object().and_then(|x| x.get("mimeType")))
        });

        let itag_13 = (*FORMATS).as_object().and_then(|x| {
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
}
