use serde::de::{self, Deserializer};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaType {
    Track,
    Video,
    Playlist,
    Album,
    Mix,
    Artist,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
#[allow(non_camel_case_types)]
pub enum AudioQuality {
    LOW,
    HIGH,
    LOSSLESS,
    HI_RES_LOSSLESS,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
#[allow(non_camel_case_types)]
pub enum AudioMode {
    STEREO,
    DOLBY_ATMOS,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum Codec {
    MP3,
    AAC,
    MP4A,
    FLAC,
    EAC3,
    AC4,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum EncryptionType {
    NONE,
}

// ---------------------------------------------------------------------------
// Helper deserializers
// ---------------------------------------------------------------------------

/// Deserialize a value that may be a number or a string into a u64.
fn deserialize_flexible_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Visitor;

    struct FlexibleIdVisitor;

    impl Visitor<'_> for FlexibleIdVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a number or a string containing a number")
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u64, E> {
            Ok(v as u64)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse::<u64>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(FlexibleIdVisitor)
}

/// Deserialize a value that may be a bool, null, or absent into Option<bool>.
/// Some endpoints return `populate` as a bool; others omit it entirely.
fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<bool>::deserialize(deserializer)
}

// ---------------------------------------------------------------------------
// Album: intermediate deserialization struct + TryFrom
// ---------------------------------------------------------------------------

/// Intermediate representation used to handle the `id` field which can be
/// either a number or a string depending on the Tidal API endpoint.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumDeserialize {
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_id",
        rename = "id"
    )]
    id: u64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    num_tracks: Option<u32>,
    #[serde(default)]
    num_videos: Option<u32>,
    #[serde(default)]
    num_volumes: Option<u32>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    available_release_date: Option<String>,
    #[serde(default)]
    copyright: Option<String>,
    #[serde(default)]
    upc: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    explicit: Option<bool>,
    #[serde(default)]
    audio_quality: Option<AudioQuality>,
    #[serde(default)]
    audio_modes: Option<Vec<AudioMode>>,
    #[serde(default)]
    media_metadata_tags: Option<Vec<String>>,
    #[serde(default)]
    artist: Option<Artist>,
    #[serde(default)]
    artists: Option<Vec<Artist>>,
    #[serde(default)]
    year: Option<u32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_bool",
        rename = "populate"
    )]
    populate: Option<bool>,
}

impl TryFrom<AlbumDeserialize> for Album {
    type Error = String;

    fn try_from(v: AlbumDeserialize) -> Result<Self, Self::Error> {
        Ok(Album {
            id: v.id,
            name: v.name.unwrap_or_default(),
            cover: v.cover,
            duration: v.duration,
            num_tracks: v.num_tracks,
            num_videos: v.num_videos,
            num_volumes: v.num_volumes,
            release_date: v.release_date,
            available_release_date: v.available_release_date,
            copyright: v.copyright,
            upc: v.upc,
            version: v.version,
            explicit: v.explicit,
            audio_quality: v.audio_quality,
            audio_modes: v.audio_modes,
            media_metadata_tags: v.media_metadata_tags,
            artist: v.artist,
            artists: v.artists,
            year: v.year,
            populate: v.populate,
        })
    }
}

// ---------------------------------------------------------------------------
// Core media structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Artist {
    #[serde(default, deserialize_with = "deserialize_flexible_id")]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "AlbumDeserialize")]
pub struct Album {
    pub id: u64,
    pub name: String,
    pub cover: Option<String>,
    pub duration: Option<u64>,
    pub num_tracks: Option<u32>,
    pub num_videos: Option<u32>,
    pub num_volumes: Option<u32>,
    pub release_date: Option<String>,
    pub available_release_date: Option<String>,
    pub copyright: Option<String>,
    pub upc: Option<String>,
    pub version: Option<String>,
    pub explicit: Option<bool>,
    pub audio_quality: Option<AudioQuality>,
    pub audio_modes: Option<Vec<AudioMode>>,
    pub media_metadata_tags: Option<Vec<String>>,
    pub artist: Option<Artist>,
    pub artists: Option<Vec<Artist>>,
    pub year: Option<u32>,
    pub populate: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(default, deserialize_with = "deserialize_flexible_id")]
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub explicit: Option<bool>,
    #[serde(default)]
    pub available: Option<bool>,
    #[serde(default)]
    pub stream_ready: Option<bool>,
    #[serde(default)]
    pub artist: Option<Artist>,
    #[serde(default)]
    pub artists: Option<Vec<Artist>>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub audio_quality: Option<AudioQuality>,
    #[serde(default)]
    pub audio_modes: Option<Vec<AudioMode>>,
    #[serde(default)]
    pub media_metadata_tags: Option<Vec<String>>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub track_num: Option<u32>,
    #[serde(default)]
    pub volume_num: Option<u32>,
    #[serde(default)]
    pub listen_url: Option<String>,
    #[serde(default)]
    pub share_url: Option<String>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub bpm: Option<u32>,
    #[serde(default)]
    pub replay_gain: Option<f64>,
    #[serde(default)]
    pub peak: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    #[serde(default, deserialize_with = "deserialize_flexible_id")]
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub explicit: Option<bool>,
    #[serde(default)]
    pub artist: Option<Artist>,
    #[serde(default)]
    pub artists: Option<Vec<Artist>>,
    #[serde(default)]
    pub image_id: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub video_quality: Option<String>,
    #[serde(default)]
    pub share_url: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub num_tracks: Option<u32>,
    #[serde(default)]
    pub num_videos: Option<u32>,
    #[serde(default)]
    pub creator: Option<Creator>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub square_picture: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub public: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Creator {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub sub_title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub share_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Search response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    #[serde(default)]
    pub tracks: Option<PaginatedResponse<Track>>,
    #[serde(default)]
    pub albums: Option<PaginatedResponse<Album>>,
    #[serde(default)]
    pub artists: Option<PaginatedResponse<Artist>>,
    #[serde(default)]
    pub videos: Option<PaginatedResponse<Video>>,
    #[serde(default)]
    pub playlists: Option<PaginatedResponse<Playlist>>,
    #[serde(default)]
    pub top_hit: Option<TopHit>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    #[serde(default, alias = "totalNumRows")]
    pub total_number_of_items: Option<u64>,
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TopHit {
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(rename = "type", default)]
    pub media_type: String,
}

// ---------------------------------------------------------------------------
// API response wrappers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub user_id: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackInfoResponse {
    pub track_id: u64,
    #[serde(default)]
    pub audio_mode: Option<AudioMode>,
    #[serde(default)]
    pub audio_quality: Option<AudioQuality>,
    #[serde(default)]
    pub manifest_mime_type: Option<String>,
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub album_replay_gain: Option<f64>,
    #[serde(default)]
    pub album_peak_amplitude: Option<f64>,
    #[serde(default)]
    pub track_replay_gain: Option<f64>,
    #[serde(default)]
    pub track_peak_amplitude: Option<f64>,
    #[serde(default)]
    pub bit_depth: Option<u32>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VideoUrlResponse {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub track_id: u64,
    #[serde(default)]
    pub lyrics_provider: Option<String>,
    #[serde(default)]
    pub subtitles: Option<Vec<LyricsLine>>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub right_to_left: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LyricsLine {
    #[serde(default)]
    pub time: Option<u64>,
    #[serde(default)]
    pub line: Option<String>,
    #[serde(default)]
    pub lrc: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(rename = "verification_uri_complete", default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FavoriteResponse {
    #[serde(default)]
    pub items: Vec<FavoriteItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FavoriteItem {
    #[serde(default)]
    pub created: Option<String>,
    #[serde(flatten)]
    pub item: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Mix page response (pages/mix endpoint)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MixPageResponse {
    #[serde(default)]
    pub categories: Vec<MixPageCategory>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MixPageCategory {
    #[serde(rename = "type", default)]
    pub category_type: Option<String>,
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub paged_list: Option<MixPagedList>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MixPagedList {
    #[serde(default)]
    pub items: Vec<MixPageItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MixPageItem {
    #[serde(default)]
    pub item: Option<serde_json::Value>,
    #[serde(rename = "type", default)]
    pub item_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Helper impls
// ---------------------------------------------------------------------------

impl Track {
    /// Return a comma-separated list of artist names.
    pub fn artist_name(&self) -> String {
        if let Some(artists) = &self.artists
            && !artists.is_empty() {
                return artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
            }
        self.artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// Return the best available display title: `full_name` > `title` > `name`.
    pub fn title_display(&self) -> String {
        self.full_name
            .as_deref()
            .or(self.title.as_deref())
            .or(self.name.as_deref())
            .unwrap_or("")
            .to_string()
    }
}

impl Album {
    /// Return a comma-separated list of artists whose role is "MAIN".
    pub fn album_artist(&self) -> String {
        if let Some(artists) = &self.artists {
            let main: Vec<&str> = artists
                .iter()
                .filter(|a| a.role.as_deref() == Some("MAIN"))
                .map(|a| a.name.as_str())
                .collect();
            if !main.is_empty() {
                return main.join(", ");
            }
        }
        // Fallback to the single artist field.
        self.artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// Extract the year from `release_date` or `available_release_date`.
    /// Tidal dates are typically ISO-8601 (e.g. "2024-03-15").
    pub fn year_str(&self) -> Option<String> {
        let date_str = self
            .release_date
            .as_deref()
            .or(self.available_release_date.as_deref());

        date_str.map(|d| {
            // Take just the first 4 characters which are the year.
            if d.len() >= 4 {
                d[..4].to_string()
            } else {
                d.to_string()
            }
        })
    }

    /// Build a Tidal cover-art URL from the `cover` UUID.
    ///
    /// The `cover` value is a hex UUID like `"a1b2c3d4-e5f6-..."`.  Dashes must be
    /// preserved.  The `dimension` parameter should be e.g. `"320x320"`.
    ///
    /// Result: `https://resources.tidal.com/images/{id}/{dimension}.jpg`
    pub fn image_url(&self, dimension: &str) -> Option<String> {
        self.cover.as_ref().map(|id| {
            format!("https://resources.tidal.com/images/{id}/{dimension}.jpg")
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_track_minimal() {
        let json = r#"{"id": 12345}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, 12345);
        assert!(track.title.is_none());
    }

    #[test]
    fn deserialize_track_full() {
        let json = r#"{
            "id": 12345,
            "title": "Bohemian Rhapsody",
            "duration": 354,
            "explicit": false,
            "available": true,
            "streamReady": true,
            "artist": {"id": 1, "name": "Queen"},
            "artists": [{"id": 1, "name": "Queen"}],
            "audioQuality": "LOSSLESS",
            "audioModes": ["STEREO"],
            "trackNum": 1,
            "volumeNum": 1,
            "replayGain": -8.5,
            "peak": 0.98
        }"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, 12345);
        assert_eq!(track.title.as_deref(), Some("Bohemian Rhapsody"));
        assert_eq!(track.artist_name(), "Queen");
        assert_eq!(track.title_display(), "Bohemian Rhapsody");
    }

    #[test]
    fn deserialize_album_with_numeric_id() {
        let json = r#"{"id": 999, "name": "Greatest Hits"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.id, 999);
        assert_eq!(album.name, "Greatest Hits");
    }

    #[test]
    fn deserialize_album_with_string_id() {
        let json = r#"{"id": "12345", "name": "Greatest Hits"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.id, 12345);
        assert_eq!(album.name, "Greatest Hits");
    }

    #[test]
    fn album_artist_filters_main() {
        let json = r#"{
            "id": 1,
            "name": "Test Album",
            "artists": [
                {"id": 1, "name": "Queen", "role": "MAIN"},
                {"id": 2, "name": "Foo", "role": "FEATURED"}
            ]
        }"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.album_artist(), "Queen");
    }

    #[test]
    fn album_year_from_release_date() {
        let json = r#"{"id": 1, "name": "X", "releaseDate": "2024-03-15"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.year_str(), Some("2024".to_string()));
    }

    #[test]
    fn album_year_from_available_release_date() {
        let json = r#"{"id": 1, "name": "X", "availableReleaseDate": "2023-12-01"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.year_str(), Some("2023".to_string()));
    }

    #[test]
    fn album_image_url() {
        let json = r#"{"id": 1, "name": "X", "cover": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        let url = album.image_url("320x320").unwrap();
        assert_eq!(
            url,
            "https://resources.tidal.com/images/a1b2c3d4-e5f6-7890-abcd-ef1234567890/320x320.jpg"
        );
    }

    #[test]
    fn deserialize_search_response() {
        let json = r#"{
            "tracks": {
                "totalNumRows": 1,
                "items": [{"id": 1, "title": "Song"}],
                "limit": 10,
                "offset": 0
            },
            "topHit": {
                "value": {"id": 1},
                "type": "TRACK"
            }
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(resp.tracks.is_some());
        assert!(resp.albums.is_none());
        let tracks = resp.tracks.unwrap();
        assert_eq!(tracks.items.len(), 1);
    }

    #[test]
    fn deserialize_device_auth_response() {
        let json = r#"{
            "deviceCode": "dc123",
            "userCode": "AB12CD34",
            "verificationUri": "https://link.tidal.com",
            "verification_uri_complete": "https://link.tidal.com/AB12CD34",
            "expiresIn": 300,
            "interval": 5
        }"#;
        let auth: DeviceAuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(auth.device_code, "dc123");
        assert_eq!(auth.expires_in, 300);
        assert_eq!(
            auth.verification_uri_complete.as_deref(),
            Some("https://link.tidal.com/AB12CD34")
        );
    }

    #[test]
    fn deserialize_device_auth_response_without_complete_uri() {
        let json = r#"{
            "deviceCode": "dc456",
            "userCode": "XY98ZW76",
            "verificationUri": "https://link.tidal.com",
            "expiresIn": 300,
            "interval": 5
        }"#;
        let auth: DeviceAuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(auth.device_code, "dc456");
        assert!(auth.verification_uri_complete.is_none());
    }

    #[test]
    fn deserialize_token_response() {
        let json = r#"{
            "access_token": "at_abc",
            "refresh_token": "rt_xyz",
            "token_type": "Bearer",
            "expires_in": 3600
        }"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token.as_deref(), Some("at_abc"));
    }

    #[test]
    fn deserialize_token_error() {
        let json = r#"{
            "error": "invalid_grant",
            "error_description": "Authorization code expired"
        }"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.error.as_deref(), Some("invalid_grant"));
        assert!(token.access_token.is_none());
    }

    #[test]
    fn deserialize_playback_info() {
        let json = r#"{
            "trackId": 12345,
            "audioMode": "STEREO",
            "audioQuality": "LOSSLESS",
            "manifestMimeType": "application/vnd.tidal.bt",
            "manifest": "base64encoded==",
            "bitDepth": 16,
            "sampleRate": 44100
        }"#;
        let info: PlaybackInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(info.track_id, 12345);
        assert_eq!(info.bit_depth, Some(16));
        assert_eq!(info.sample_rate, Some(44100));
    }

    #[test]
    fn deserialize_lyrics() {
        let json = r#"{
            "trackId": 1,
            "subtitles": [
                {"time": 0, "line": "First line"},
                {"time": 5000, "line": "Second line"}
            ]
        }"#;
        let lyrics: LyricsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(lyrics.subtitles.unwrap().len(), 2);
    }

    #[test]
    fn deserialize_favorite_response() {
        let json = r#"{
            "items": [
                {"created": "2024-01-01", "id": 1, "title": "Song"}
            ]
        }"#;
        let fav: FavoriteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(fav.items.len(), 1);
    }

    #[test]
    fn track_artist_name_falls_back_to_single_artist() {
        let json = r#"{"id": 1, "artist": {"id": 10, "name": "Solo"}}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert_eq!(track.artist_name(), "Solo");
    }

    #[test]
    fn track_title_display_prefers_full_name() {
        let json = r#"{"id": 1, "fullName": "Full (Deluxe)", "title": "Full", "name": "N"}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert_eq!(track.title_display(), "Full (Deluxe)");
    }

    #[test]
    fn deserialize_audio_quality() {
        let json = r#""HI_RES_LOSSLESS""#;
        let q: AudioQuality = serde_json::from_str(json).unwrap();
        assert_eq!(q, AudioQuality::HI_RES_LOSSLESS);
    }

    #[test]
    fn deserialize_media_type() {
        let json = r#""TRACK""#;
        let mt: MediaType = serde_json::from_str(json).unwrap();
        assert_eq!(mt, MediaType::Track);
    }

    #[test]
    fn deserialize_playlist() {
        let json = r#"{
            "uuid": "pl-abc123",
            "name": "My Playlist",
            "numTracks": 42,
            "creator": {"id": 100, "name": "User1"},
            "public": true
        }"#;
        let pl: Playlist = serde_json::from_str(json).unwrap();
        assert_eq!(pl.name.as_deref(), Some("My Playlist"));
        assert_eq!(pl.num_tracks, Some(42));
    }

    #[test]
    fn deserialize_mix() {
        let json = r#"{
            "id": "mix-001",
            "title": "Daily Mix 1",
            "subTitle": "Based on your listening"
        }"#;
        let mix: Mix = serde_json::from_str(json).unwrap();
        assert_eq!(mix.id, "mix-001");
    }

    #[test]
    fn deserialize_video() {
        let json = r#"{
            "id": 555,
            "title": "Live Concert",
            "duration": 7200,
            "imageId": "img123"
        }"#;
        let vid: Video = serde_json::from_str(json).unwrap();
        assert_eq!(vid.id, 555);
        assert_eq!(vid.duration, Some(7200));
    }

    #[test]
    fn album_populate_bool() {
        let json = r#"{"id": 1, "name": "X", "populate": true}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.populate, Some(true));
    }

    #[test]
    fn album_populate_missing() {
        let json = r#"{"id": 1, "name": "X"}"#;
        let album: Album = serde_json::from_str(json).unwrap();
        assert_eq!(album.populate, None);
    }
}
