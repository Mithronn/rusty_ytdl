use crate::block_async;
use crate::structs::{VideoError, VideoInfo, VideoOptions};
use crate::Video as AsyncVideo;

#[derive(Clone, Debug, derive_more::Display, PartialEq, Eq)]
pub struct Video(AsyncVideo);

impl Video {
    pub fn new(url_or_id: impl Into<String>) -> Result<Self, VideoError> {
        Ok(Self(AsyncVideo::new(url_or_id)?))
    }

    pub fn new_with_options(
        url_or_id: impl Into<String>,
        options: VideoOptions,
    ) -> Result<Self, VideoError> {
        Ok(Self(AsyncVideo::new_with_options(url_or_id, options)?))
    }

    pub fn get_basic_info(&self) -> Result<VideoInfo, VideoError> {
        Ok(block_async!(self.0.get_basic_info())?)
    }

    pub fn get_info(&self) -> Result<VideoInfo, VideoError> {
        Ok(block_async!(self.0.get_info())?)
    }

    pub fn download(&self) -> Result<Vec<u8>, VideoError> {
        Ok(block_async!(self.0.download())?)
    }

    pub fn get_video_url(&self) -> String {
        self.0.get_video_url()
    }

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
