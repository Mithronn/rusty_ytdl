pub use crate::stream::{LiveStreamOptions, NonLiveStreamOptions};

use crate::stream::{LiveStream as AsyncLiveStream, NonLiveStream as AsyncNonLiveStream};
use crate::{block_async, VideoError};

pub trait Stream {
    /// Stream a chunk of the [`u8`] bytes
    ///
    /// When the bytes has been exhausted, this will return `None`.
    fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError>;
}

pub struct NonLiveStream(AsyncNonLiveStream);

impl NonLiveStream {
    pub fn new(options: NonLiveStreamOptions) -> Result<Self, VideoError> {
        Ok(Self(AsyncNonLiveStream::new(options)?))
    }
}

impl Stream for NonLiveStream {
    fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError> {
        use crate::stream::Stream;
        Ok(block_async!(self.0.chunk())?)
    }
}

impl std::ops::Deref for NonLiveStream {
    type Target = AsyncNonLiveStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for NonLiveStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct LiveStream(AsyncLiveStream);

impl LiveStream {
    pub fn new(options: LiveStreamOptions) -> Result<Self, VideoError> {
        Ok(Self(AsyncLiveStream::new(options)?))
    }
}

impl Stream for LiveStream {
    fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError> {
        use crate::stream::Stream;
        Ok(block_async!(self.0.chunk())?)
    }
}

impl std::ops::Deref for LiveStream {
    type Target = AsyncLiveStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for LiveStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
