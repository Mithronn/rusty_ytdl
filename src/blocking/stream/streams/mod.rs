#[cfg(feature = "live")]
pub use crate::stream::LiveStreamOptions;
pub use crate::stream::NonLiveStreamOptions;

use crate::VideoError;

#[cfg(feature = "live")]
mod live;
mod non_live;

#[cfg(feature = "live")]
pub use live::LiveStream;
pub use non_live::NonLiveStream;

pub trait Stream {
    /// Stream a chunk of the [`u8`] bytes
    ///
    /// When the bytes has been exhausted, this will return `None`.
    fn chunk(&self) -> Result<Option<Vec<u8>>, VideoError>;

    /// Content length of the stream
    ///
    /// If stream is [`LiveStream`] returns always `0`
    fn content_length(&self) -> usize {
        0
    }
}
