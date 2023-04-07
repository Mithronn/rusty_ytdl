#[tokio::test]
async fn download_live_video() {
    use rusty_ytdl::Video;

    let url = "https://www.youtube.com/watch?v=0ThMultL4PY";

    let video = Video::new(url).unwrap();

    let video_download_buffer = video.download().await;
    println!("RESULT => {video_download_buffer:?}");
}
