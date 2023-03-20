#[tokio::test]
async fn get_live_video_info() {
    use rusty_ytdl::Video;

    let url = "https://www.youtube.com/watch?v=0ThMultL4PY";

    let video = Video::new(url).unwrap();

    let video_info = video.get_info().await.unwrap();

    println!("Formats: {:#?}", video_info.formats);
}
