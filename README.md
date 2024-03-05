# <div align="center"> rusty_ytdl </div>

<div align="center">

[![crates.io](https://img.shields.io/crates/v/rusty_ytdl.svg?style=for-the-badge&logo=rust)](https://crates.io/crates/rusty_ytdl)
[![Released API docs](https://img.shields.io/badge/docs.rs-rusty__ytdl-C36241?style=for-the-badge&logo=docs.rs)](https://docs.rs/rusty_ytdl)

</div>

Youtube searching and downloading module written with **pure Rust**.
Download videos **blazing-fast** without getting stuck on Youtube download speed (Downloads 20MB video files in just 10 seconds!)

## Overview

- [Roadmap](#roadmap)
- [Features](#features)
- [Usage](#usage)
- [Limitations](#limitations)

## Roadmap

- [ ] ffmpeg feature
- [ ] benchmarks

## Features

- Download live and non-live videos
- Search with query (Video, Playlist, Channel)
- Blocking and asynchronous API
- Proxy, IPv6, and cookie support on request
- [CLI](https://crates.io/crates/rusty_ytdl-cli)

# Usage

```rust,ignore
use rusty_ytdl::Video;

#[tokio::main]
async fn main() {
  let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; // FZ8BxMU3BYc works too!
  let video = Video::new(url).unwrap();

  let stream = video.stream().await.unwrap();

  while let Some(chunk) = stream.chunk().await.unwrap() {
    // Do what you want with chunks
    println!("{:#?}", chunk);
  }

  // Or direct download to path
  let path = std::path::Path::new(r"test.mp3");

  video.download(path).await.unwrap();

  //
  // Or with options
  //

  let video_options = VideoOptions {
    quality: VideoQuality::Lowest,
    filter: VideoSearchOptions::Audio,
    ..Default::default()
  };

  let video = Video::new_with_options(url, video_options).unwrap();

  let stream = video.stream().await.unwrap();

  while let Some(chunk) = stream.chunk().await.unwrap() {
    // Do what you want with chunks
    println!("{:#?}", chunk);
  }

  // Or direct download to path
  let path = std::path::Path::new(r"test.mp3");

  video.download(path).await.unwrap();
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
rusty_ytdl = "0.6.7"
```
