use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video = Video::new(url).unwrap();

    let path = std::path::Path::new(r"test.mp4");

    video.download(path).await.unwrap();
}
