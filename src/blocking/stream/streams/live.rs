use crate::blocking::stream::Stream;
use crate::stream::{LiveStream as AsyncLiveStream, LiveStreamOptions};
use crate::{block_async, VideoError};

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
