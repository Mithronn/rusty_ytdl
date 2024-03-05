use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use rusty_ytdl::{VideoFormat, VideoInfo};

use crate::args::output::OutputLevel;

#[derive(Debug)]
pub struct ResultSerializer {
    output_level: OutputLevel,
    video_info: VideoInfo,
    formats: Vec<FormatSerializer>,
}

impl ResultSerializer {
    pub fn new(video_info: VideoInfo, output_level: OutputLevel) -> Self {
        let mut formats = video_info.formats.clone();

        let formats = formats
            .iter_mut()
            .map(|format| FormatSerializer {
                format: format.clone(),
                output_level: output_level.clone(),
            })
            .collect::<Vec<_>>();

        Self {
            output_level,
            video_info,
            formats,
        }
    }
}

impl Serialize for ResultSerializer {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;

        if self.output_level.contains(OutputLevel::VIDEO) {
            map.serialize_entry("video_info", &self.video_info.video_details)?;
        }

        map.serialize_entry("streams", &self.formats)?;

        map.end()
    }
}

#[derive(Debug)]
struct FormatSerializer {
    pub output_level: OutputLevel,
    pub format: VideoFormat,
}

impl Serialize for FormatSerializer {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        macro_rules! partly_serialize {
            ($self:ident, $map:ident; $($level:expr => { $($field:ident $(with $func:ident)?),* $(,)? })*) => {
                $(
                    if self.output_level.contains($level) {
                        $(
                            $map.serialize_entry(
                                stringify!($field),
                                &partly_serialize!{ @__ser($self.format.$field $(=> $func)?) }
                            )?;
                        )*
                    }
                )*
            };
            (@__ser($field:expr)) => { $field };
            (@__ser($field:expr => $func:ident)) => { $func(&$field) };
        }

        let mut map = serializer.serialize_map(None)?;

        partly_serialize!(self, map;
            OutputLevel::GENERAL => {
                itag, mime_type, quality, has_video, has_audio,
                approx_duration_ms, url
            }
            OutputLevel::GENERAL | OutputLevel::VERBOSE => {

            }

            OutputLevel::VIDEO_TRACK => {
                height, width, quality_label, fps
            }
            OutputLevel::VIDEO_TRACK | OutputLevel::VERBOSE => {
                color_info, high_replication,
            }

            OutputLevel::AUDIO_TRACK => {
                audio_quality, bitrate, audio_sample_rate, audio_channels, loudness_db, high_replication
            }
            OutputLevel::AUDIO_TRACK | OutputLevel::VERBOSE => {
                average_bitrate
            }

            OutputLevel::all() => {
                index_range, init_range, last_modified, projection_type, is_live, is_hls, is_dash_mpd
            }
        );

        map.end()
    }
}
