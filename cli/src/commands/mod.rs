pub mod download;

use clap::Parser;

use self::download::DownloadArgs;

#[derive(Parser)]
#[clap(
    version = "0.0.1",
    about = "\n\
    A CLI for rusty_ytdl crate.
    ",
    author = "Mithronn",
    arg_required_else_help = true,
    subcommand_required = true
)]
pub enum Commands {
    #[clap(about = "\
    Download the video to spesific folder or stdout
    ")]
    Download(DownloadArgs),
}
