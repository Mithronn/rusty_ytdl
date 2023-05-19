pub use crate::search::{
    Channel, EmbedOptions, PlaylistSearchOptions, RequestOptions, SearchOptions, SearchResult,
    SearchType, Video,
};
use crate::search::{Playlist as AsyncPlaylist, YouTube as AsyncYouTube};
use crate::{block_async, VideoError};
use serde::Serialize;

#[derive(Clone, derive_more::Display, derivative::Derivative)]
#[display(fmt = "YouTube()")]
#[derivative(Debug, PartialEq, Eq)]
pub struct YouTube(AsyncYouTube);

impl YouTube {
    pub fn new() -> Result<Self, VideoError> {
        Ok(Self(AsyncYouTube::new()?))
    }

    pub fn new_with_options(request_options: &RequestOptions) -> Result<Self, VideoError> {
        Ok(Self(AsyncYouTube::new_with_options(&request_options)?))
    }

    pub fn search(
        &self,
        query: impl Into<String>,
        search_options: Option<&SearchOptions>,
    ) -> Result<Vec<SearchResult>, VideoError> {
        Ok(block_async!(self.0.search(query, search_options))?)
    }

    pub fn search_one(
        &self,
        query: impl Into<String>,
        search_options: Option<&SearchOptions>,
    ) -> Result<Option<SearchResult>, VideoError> {
        Ok(block_async!(self.0.search_one(query, search_options))?)
    }
}

impl std::ops::Deref for YouTube {
    type Target = AsyncYouTube;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for YouTube {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, derivative::Derivative, Serialize)]
#[derivative(Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist(pub(super) AsyncPlaylist);

impl Playlist {
    /// Try to get [`Playlist`] than fetch videos according to the [`PlaylistSearchOptions`]
    pub fn get(
        url: impl Into<String>,
        options: Option<&PlaylistSearchOptions>,
    ) -> Result<Self, VideoError> {
        Ok(Self(block_async!(AsyncPlaylist::get(url, options))?))
    }

    /// Get next chunk of videos from playlist and return fetched [`Video`] array.
    /// - If limit is [`None`] it will be [`u64::MAX`]
    /// - If [`Playlist`] is coming from [`SearchResult`] this function always return empty [`Vec<Video>`]!
    /// to use this function with [`SearchResult`] follow example
    ///
    /// # Example
    ///
    /// ```
    /// let youtube = YouTube::new().unwrap();
    ///
    /// let res = youtube
    ///    .search(
    ///       "manga",
    ///       Some(&SearchOptions {
    ///           search_type: SearchType::Playlist,
    ///           ..Default::default()
    ///       }),
    /// );
    ///
    /// for result in res.unwrap() {
    ///    match result {
    ///       SearchResult::Playlist(raw_playlist) => {
    ///            let mut playlist = Playlist::get(raw_playlist.url, None);
    ///            playlist.unwrap().next(Some(50)).unwrap();
    ///       }
    ///       _ => {}
    ///    }
    /// }
    /// ```
    pub fn next(&mut self, limit: Option<u64>) -> Result<Vec<Video>, VideoError> {
        Ok(block_async!(self.0.next(limit))?)
    }

    /// Try to fetch all playlist videos and return [`Playlist`].
    /// - If limit is [`None`] it will be [`u64::MAX`]
    /// - If [`Playlist`] is coming from [`SearchResult`] this function always return [`Playlist`] with empty [`Vec<Video>`]!
    /// to use this function with [`SearchResult`] follow example
    ///
    /// # Example
    ///
    /// ```
    /// let youtube = YouTube::new().unwrap();
    ///
    /// let res = youtube
    ///    .search(
    ///       "manga",
    ///       Some(&SearchOptions {
    ///           search_type: SearchType::Playlist,
    ///           ..Default::default()
    ///       }),
    /// );
    ///
    /// for result in res.unwrap() {
    ///    match result {
    ///       SearchResult::Playlist(raw_playlist) => {
    ///            let playlist = Playlist::get(raw_playlist.url, None);
    ///            let playlist = playlist.unwrap().fetch(None);
    ///       }
    ///       _ => {}
    ///    }
    /// }
    /// ```
    pub fn fetch(&mut self, limit: Option<u64>) -> &mut Self {
        self.0 = block_async!(self.0.fetch(limit)).clone();

        self
    }

    pub fn is_playlist(url_or_id: impl Into<String>) -> bool {
        AsyncPlaylist::is_playlist(url_or_id)
    }

    pub fn get_playlist_url(url_or_id: impl Into<String>) -> Option<String> {
        AsyncPlaylist::get_playlist_url(url_or_id)
    }
}

impl std::ops::Deref for Playlist {
    type Target = AsyncPlaylist;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Playlist {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
