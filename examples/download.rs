use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video = Video::new(video_url).unwrap();

    println!("{:#?}", video.download().await.unwrap());
}
