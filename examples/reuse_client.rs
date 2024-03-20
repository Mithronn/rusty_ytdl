use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let urls = vec![
        "https://www.youtube.com/watch?v=FZ8BxMU3BYc",
        "https://www.youtube.com/watch?v=QpgevVPHI-4",
    ];

    let client = reqwest::Client::builder().build().unwrap();

    let options = VideoOptions {
        request_options: RequestOptions {
            client: Some(client),
            ..Default::default()
        },
        ..Default::default()
    };

    for url in urls {
        let video = Video::new_with_options(url, options.clone()).unwrap();

        let info = video.get_info().await.unwrap();

        println!("Video title: {}", info.video_details.title);
    }
}
