pub mod args;
pub mod commands;
pub mod utils;

use std::{
    fs::File,
    path::{Path, PathBuf},
    pin::Pin,
};

use anyhow::{Error, Result};
use clap::Parser;
use colored::Colorize;
use rusty_ytdl::{Video, VideoOptions, VideoSearchOptions};
use tokio::io::{self, AsyncWriteExt};

use args::video_options::Quality;
use commands::{download::DownloadArgs, Commands};
use utils::result_serializer::ResultSerializer;

#[tokio::main]
async fn main() -> Result<()> {
    let commands = Commands::parse();

    let res = match commands {
        Commands::Download(args) => download(args).await,
    };

    if let Err(ref err) = res {
        log::error!("{}\n", err);
        eprintln!(
            "\
            If the error is caused by a change to the YouTube API, it would be great if you could \
            report this. Common indicators of an API change are:\n\n\
            1. Repeated HTTP 403 status\n\
            2. Unexpected response errors\n\
            3. Deserialization errors\n\
            "
        );
    }

    res
}

async fn download(args: DownloadArgs) -> Result<()> {
    args.log.init_logger();

    let video_identifier = args.id;
    let download_path = args.path.unwrap_or(PathBuf::new().join("."));

    if !download_path.exists() {
        return Err(Error::msg("Folder path not found!"));
    } else if !download_path.is_dir() {
        return Err(Error::msg("Output path must be a directory!"));
    }

    let filter = if args.video_options.only_audio {
        VideoSearchOptions::Audio
    } else if args.video_options.only_video {
        VideoSearchOptions::Video
    } else {
        VideoSearchOptions::VideoAudio
    };

    let quality = args
        .video_options
        .quality
        .unwrap_or(Quality::Highest)
        .into();

    let download_options = VideoOptions {
        quality,
        filter,
        ..Default::default()
    };

    let video = Video::new_with_options(&video_identifier, download_options.clone());

    if let Err(err) = video {
        return Err(Error::msg(err.to_string()));
    }

    let video = video.unwrap();
    let video_info = video.get_info().await;

    if let Err(err) = video_info {
        return Err(Error::msg(err.to_string()));
    }

    let video_info = video_info.unwrap();
    let stream = video.stream().await;

    if let Err(err) = stream {
        return Err(Error::msg(err.to_string()));
    }

    let stream = stream.unwrap();

    let download_file = args
        .filename
        .unwrap_or(format!("{}.mp3", video_info.video_details.video_id).into());
    let video_size = stream.content_length();

    let pb = args.log.init_progress_bar(video_size as u64);

    pb.set_message(format!(
        "{} {}",
        video_info.video_details.title.cyan(),
        "is downloading...".white().bold(),
    ));

    let mut downloaded: u64 = 0_u64;

    let pb_clone = pb.clone();

    let path_package_clone = download_path.clone();
    let download_file_clone = download_file.clone();

    let future = async move {
        if matches!(
            args.output.output_format,
            args::output::OutputFormat::Stdout
        ) {
            let mut stdout = io::stdout();
            let mut stdout = Pin::new(&mut stdout);

            while let Some(bytes) = stream.chunk().await.unwrap() {
                if let Err(err) = stdout.write_all(&bytes).await {
                    pb_clone.finish_and_clear();
                    return Err(Error::msg(err.to_string()));
                }

                let new = std::cmp::min(downloaded + bytes.len() as u64, video_size as u64);
                downloaded = new;
                pb_clone.set_position(new);
            }
        } else {
            let path_values = Path::new(&path_package_clone);

            let file = File::create(path_values.join(Path::new(&download_file_clone)));

            if let Err(err) = file.as_ref() {
                pb_clone.finish_and_clear();
                return Err(Error::msg(err.to_string()));
            }

            let mut file = file.unwrap();

            while let Some(bytes) = stream.chunk().await.unwrap() {
                use std::io::Write;

                if let Err(err) = file.write_all(&bytes) {
                    pb_clone.finish_and_clear();
                    return Err(Error::msg(err.to_string()));
                }

                let new = std::cmp::min(downloaded + bytes.len() as u64, video_size as u64);
                downloaded = new;
                pb_clone.set_position(new);
            }
        }

        Ok(())
    };

    let download_thread = tokio::spawn(future);

    let pb_clone_clone = pb.clone();
    let progress_tick = tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            pb_clone_clone.tick();
        }
    });

    let finished = download_thread.is_finished();
    let _ = download_thread.await?;

    if finished {
        progress_tick.abort()
    }

    pb.finish_and_clear();

    // Display successfuly download message than exit with success
    println!(
        "\n{} {}\n",
        "Video successfully downloaded to".white().bold(),
        String::from(
            download_path
                .join(Path::new(&download_file))
                .to_string_lossy()
        )
        .as_str()
        .green()
        .underline()
    );

    // output format
    let output = args
        .output
        .output_format
        .serialize(&ResultSerializer::new(video_info, args.output.output_level))
        .unwrap();
    println!("{output}");

    Ok(())
}
