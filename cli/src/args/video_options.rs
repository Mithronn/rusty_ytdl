use clap::Parser;
use rusty_ytdl::VideoQuality;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
pub struct VideoOptionsArgs {
    /// Pick a Stream, that contains only video track
    #[clap(
        long,
        conflicts_with_all(& ["only_audio"])
    )]
    pub only_video: bool,

    /// Pick a Stream, that contains only audio track
    #[clap(
        long,
        conflicts_with_all(& ["only_video"])
    )]
    pub only_audio: bool,

    /// Download the stream with this quality
    ///
    /// [possible_values: highest, lowest, highest_audio, lowest_audio, highest_video, lowest_video]
    #[clap(
    long,
    value_enum,
    value_parser = parse_from_str,
    default_value = "highest",
    )]
    pub quality: Option<Quality>,
}

fn parse_from_str(s: &str) -> anyhow::Result<Quality> {
    Ok(serde_json::from_str(&format!("\"{s}\""))?)
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Quality {
    #[serde(rename = "highest")]
    Highest,
    #[serde(rename = "lowest")]
    Lowest,
    #[serde(rename = "highest_audio")]
    HighestAudio,
    #[serde(rename = "lowest_audio")]
    LowestAudio,
    #[serde(rename = "highest_video")]
    HighestVideo,
    #[serde(rename = "lowest_video")]
    LowestVideo,
}

impl From<Quality> for VideoQuality {
    fn from(value: Quality) -> Self {
        match value {
            Quality::Highest => VideoQuality::Highest,
            Quality::Lowest => VideoQuality::Lowest,
            Quality::HighestAudio => VideoQuality::HighestAudio,
            Quality::LowestAudio => VideoQuality::LowestAudio,
            Quality::HighestVideo => VideoQuality::HighestVideo,
            Quality::LowestVideo => VideoQuality::LowestVideo,
        }
    }
}
