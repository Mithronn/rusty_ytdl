// #[ignore]
#[tokio::test]
async fn ffmpeg_test() {
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

        video
            .download(
                r"./filter_applied_audio.mp3",
                Some(FFmpegArgs {
                    format: Some("mp3".to_string()),
                    // audio_filter: Some(
                    //     "aresample=48000,asetrate=48000*0.8,bass=g=30:f=110:w=0.3".to_string(),
                    // ),
                    audio_filter: None,
                    video_filter: Some("eq=brightness=150:saturation=2".to_string()),
                }),
            )
            .await
            .unwrap();
    }
}
