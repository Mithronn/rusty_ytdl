use std::path::Path;
use std::borrow::Cow;

use crate::block_async;
#[cfg(feature = "live")]
use crate::blocking::stream::LiveStream;
use crate::blocking::stream::NonLiveStream;
use crate::constants::DEFAULT_DL_CHUNK_SIZE;
use crate::structs::{VideoError, VideoInfo, VideoOptions};
use crate::utils::choose_format;
use crate::Video as AsyncVideo;

#[cfg(feature = "live")]
use super::stream::LiveStreamOptions;
use super::stream::{NonLiveStreamOptions, Stream};

#[cfg(feature = "ffmpeg")]
use crate::structs::FFmpegArgs;

#[derive(Clone, Debug, derive_more::Display, PartialEq, Eq)]
/// If a video was created with a reference to options, it is tied to their lifetime `'opts`.
pub struct Video<'opts>(AsyncVideo<'opts>);

impl Video<'static> {
    /// Crate [`Video`] struct to get info or download with default [`VideoOptions`]
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        Ok(Self(AsyncVideo::new(url_or_id)?))
    }
}

impl<'opts> Video<'opts> {
    /// Crate [`Video`] struct to get info or download with custom [`VideoOptions`]
    /// `VideoOptions` can be passed by value or by reference, if passed by
    /// reference, returned `Video` will be tied to the lifetime of the `VideoOptions`.
    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: impl Into<Cow<'opts, VideoOptions>>,
    ) -> Result<Self, VideoError> {
        Ok(Self(AsyncVideo::new_with_options(url_or_id, options)?))
    }

    /// Try to get basic information about video
    /// - `HLS` and `DashMPD` formats excluded!
    pub fn get_basic_info(&self) -> Result<VideoInfo, VideoError> {
        Ok(block_async!(self.0.get_basic_info())?)
    }

    /// Try to get full information about video
    /// - `HLS` and `DashMPD` formats included!
    pub fn get_info(&self) -> Result<VideoInfo, VideoError> {
        Ok(block_async!(self.0.get_info())?)
    }

    /// Try to turn [`Stream`] implemented [`LiveStream`] or [`NonLiveStream`] depend on the video.
    /// If function successfully return can download video chunk by chunk
    /// # Example
    /// ```ignore
    ///     let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";
    ///
    ///     let video = Video::new(video_url).unwrap();
    ///
    ///     let stream = video.stream().unwrap();
    ///
    ///     while let Some(chunk) = stream.chunk().unwrap() {
    ///           println!("{:#?}", chunk);
    ///     }
    /// ```
    pub fn stream(&self) -> Result<Box<dyn Stream + Send + Sync>, VideoError> {
        let client = self.0.get_client();

        let options = self.0.get_options();

        let info = block_async!(self.0.get_info())?;
        let format = choose_format(&info.formats, &options)
            .map_err(|_op| VideoError::VideoSourceNotFound)?;

        let link = format.url;

        if link.is_empty() {
            return Err(VideoError::VideoSourceNotFound);
        }

        // Only check for HLS formats for live streams
        if format.is_hls {
            #[cfg(feature = "live")]
            {
                let stream = LiveStream::new(LiveStreamOptions {
                    client: Some(client.clone()),
                    stream_url: link,
                })?;

                return Ok(Box::new(stream));
            }
            #[cfg(not(feature = "live"))]
            {
                return Err(VideoError::LiveStreamNotSupported);
            }
        }

        let dl_chunk_size = options
            .download_options
            .dl_chunk_size
            .unwrap_or(DEFAULT_DL_CHUNK_SIZE);

        let start = 0;
        let end = start + dl_chunk_size;

        let mut content_length = format
            .content_length
            .unwrap_or("0".to_string())
            .parse::<u64>()
            .unwrap_or(0);

        // Get content length from source url if content_length is 0
        if content_length == 0 {
            let content_length_response = block_async!(client.get(&link).send())
                .map_err(VideoError::ReqwestMiddleware)?
                .content_length()
                .ok_or(VideoError::VideoNotFound)?;

            content_length = content_length_response;
        }

        let stream = NonLiveStream::new(NonLiveStreamOptions {
            client: Some(client.clone()),
            link,
            content_length,
            dl_chunk_size,
            start,
            end,
            #[cfg(feature = "ffmpeg")]
            ffmpeg_args: None,
        })?;

        Ok(Box::new(stream))
    }

    #[cfg(feature = "ffmpeg")]
    /// Try to turn [`Stream`] implemented [`LiveStream`] or [`NonLiveStream`] depend on the video with [`FFmpegArgs`].
    /// If function successfully return can download video with applied ffmpeg filters and formats chunk by chunk
    /// # Example
    /// ```ignore
    ///     let video_url = "https://www.youtube.com/watch?v=FZ8BxMU3BYc";
    ///
    ///     let video = Video::new(video_url).unwrap();
    ///
    ///     let stream = video.stream_with_ffmpeg(Some(FFmpegArgs {
    ///            format: Some("mp3".to_string()),
    ///            audio_filter: Some("aresample=48000,asetrate=48000*0.8".to_string()),
    ///            video_filter: Some("eq=brightness=150:saturation=2".to_string()),
    ///        })).unwrap();
    ///
    ///     while let Some(chunk) = stream.chunk().unwrap() {
    ///           println!("{:#?}", chunk);
    ///     }
    /// ```
    pub async fn stream_with_ffmpeg(
        &self,
        ffmpeg_args: Option<FFmpegArgs>,
    ) -> Result<Box<dyn Stream + Send + Sync>, VideoError> {
        let client = self.0.get_client();

        let options = self.0.get_options();

        let info = block_async!(self.0.get_info())?;
        let format = choose_format(&info.formats, &options)
            .map_err(|_op| VideoError::VideoSourceNotFound)?;

        let link = format.url;

        if link.is_empty() {
            return Err(VideoError::VideoSourceNotFound);
        }

        // Only check for HLS formats for live streams
        if format.is_hls {
            #[cfg(feature = "live")]
            {
                let stream = LiveStream::new(LiveStreamOptions {
                    client: Some(client.clone()),
                    stream_url: link,
                })?;

                return Ok(Box::new(stream));
            }
            #[cfg(not(feature = "live"))]
            {
                return Err(VideoError::LiveStreamNotSupported);
            }
        }

        let dl_chunk_size = options
            .download_options
            .dl_chunk_size
            .unwrap_or(DEFAULT_DL_CHUNK_SIZE);

        let start = 0;
        let end = start + dl_chunk_size;

        let mut content_length = format
            .content_length
            .unwrap_or("0".to_string())
            .parse::<u64>()
            .unwrap_or(0);

        // Get content length from source url if content_length is 0
        if content_length == 0 {
            let content_length_response = client
                .get(&link)
                .send()
                .await
                .map_err(VideoError::ReqwestMiddleware)?
                .content_length()
                .ok_or(VideoError::VideoNotFound)?;

            content_length = content_length_response;
        }

        let stream = NonLiveStream::new(NonLiveStreamOptions {
            client: Some(client.clone()),
            link,
            content_length,
            dl_chunk_size,
            start,
            end,
            ffmpeg_args,
        })?;

        Ok(Box::new(stream))
    }

    /// Download video directly to the file
    pub fn download<P: AsRef<Path>>(&self, path: P) -> Result<(), VideoError> {
        Ok(block_async!(self.0.download(path))?)
    }

    #[cfg(feature = "ffmpeg")]
    /// Download video with ffmpeg args directly to the file
    pub async fn download_with_ffmpeg<P: AsRef<Path>>(
        &self,
        path: P,
        ffmpeg_args: Option<FFmpegArgs>,
    ) -> Result<(), VideoError> {
        Ok(block_async!(self
            .0
            .download_with_ffmpeg(path, ffmpeg_args))?)
    }

    /// Get video URL
    pub fn get_video_url(&self) -> String {
        self.0.get_video_url()
    }

    /// Get video id
    pub fn get_video_id(&self) -> String {
        self.0.get_video_id()
    }
}

impl<'opts> std::ops::Deref for Video<'opts> {
    type Target = AsyncVideo<'opts>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'opts> std::ops::DerefMut for Video<'opts> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
