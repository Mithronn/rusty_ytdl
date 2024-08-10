#[test]
fn download_with_blocking_chunks() {
    #[cfg(feature = "blocking")]
    {
        use rusty_ytdl::{blocking::Video, VideoOptions, VideoQuality};

        let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

        let video_options = VideoOptions {
            quality: VideoQuality::Highest,
            ..Default::default()
        };

        let video = Video::new_with_options(url, video_options).unwrap();

        let stream = video.stream().unwrap();

        while let Some(chunk) = stream.chunk().unwrap() {
            println!("{} byte downloaded", chunk.len());
        }
    }
}
