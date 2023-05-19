use rusty_ytdl;

#[tokio::test]
async fn download_to_file() {
    use rusty_ytdl::{Video, VideoOptions, VideoQuality, VideoSearchOptions};

    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video_options = VideoOptions {
        quality: VideoQuality::Highest,
        filter: VideoSearchOptions::VideoAudio,
        ..Default::default()
    };

    let video = Video::new_with_options(url, video_options).unwrap();

    let path = std::path::Path::new(r"test.mp4");

    video.download(path).await.unwrap();
}
