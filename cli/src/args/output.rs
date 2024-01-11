use anyhow::{Error, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// use crate::output_level::OutputLevel;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Debug,
    PrettyDebug,
    Json,
    PrettyJson,
    Yaml,
    Stdout,
}

impl OutputFormat {
    pub fn serialize<T>(&self, output: &T) -> Result<String>
    where
        T: Serialize + std::fmt::Debug,
    {
        use OutputFormat::*;

        match self {
            Debug => Ok(format!("{output:?}")),
            PrettyDebug => Ok(format!("{output:#?}")),
            Json => Ok(serde_json::to_string(output)?),
            PrettyJson => Ok(serde_json::to_string_pretty(output)?),
            Yaml => Ok(serde_yaml::to_string(output)?),
            Stdout => Ok(format!("{output:?}")),
        }
    }
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::PrettyJson
    }
}

bitflags::bitflags! {
    #[derive(Clone, Debug)]
    pub struct OutputLevel: u16 {
        const GENERAL       = 0b000000100;
        const VIDEO_TRACK   = 0b000001000;
        const AUDIO_TRACK   = 0b000010000;
        const VERBOSE       = 0b100000000;

        const VIDEO         = 0b010000000;
    }
}

impl FromStr for OutputLevel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s
            .trim()
            .split(|c: char| c.is_whitespace() || c == ',' || c == '|');
        let mut level = Self::empty();

        for s in split {
            if s.is_empty() {
                continue;
            }

            let next_level = match s {
                "general" => Self::GENERAL,
                "video-track" => Self::VIDEO_TRACK,
                "audio-track" => Self::AUDIO_TRACK,
                "verbose" => Self::VERBOSE,
                "all" => Self::all(),

                "video-info" => Self::VIDEO,

                _ => anyhow::bail!("Could not parse {:?} to an OutputLevel", s),
            };

            level |= next_level;
        }

        Ok(level)
    }
}

#[derive(Parser)]
pub struct OutputArgs {
    /// The format in which the information should be printed
    ///
    /// [possible_values: debug, pretty-debug, json, pretty-json, yaml, stdout]
    #[clap(
    short, long = "output",
    default_value = "pretty-json",
    value_parser = parse_from_str,
    )]
    pub output_format: OutputFormat,

    /// The amount of information printed to the terminal
    ///
    /// To get more information, different levels can be combined, by separating them with a `|`.
    ///
    /// [possible_values: general, video-track, audio-track, video-info]
    #[clap(
        short = 'l',
        long = "level",
        default_value = "general | video-track | audio-track"
    )]
    pub output_level: OutputLevel,
}

fn parse_from_str(s: &str) -> anyhow::Result<OutputFormat> {
    Ok(serde_json::from_str(&format!("\"{s}\""))?)
}
