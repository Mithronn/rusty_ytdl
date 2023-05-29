fn main() {
    #[cfg(feature = "blocking")]
    {
        use rusty_ytdl::blocking::*;
        let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

        let _video = Video::new(video_url).unwrap();
    }
}
