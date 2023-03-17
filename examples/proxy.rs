use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video_options = VideoOptions {
        request_options: RequestOptions {
            proxy: Some(
                reqwest::Proxy::http("https://my.prox")
                    .unwrap()
                    .basic_auth("a", "b"),
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    let _video = Video::new_with_options(video_url, video_options).unwrap();
}
