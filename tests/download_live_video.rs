#[tokio::test]
async fn download_live_video() {
    use rusty_ytdl::Video;

    let url = "https://www.youtube.com/watch?v=9HhhFKxfqUk";

    let video = Video::new(url).unwrap();

    // let video_download_buffer = video.stream().await.unwrap();
    let path = std::path::Path::new(r"test.mp4");

    // video_download_buffer.chunk().await.unwrap();
    video.download(path).await.unwrap();
}
