#[tokio::test]
async fn search_playlist() {
    use rusty_ytdl::search::{Playlist, PlaylistSearchOptions};

    let playlist = Playlist::get(
        "https://www.youtube.com/playlist?list=PLwMEL7UNT4o9iMzrvNBXZqXbNPFfT6rVD",
        Some(&PlaylistSearchOptions {
            limit: 6000,
            fetch_all: false,
            ..Default::default()
        }),
    )
    .await;

    println!(
        "{:#?}",
        playlist.and_then(|x| Ok(x.videos.len())).unwrap_or(0)
    );
}
