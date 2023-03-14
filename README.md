# <div align="center">rusty_ytdl</div>

<div align="center">

[![crates.io](https://img.shields.io/crates/v/rusty_ytdl.svg?style=for-the-badge)](https://crates.io/crates/rusty_ytdl)
[![Released API docs](https://img.shields.io/badge/docs.rs-rusty__ytdl-C36241?style=for-the-badge)](https://docs.rs/rusty_ytdl)

</div>

Youtube downloading module. Written with pure Rust.

## Overview

- [Roadmap](#roadmap)
- [Usage](#usage)
- [Limitations](#limitations)

## Roadmap

- [x] download normal videos
- [ ] download live videos
- [x] asynchronous API
- [ ] blocking API
- [ ] full video info deserialization
- [ ] CLI
- [ ] testing suite
- [ ] benchmarks

# Usage

Download videos incredibly fast without getting stuck on youtube download speed (Downloads 20mb files in just 10 seconds\*)

```rust,ignore
use rusty_ytdl::info::download;
use rusty_ytdl::structs::DownloadOptions;

#[tokio::main]
async fn main() {
  let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; // FZ8BxMU3BYc works too!
  let video_buffer: Vec<u8> = download(video_url,None, DownloadOptions::default()).await.unwrap();

  // Do what you want whit video buffer vector
  println!("{:#?}",video_buffer);
}
```

or get only video informations

```rust,ignore
use rusty_ytdl::info::get_info;
use rusty_ytdl::utils::choose_format;
use rusty_ytdl::structs::VideoOptions;

#[tokio::main]
async fn main() {
  let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc"; // FZ8BxMU3BYc works too!
  let video_info = get_info(video_url,None).await;
  // Also works with live videos!!

  println!("{:#?}",video_info);
  /*
  VideoInfo {
    ...
    video_details: VideoDetails,
    formats: Vec<serde_json::Value>,
    related_videos: Vec<serde_json::Value>
  }
  */

  let video_options = VideoOptions::default();
  let format = choose_format(&video_info.unwrap().formats,&video_options);

  // Get a format by VideoOptions filter parameter
  println!("{:#?}",format);
}
```

## Limitations

rusty-ytdl cannot download videos that fall into the following

- Regionally restricted (requires a [proxy](example/proxy.js))
- Private (if you have access, requires [cookies](example/cookies.js))
- Rentals (if you have access, requires [cookies](example/cookies.js))
- YouTube Premium content (if you have access, requires [cookies](example/cookies.js))
- Only [HLS Livestreams](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) are currently supported. Other formats not will be fetch

Generated download links are valid for 6 hours, and may only be downloadable from the same IP address.

# Installation

```bash
cargo add rusty_ytdl
```

Or add the following to your `Cargo.toml` file:

```toml
[dependencies]
rusty_ytdl = "0.1.0"
```
