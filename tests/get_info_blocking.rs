#[test]
fn get_info_blocking() {
    #[cfg(feature = "blocking")]
    {
        use rusty_ytdl::blocking::Video;

        let url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; //"https://www.youtube.com/watch?v=0ThMultL4PY";

        let video = Video::new(url).unwrap();

        let video_info = video.get_info().unwrap();

        println!("Formats: {:#?}", video_info.formats);
    }
}
