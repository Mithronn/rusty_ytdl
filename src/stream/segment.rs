#![allow(dead_code)]
use super::media_format::MediaFormat;
use super::remote_data::RemoteData;

/// Type of media segment
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Segment {
    pub data: RemoteData,
    pub discon_seq: u64,
    pub seq: u64,
    pub format: MediaFormat,
    pub initialization: Option<RemoteData>,
}

impl Segment {
    /// URL of segment
    pub fn url(&self) -> &url::Url {
        self.data.url()
    }

    /// String identifier of segment
    pub fn id(&self) -> String {
        format!("d{:010}s{:010}", self.discon_seq, self.seq)
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.discon_seq, self.seq).cmp(&(other.discon_seq, other.seq))
    }
}
