use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video = Video::new(video_url).unwrap();

    let stream = video.stream().await.unwrap();

    while let Ok(chunk) = stream.chunk().await {
        println!("{:#?}", chunk);
    }
}
