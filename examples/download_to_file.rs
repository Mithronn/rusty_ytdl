use std::io::Write;

use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let video = Video::new(url).unwrap();

    let video_download_buffer = video.download().await;

    if video_download_buffer.is_ok() {
        let path = std::path::Path::new(r"test.mp3");
        let mut file = std::fs::File::create(path).unwrap();
        let info = file.write_all(&video_download_buffer.unwrap());
        println!("{:?}", info);
    }
}
