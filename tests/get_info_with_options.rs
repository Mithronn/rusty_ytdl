use rusty_ytdl;

#[tokio::test]
async fn get_info_with_options() {
    use rusty_ytdl::{choose_format, Video, VideoOptions, VideoQuality, VideoSearchOptions};

    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; //"https://www.youtube.com/watch?v=0ThMultL4PY";

    let video_options = VideoOptions {
        quality: VideoQuality::Lowest,
        filter: VideoSearchOptions::Audio,
        ..Default::default()
    };

    let video = Video::new_with_options(url, video_options.clone()).unwrap();

    let video_info = video.get_info().await.unwrap();

    let format = choose_format(&video_info.formats, &video_options);

    println!("Formats: {:#?}", video_info.formats);
    println!("Format: {:#?}", format);
}
