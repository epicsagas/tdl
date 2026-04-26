use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::tidal::media::{
    Album, Artist, LyricsResponse, MediaType, MixPageItem, PaginatedResponse, Playlist,
    SearchResponse, Track, Video,
};
use crate::tidal::request::TidalRequest;

// ---------------------------------------------------------------------------
// TidalSearch — high-level search wrapper
// ---------------------------------------------------------------------------

pub struct TidalSearch<'a> {
    request: &'a TidalRequest,
}

impl<'a> TidalSearch<'a> {
    pub fn new(request: &'a TidalRequest) -> Self {
        Self { request }
    }

    /// Execute a general search against the Tidal catalogue.
    ///
    /// `types` is a slice of media type strings, e.g. `["tracks", "albums"]`.
    pub async fn search(
        &self,
        query: &str,
        types: &[&str],
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<SearchResponse> {
        let mut params = HashMap::new();
        params.insert("query".to_string(), query.to_string());
        params.insert("type".to_string(), types.join(","));
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }
        if let Some(offset) = offset {
            params.insert("offset".to_string(), offset.to_string());
        }

        self.request.get("search", Some(params)).await
    }

    /// Search for tracks only.
    pub async fn search_tracks(&self, query: &str, limit: Option<u64>) -> Result<Vec<Track>> {
        let resp = self.search(query, &["tracks"], limit, None).await?;
        Ok(resp.tracks.map(|p| p.items).unwrap_or_default())
    }

    /// Search for albums only.
    pub async fn search_albums(&self, query: &str, limit: Option<u64>) -> Result<Vec<Album>> {
        let resp = self.search(query, &["albums"], limit, None).await?;
        Ok(resp.albums.map(|p| p.items).unwrap_or_default())
    }

    /// Search for artists only.
    pub async fn search_artists(&self, query: &str, limit: Option<u64>) -> Result<Vec<Artist>> {
        let resp = self.search(query, &["artists"], limit, None).await?;
        Ok(resp.artists.map(|p| p.items).unwrap_or_default())
    }

    /// Search for videos only.
    pub async fn search_videos(&self, query: &str, limit: Option<u64>) -> Result<Vec<Video>> {
        let resp = self.search(query, &["videos"], limit, None).await?;
        Ok(resp.videos.map(|p| p.items).unwrap_or_default())
    }

    /// Search for playlists only.
    pub async fn search_playlists(&self, query: &str, limit: Option<u64>) -> Result<Vec<Playlist>> {
        let resp = self.search(query, &["playlists"], limit, None).await?;
        Ok(resp.playlists.map(|p| p.items).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Standalone helper functions — fetching media by ID
// ---------------------------------------------------------------------------

/// Fetch a single track by its ID.
pub async fn get_track(request: &TidalRequest, track_id: u64) -> Result<Track> {
    let path = format!("tracks/{track_id}");
    request.get(&path, None).await
}

/// Fetch a single album by its ID.
pub async fn get_album(request: &TidalRequest, album_id: u64) -> Result<Album> {
    let path = format!("albums/{album_id}");
    request.get(&path, None).await
}

/// Fetch all tracks belonging to an album, automatically paginating.
///
/// The album items endpoint wraps each track in `{"item": {track}, "type": "track"}`.
pub async fn get_album_tracks(request: &TidalRequest, album_id: u64) -> Result<Vec<Track>> {
    let path = format!("albums/{album_id}/items");
    paginate_wrapped_items::<Track>(request, &path).await
}

/// Fetch all tracks belonging to a playlist, automatically paginating.
///
/// The playlist items endpoint wraps each track in `{"item": {track}, "type": "track"}`.
pub async fn get_playlist_tracks(
    request: &TidalRequest,
    playlist_id: &str,
) -> Result<Vec<Track>> {
    let path = format!("playlists/{playlist_id}/items");
    paginate_wrapped_items::<Track>(request, &path).await
}

/// Fetch albums by an artist.
pub async fn get_artist_albums(request: &TidalRequest, artist_id: u64) -> Result<Vec<Album>> {
    let path = format!("artists/{artist_id}/albums");
    paginate_items::<Album>(request, &path).await
}

/// Fetch the user's favourite tracks, automatically paginating.
pub async fn get_favorite_tracks(
    request: &TidalRequest,
    user_id: u64,
) -> Result<Vec<Track>> {
    let path = format!("users/{user_id}/favorites/tracks");
    paginate_favorites::<Track>(request, &path).await
}

/// Fetch the user's favourite albums, automatically paginating.
pub async fn get_favorite_albums(
    request: &TidalRequest,
    user_id: u64,
) -> Result<Vec<Album>> {
    let path = format!("users/{user_id}/favorites/albums");
    paginate_favorites::<Album>(request, &path).await
}

/// Fetch the user's favourite artists, automatically paginating.
pub async fn get_favorite_artists(
    request: &TidalRequest,
    user_id: u64,
) -> Result<Vec<Artist>> {
    let path = format!("users/{user_id}/favorites/artists");
    paginate_favorites::<Artist>(request, &path).await
}

/// Fetch the user's favourite videos, automatically paginating.
pub async fn get_favorite_videos(
    request: &TidalRequest,
    user_id: u64,
) -> Result<Vec<Video>> {
    let path = format!("users/{user_id}/favorites/videos");
    paginate_favorites::<Video>(request, &path).await
}

/// Fetch timed lyrics for a track.
pub async fn get_lyrics(
    request: &TidalRequest,
    track_id: u64,
) -> Result<LyricsResponse> {
    let path = format!("tracks/{track_id}/lyrics");
    request.get(&path, None).await
}

/// Fetch a user's playlists.
pub async fn get_user_playlists(
    request: &TidalRequest,
    user_id: u64,
) -> Result<Vec<Playlist>> {
    let path = format!("users/{user_id}/playlists");
    paginate_items::<Playlist>(request, &path).await
}

/// Fetch the tracks in a mix.
///
/// Uses the `pages/mix` endpoint which returns a page structure where
/// `categories[1].pagedList.items` contains the track list.
pub async fn get_mix_items(request: &TidalRequest, mix_id: &str) -> Result<Vec<Track>> {
    let path = "pages/mix";
    let mut params = HashMap::new();
    params.insert("mixId".to_string(), mix_id.to_string());
    params.insert("deviceType".to_string(), "BROWSER".to_string());

    let page: crate::tidal::media::MixPageResponse = request.get(path, Some(params)).await?;

    // The first category is the mix header, the second contains the tracks.
    let tracks_category = page
        .categories
        .into_iter()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Mix page has no track category"))?;

    let paged = tracks_category
        .paged_list
        .ok_or_else(|| anyhow::anyhow!("Mix track category has no pagedList"))?;

    let mut tracks = Vec::new();
    for item in paged.items {
        if let Some(value) = item.item
            && let Ok(track) = serde_json::from_value::<Track>(value) {
                tracks.push(track);
            }
    }

    if tracks.is_empty() {
        anyhow::bail!("Mix contains no downloadable tracks");
    }

    Ok(tracks)
}

// ---------------------------------------------------------------------------
// URL parsing
// ---------------------------------------------------------------------------

/// Parse a Tidal media URL into a `(MediaType, id_string)` tuple.
///
/// Recognised patterns:
///   - `https://tidal.com/browse/track/12345`
///   - `https://listen.tidal.com/album/12345`
///   - `https://listen.tidal.com/playlist/abc-def`
///   - `https://tidal.com/browse/mix/mix-001`
pub fn parse_media_url(url: &str) -> Result<(MediaType, String)> {
    // Strip query string and trailing slash.
    let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');

    let segments: Vec<&str> = path.split('/').collect();

    // Walk segments looking for a known type followed by an ID.
    for i in 0..segments.len().saturating_sub(1) {
        let seg = segments[i].to_ascii_lowercase();
        let next = segments.get(i + 1).unwrap_or(&"");

        match seg.as_str() {
            "track" => return Ok((MediaType::Track, next.to_string())),
            "video" => return Ok((MediaType::Video, next.to_string())),
            "album" => return Ok((MediaType::Album, next.to_string())),
            "artist" => return Ok((MediaType::Artist, next.to_string())),
            "playlist" => return Ok((MediaType::Playlist, next.to_string())),
            "mix" => return Ok((MediaType::Mix, next.to_string())),
            _ => {}
        }
    }

    Err(anyhow!(
        "Could not extract media type and ID from URL: {url}"
    ))
}

// ---------------------------------------------------------------------------
// Pagination helpers (private)
// ---------------------------------------------------------------------------

/// Paginate endpoints that return `PaginatedResponse<T>` (search results,
/// artist albums, user playlists, favorites).
///
/// The `TidalRequest::get` method (via `get_v1`) already injects a default
/// `limit` of 10 000.  We still explicitly control pagination here so that
/// responses that do not fit in a single page are fully collected.
async fn paginate_items<T: serde::de::DeserializeOwned>(
    request: &TidalRequest,
    base_path: &str,
) -> Result<Vec<T>> {
    let limit: u64 = 100;
    let mut offset: u64 = 0;
    let mut all_items: Vec<T> = Vec::new();

    loop {
        let mut params = HashMap::new();
        params.insert("limit".to_string(), limit.to_string());
        params.insert("offset".to_string(), offset.to_string());

        let page: PaginatedResponse<T> = request.get(base_path, Some(params)).await?;

        let count = page.items.len();
        all_items.extend(page.items);

        let total = page.total_number_of_items.unwrap_or(0);

        if (count as u64) < limit || (total > 0 && all_items.len() as u64 >= total) {
            break;
        }

        offset += limit;
    }

    Ok(all_items)
}

/// Paginate endpoints that wrap each item in `{"item": {...}, "type": "..."}`.
///
/// The album items and playlist items endpoints use this format.
async fn paginate_wrapped_items<T: serde::de::DeserializeOwned>(
    request: &TidalRequest,
    base_path: &str,
) -> Result<Vec<T>> {
    let limit: u64 = 100;
    let mut offset: u64 = 0;
    let mut all_items: Vec<T> = Vec::new();

    loop {
        let mut params = HashMap::new();
        params.insert("limit".to_string(), limit.to_string());
        params.insert("offset".to_string(), offset.to_string());

        let page: PaginatedResponse<MixPageItem> =
            request.get(base_path, Some(params)).await?;

        let count = page.items.len();
        for wrapped in page.items {
            if let Some(value) = wrapped.item {
                if let Ok(item) = serde_json::from_value::<T>(value) {
                    all_items.push(item);
                }
            }
        }

        let total = page.total_number_of_items.unwrap_or(0);

        if (count as u64) < limit || (total > 0 && all_items.len() as u64 >= total) {
            break;
        }

        offset += limit;
    }

    Ok(all_items)
}

/// Paginate favourites endpoints.
///
/// These use the same `PaginatedResponse` envelope shape.
async fn paginate_favorites<T: serde::de::DeserializeOwned>(
    request: &TidalRequest,
    base_path: &str,
) -> Result<Vec<T>> {
    paginate_items::<T>(request, base_path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_track() {
        let (mt, id) =
            parse_media_url("https://tidal.com/browse/track/12345").unwrap();
        assert_eq!(mt, MediaType::Track);
        assert_eq!(id, "12345");
    }

    #[test]
    fn parse_url_album() {
        let (mt, id) =
            parse_media_url("https://listen.tidal.com/album/67890").unwrap();
        assert_eq!(mt, MediaType::Album);
        assert_eq!(id, "67890");
    }

    #[test]
    fn parse_url_playlist() {
        let (mt, id) =
            parse_media_url("https://listen.tidal.com/playlist/abc-def-123").unwrap();
        assert_eq!(mt, MediaType::Playlist);
        assert_eq!(id, "abc-def-123");
    }

    #[test]
    fn parse_url_mix() {
        let (mt, id) =
            parse_media_url("https://tidal.com/browse/mix/mix-001").unwrap();
        assert_eq!(mt, MediaType::Mix);
        assert_eq!(id, "mix-001");
    }

    #[test]
    fn parse_url_video() {
        let (mt, id) =
            parse_media_url("https://tidal.com/browse/video/99999").unwrap();
        assert_eq!(mt, MediaType::Video);
        assert_eq!(id, "99999");
    }

    #[test]
    fn parse_url_with_query_params() {
        let (mt, id) = parse_media_url(
            "https://listen.tidal.com/track/42?foo=bar&baz=qux",
        )
        .unwrap();
        assert_eq!(mt, MediaType::Track);
        assert_eq!(id, "42");
    }

    #[test]
    fn parse_url_trailing_slash() {
        let (mt, id) =
            parse_media_url("https://listen.tidal.com/album/111/").unwrap();
        assert_eq!(mt, MediaType::Album);
        assert_eq!(id, "111");
    }

    #[test]
    fn parse_url_failure() {
        let result = parse_media_url("https://example.com/nope");
        assert!(result.is_err());
    }

    #[test]
    fn parse_url_artist() {
        let (mt, id) =
            parse_media_url("https://listen.tidal.com/artist/5555").unwrap();
        assert_eq!(mt, MediaType::Artist);
        assert_eq!(id, "5555");
    }
}
