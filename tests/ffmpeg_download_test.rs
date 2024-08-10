#[ignore]
#[tokio::test]
async fn ffmpeg_download_test() {
    #[cfg(feature = "ffmpeg")]
    {
        use rusty_ytdl::{FFmpegArgs, Video, VideoOptions, VideoQuality};

        let url = "FZ8BxMU3BYc";

        let video_options = VideoOptions {
            quality: VideoQuality::Highest,
            ..Default::default()
        };

        let video = Video::new_with_options(url, video_options).unwrap();

        video
            .download_with_ffmpeg(
                r"./filter_applied_audio.mp3",
                Some(FFmpegArgs {
                    format: Some("mpegts".to_string()),
                    audio_filter: Some("aresample=48000,asetrate=48000*0.8".to_string()),
                    video_filter: Some("eq=brightness=150:saturation=2".to_string()),
                }),
            )
            .await
            .unwrap();
    }
}
