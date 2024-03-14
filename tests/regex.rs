#[tokio::test]
async fn is_valid_id_or_link() {
    use rusty_ytdl::get_video_id;

    assert_eq!(Some("FZ8BxMU3BYc".to_string()), get_video_id("FZ8BxMU3BYc"));
    assert_eq!(
        Some("FZ8BxMU3BYc".to_string()),
        get_video_id("https://www.youtube.com/watch?v=FZ8BxMU3BYc")
    );
    assert_eq!(
        Some("FZ8BxMU3BYc".to_string()),
        get_video_id("https://music.youtube.com/watch?v=FZ8BxMU3BYc&feature=share")
    );
    assert_eq!(
        Some("FZ8BxMU3BYc".to_string()),
        get_video_id("https://youtu.be/FZ8BxMU3BYc")
    );
    assert_eq!(
        Some("FZ8BxMU3BYc".to_string()),
        get_video_id("https://www.youtube.com/shorts/FZ8BxMU3BYc")
    );
    assert_eq!(
        Some("FZ8BxMU3BYc".to_string()),
        get_video_id("https://www.youtube.com/embed/FZ8BxMU3BYc")
    );

    // Not valid video id
    assert_eq!(None, get_video_id("FZ8BxU3BYc"));
}
