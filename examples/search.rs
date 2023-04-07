use rusty_ytdl::search::YouTube;

#[tokio::main]
async fn main() {
    let youtube = YouTube::new().unwrap();

    let res = youtube.search("i know your ways", None).await;

    println!("{res:#?}");
}
