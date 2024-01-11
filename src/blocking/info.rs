use crate::block_async;
#[cfg(feature = "live")]
use crate::blocking::stream::LiveStream;
use crate::blocking::stream::NonLiveStream;
use crate::structs::{VideoError, VideoInfo, VideoOptions};
use crate::utils::choose_format;
use crate::Video as AsyncVideo;

#[cfg(feature = "live")]
use super::stream::LiveStreamOptions;
use super::stream::{NonLiveStreamOptions, Stream};

#[derive(Clone, Debug, derive_more::Display, PartialEq, Eq)]
pub struct Video(AsyncVideo);

impl Video {
    /// Crate [`Video`] struct to get info or download with default [`VideoOptions`]
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        Ok(Self(AsyncVideo::new(url_or_id)?))
    }

    /// Crate [`Video`] struct to get info or download with custom [`VideoOptions`]
    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: VideoOptions,
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
                });

                if stream.is_err() {
                    return Err(stream.err().unwrap());
                }

                return Ok(Box::new(stream.unwrap()));
            }
            #[cfg(not(feature = "live"))]
            {
                return Err(VideoError::LiveStreamNotSupported);
            }
        }

        let dl_chunk_size = if options.download_options.dl_chunk_size.is_some() {
            options.download_options.dl_chunk_size.unwrap()
        } else {
            1024 * 1024 * 10_u64 // -> Default is 10MB to avoid Youtube throttle (Bigger than this value can be throttle by Youtube)
        };

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
                .content_length();

            if content_length_response.is_none() {
                return Err(VideoError::VideoNotFound);
            }

            content_length = content_length_response.unwrap();
        }

        let stream = NonLiveStream::new(NonLiveStreamOptions {
            client: Some(client.clone()),
            link,
            content_length,
            dl_chunk_size,
            start,
            end,
        })?;

        Ok(Box::new(stream))
    }

    /// Download video directly to the file
    pub fn download<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), VideoError> {
        Ok(block_async!(self.0.download(path))?)
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

impl std::ops::Deref for Video {
    type Target = AsyncVideo;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Video {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
