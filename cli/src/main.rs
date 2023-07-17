use std::{path::Path, process::exit};

use clap::{Arg, Command};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use rusty_ytdl::{choose_format, Video, VideoOptions, VideoQuality, VideoSearchOptions};

#[tokio::main]
async fn main() {
    let cmd = Command::new("rusty_ytdl")
        .about("A CLI for rusty_ytdl")
        .version("0.6.1")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .author("Mithronn")
        .subcommand(
            Command::new("download")
                .about("Download the video to spesific folder")
                .arg(
                    Arg::new("id-or-url")
                        .short('i')
                        .long("id")
                        .help("Video ID or URL")
                        .num_args(1)
                        .required(true),
                )
                .arg(
                    Arg::new("path")
                        .long("path")
                        .short('p')
                        .help("Location folder to download")
                        .num_args(1)
                        .required(true),
                ),
        );

    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("download", download_matches)) => {
            let search_package: Vec<_> = download_matches
                .get_many::<String>("id-or-url")
                .expect("id-or-url")
                .map(|s| s.as_str())
                .collect();

            let search_values = search_package.join(" ");

            let path_package: Vec<_> = download_matches
                .get_many::<String>("path")
                .expect("path")
                .map(|s| s.as_str())
                .collect();

            let path_values_string = path_package.join(" ");
            let path_values = Path::new(&path_values_string);

            if !path_values.exists() {
                print_error("Folder path not found!");
                exit(1);
            } else if !path_values.is_dir() {
                print_error("Output path must be folder!");
                exit(1);
            }

            let download_options = VideoOptions {
                quality: VideoQuality::Highest,
                filter: VideoSearchOptions::Video,
                ..Default::default()
            };

            // if everything is okay continue to search with spesific paramters
            let video = Video::new_with_options(&search_values, download_options.clone());

            if video.is_err() {
                print_error(video.err().unwrap().to_string());
                exit(1);
            }

            let video = video.unwrap();

            let video_info = video.get_info().await;

            if video_info.is_err() {
                print_error(video_info.err().unwrap().to_string());
                exit(1);
            }

            let video_info = video_info.unwrap();

            let stream = video.stream().await;

            if stream.is_err() {
                print_error(stream.err().unwrap().to_string());
                exit(1);
            }

            let stream = stream.unwrap();

            let download_file = video_info.video_details.video_id;
            let video_size = stream.content_length();
            let video_format = choose_format(&video_info.formats, &download_options);

            video_info
                .formats
                .iter()
                .for_each(|x| println!("{:?} {:?}", x.codecs, x.quality_label));
            println!("{:?}", video_info.formats.len());

            let pb = ProgressBar::new(video_size as u64);

            pb.set_style(ProgressStyle::with_template("{msg}\n\n{spinner:.blue} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("█░░")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"));

            pb.set_message(format!(
                "{} {}",
                video_info.video_details.title.cyan(),
                "is downloading...".white().bold(),
            ));

            let mut downloaded = 0_u64;

            let pb_clone = pb.clone();
            let path_package_clone: Vec<_> = download_matches
                .get_many::<String>("path")
                .expect("path")
                .map(|s| s.clone())
                .collect();

            let download_file_clone = download_file.clone();

            let future = async move {
                let path_values_string = path_package_clone.join(" ");
                let path_values = Path::new(&path_values_string);

                let file = std::fs::File::create(
                    path_values.join(Path::new(format!("{}.mp3", download_file_clone).as_str())),
                );
                if let Err(err) = file {
                    pb_clone.finish_and_clear();
                    print_error(err.to_string());
                    exit(1);
                }
                let mut file = file.unwrap();

                while let Some(bytes) = stream.chunk().await.unwrap() {
                    use std::io::Write;

                    if let Err(err) = file.write_all(&bytes) {
                        pb_clone.finish_and_clear();
                        print_error(err.to_string());
                        exit(1);
                    }

                    let new = std::cmp::min(downloaded + bytes.len() as u64, video_size as u64);
                    downloaded = new;
                    pb_clone.set_position(new);
                }
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
            download_thread.await.unwrap();

            if finished {
                progress_tick.abort()
            }

            pb.finish_and_clear();

            // Display successfuly download message than exit with success
            println!(
                "\n{} {}\n",
                "Video successfully downloaded to".white().bold(),
                String::from(
                    path_values
                        .join(Path::new(format!("{}.mp3", download_file).as_str()))
                        .to_string_lossy()
                )
                .as_str()
                .green()
                .underline()
            );
            exit(0);
        }
        None => unreachable!(),
        _ => unreachable!(),
    }
}

fn print_error(msg: impl Into<String>) {
    println!(
        "{} {}\n\nFor more information, try '{}'.",
        "error:".bold().red(),
        msg.into(),
        "--help".white().bold(),
    );
}
