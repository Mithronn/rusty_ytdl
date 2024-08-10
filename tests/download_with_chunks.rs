#[tokio::test]
async fn download_with_chunks() {
    use rusty_ytdl::{Video, VideoOptions, VideoQuality};

    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video_options = VideoOptions {
        quality: VideoQuality::Highest,
        ..Default::default()
    };

    let video = Video::new_with_options(url, video_options).unwrap();

    let stream = video.stream().await.unwrap();

    while let Some(chunk) = stream.chunk().await.unwrap() {
        println!("{} byte downloaded", chunk.len());
    }
}
