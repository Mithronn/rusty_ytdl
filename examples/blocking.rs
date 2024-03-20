use rusty_ytdl::blocking::*;

fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    let _video = Video::new(video_url).unwrap();
}
