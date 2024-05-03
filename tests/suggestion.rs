#[tokio::test]
async fn suggestion() {
    use rusty_ytdl::search::YouTube;

    let youtube = YouTube::new().unwrap();

    let res = youtube.suggestion("i know ").await;

    println!("{res:#?}");
}