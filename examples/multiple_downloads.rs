use std::fs::File;

use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let urls = [
        "https://youtube.com/watch?v=Rbgw_rduQpM",
        "https://youtube.com/watch?v=-h6PCkfTBcc",
        "https://youtube.com/watch?v=2SUwOgmvzK4",
        "https://youtube.com/watch?v=9Ueulv6BugQ",
        "https://youtube.com/watch?v=R4hDcd9fzRk",
        "https://youtube.com/watch?v=W5Sq71VTJ9Q",
    ];

    let instant = std::time::Instant::now();
    for url in urls {
        let video = Video::new(url).unwrap();

        let info = video.get_info().await.unwrap();

        println!("Downloading: {}", info.video_details.title);
    }
    println!("Time taken: {:?}", instant.elapsed());
    flame::dump_html(File::create("flamegraph.html").unwrap()).unwrap();
}
