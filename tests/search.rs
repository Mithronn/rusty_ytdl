#[tokio::test]
async fn search() {
    use rusty_ytdl::search::YouTube;

    let youtube = YouTube::new().unwrap();

    let res = youtube.search("i know your ways", None).await;

    println!("{res:#?}");
}
