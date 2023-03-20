use rusty_ytdl::*;

#[tokio::main]
async fn main() {
    let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";

    // Check the link:
    // https://www.iana.org/assignments/ipv6-unicast-address-assignments/ipv6-unicast-address-assignments.xhtml
    let ipv6_block = "2001:4::/48".to_string();

    let video_options = VideoOptions {
        request_options: RequestOptions {
            ipv6_block: Some(ipv6_block),
            ..Default::default()
        },
        ..Default::default()
    };

    let _video = Video::new_with_options(video_url, video_options).unwrap();
}
