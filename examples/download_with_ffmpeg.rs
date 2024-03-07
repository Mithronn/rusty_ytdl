#[tokio::main]
async fn main() {
    #[cfg(feature = "ffmpeg")]
    {
        use rusty_ytdl::{FFmpegArgs, Video, VideoOptions, VideoQuality, VideoSearchOptions};

        let url = "FZ8BxMU3BYc";

        let video_options = VideoOptions {
            quality: VideoQuality::Highest,
            filter: VideoSearchOptions::VideoAudio,
            ..Default::default()
        };

        let video = Video::new_with_options(url, video_options).unwrap();

        let stream = video
            .stream_with_ffmpeg(Some(FFmpegArgs {
                format: Some("mp3".to_string()),
                audio_filter: Some("aresample=48000,asetrate=48000*0.8".to_string()),
                video_filter: Some("eq=brightness=150:saturation=2".to_string()),
            }))
            .await
            .unwrap();

        while let Some(chunk) = stream.chunk().await.unwrap() {
            println!("{:#?}", chunk);
        }
    }
}
