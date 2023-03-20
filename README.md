# <div align="center">rusty_ytdl</div>

<div align="center">

[![crates.io](https://img.shields.io/crates/v/rusty_ytdl.svg?style=for-the-badge&logo=rust)](https://crates.io/crates/rusty_ytdl)
[![Released API docs](https://img.shields.io/badge/docs.rs-rusty__ytdl-C36241?style=for-the-badge&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA4AAAAOCAYAAAAfSC3RAAAAAXNSR0IArs4c6QAAAHhJREFUOE+tkuEKgDAIhNvw/R94icUFDnMeUbSf6nfn1LaRZ2YHUr33VpUsQQdycRa4gRHywioGUQpGR4hAILo+gtH5P5ANhE0czlfveexZKLbouQkydRYvQZ8i2yVcF7DanwvE3HdwjLGLiLz5o6rqPAB2WhCscidaPm/VahMnogAAAABJRU5ErkJggg==)](https://docs.rs/rusty_ytdl)

</div>

Youtube downloading module written with **pure Rust**.
Download videos **blazing-fast** without getting stuck on Youtube download speed (Downloads 20MB video files in just 10 seconds!)

## Overview

- [Roadmap](#roadmap)
- [Usage](#usage)
- [Limitations](#limitations)

## Roadmap

- [x] download normal videos
- [ ] download live videos
- [x] asynchronous API
- [x] blocking API
- [x] proxy options
- [x] cookie options
- [x] full video info deserialization
- [ ] CLI
- [ ] benchmarks

# Usage

```rust,ignore
use rusty_ytdl::Video;

#[tokio::main]
async fn main() {
  let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; // FZ8BxMU3BYc works too!
  let video = Video::new(url).unwrap();

  let video_download_buffer = video.download().await;

  // Do what you want with video buffer vector
  println!("{:#?}",video_buffer);

  // Or with options

  let video_options = VideoOptions {
    quality: VideoQuality::Lowest,
    filter: VideoSearchOptions::Audio,
    ..Default::default()
  };

  let video = Video::new_with_options(url, video_options).unwrap();
  let video_download_buffer = video.download().await;

  // Do what you want with video buffer vector
  println!("{:#?}",video_buffer);
}
```

or get only video informations

```rust,ignore
use rusty_ytdl::Video;
use rusty_ytdl::{choose_format,VideoOptions};

#[tokio::main]
async fn main() {
  let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; // FZ8BxMU3BYc works too!
  // Also works with live videos!!
  let video = Video::new(url).unwrap();

  let video_info = video.get_info().await.unwrap();
  println!("{:#?}",video_info);

  /*
  VideoInfo {
    dash_manifest_url: Option<String>,
    hls_manifest_url: Option<String>,
    video_details: VideoDetails,
    formats: Vec<VideoFormat>,
    related_videos: Vec<RelatedVideo>
  }
  */

  let video_options = VideoOptions {
    quality: VideoQuality::Lowest,
    filter: VideoSearchOptions::Audio,
      ..Default::default()
  };

  let format = choose_format(&video_info.unwrap().formats,&video_options);

  println!("{:#?}",format);

  // Or with options
  let video = Video::new_with_options(url, video_options.clone()).unwrap();

  let format = choose_format(&video_info.formats, &video_options);

  let video_info = video.get_info().await.unwrap();

  println!("{:#?}",video_info);
}
```

For more examples, check [examples](examples/)

## Limitations

rusty_ytdl cannot download videos that fall into the following

- Regionally restricted (requires a [proxy](examples/proxy.rs))
- Private (if you have access, requires [cookies](examples/cookies.rs))
- Rentals (if you have access, requires [cookies](examples/cookies.rs))
- YouTube Premium content (if you have access, requires [cookies](examples/cookies.rs))
- Only [HLS Livestreams](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) are currently supported. Other formats not will be fetch

Generated download links are valid for 6 hours, and may only be downloadable from the same IP address.

### Ratelimits

When doing to many requests YouTube might block. This will result in your requests getting denied with HTTP Status Code 429. The following steps might help you:

- Use proxies (you can find an example [proxy](examples/proxy.rs))
- Extend on the Proxy Idea by rotating (IPv6)Addresses (you can find an example [IPv6](examples/ipv6.rs))
- Use cookies (you can find an example [cookies](examples/cookies.rs))
  - for this to take effect you have to first wait for the current ratelimit to expire!
- Wait it out

# Installation

```bash
cargo add rusty_ytdl
```

Or add the following to your `Cargo.toml` file:

```toml
[dependencies]
rusty_ytdl = "0.4.0"
```
