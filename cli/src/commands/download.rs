use clap::Parser;
use std::path::PathBuf;

use crate::args::{log::LogArgs, output::OutputArgs, video_options::VideoOptionsArgs};

#[derive(Parser)]
pub struct DownloadArgs {
    #[clap(
        short = 'i',
        long = "id",
        help = "Video ID or URL",
        num_args = 1,
        required = true
    )]
    pub id: String,

    #[clap(flatten)]
    pub video_options: VideoOptionsArgs,

    #[clap(flatten)]
    pub log: LogArgs,

    #[clap(flatten)]
    pub output: OutputArgs,

    /// Where to download the video to [default: **./**]
    #[clap(
        short = 'p',
        long = "path",
        help = "Location folder to download [default: ./]",
        num_args = 1,
        required = false
    )]
    pub path: Option<PathBuf>,

    /// The filename of the video file [default: <VIDEO_ID>.mp3]
    ///
    /// If the file already exists, it will be removed, even if the download fails!
    #[clap(
        short,
        long = "filename",
        help = "The filename of the video file [default: <VIDEO_ID>.mp3]",
        num_args = 1,
        required = false
    )]
    pub filename: Option<PathBuf>,
}
