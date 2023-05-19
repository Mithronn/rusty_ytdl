use std::hash::Hash;
use std::ops::Deref;

use m3u8_rs::ByteRange;

#[derive(Clone, Eq, Debug)]
pub struct HashableByteRange(ByteRange);

impl HashableByteRange {
    pub fn new(b: ByteRange) -> Self {
        Self(b)
    }
}

impl Deref for HashableByteRange {
    type Target = ByteRange;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for HashableByteRange {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for HashableByteRange {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.length.hash(state);
        self.0.offset.hash(state);
    }
}
