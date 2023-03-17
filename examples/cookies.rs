use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video_options = VideoOptions {
        request_options: RequestOptions {
            cookies: Some("key1=value1; key2=value2; key3=value3".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _video = Video::new_with_options(video_url, video_options).unwrap();
}
