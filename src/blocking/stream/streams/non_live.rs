use bytes::Bytes;

use crate::blocking::stream::Stream;
use crate::stream::{NonLiveStream as AsyncNonLiveStream, NonLiveStreamOptions};
use crate::{block_async, VideoError};

pub struct NonLiveStream(AsyncNonLiveStream);

impl NonLiveStream {
    pub fn new(options: NonLiveStreamOptions) -> Result<Self, VideoError> {
        Ok(Self(AsyncNonLiveStream::new(options)?))
    }
}

impl Stream for NonLiveStream {
    fn chunk(&self) -> Result<Option<Bytes>, VideoError> {
        use crate::stream::Stream;
        Ok(block_async!(self.0.chunk())?)
    }

    fn content_length(&self) -> usize {
        self.0.content_length() as usize
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
