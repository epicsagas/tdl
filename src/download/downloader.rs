use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rand::RngExt;
use tokio::sync::Mutex;

use crate::config::settings::{CoverDimensions, Settings};
use crate::download::decrypt;
use crate::download::segment;
use crate::download::video;
use crate::metadata::writer::{AudioMetadata, write_metadata};
use crate::pathfmt::format::{
    MediaInfo, check_file_exists, extension_guess, file_unique_suffix, format_path_media,
    get_format_template,
};
use crate::tidal::media::{MediaType, Track};
use crate::tidal::search;
use crate::tidal::session::TidalSession;
use crate::tidal::stream;

// ---------------------------------------------------------------------------
// Downloader
// ---------------------------------------------------------------------------

pub struct Downloader {
    session: Arc<Mutex<TidalSession>>,
    settings: Settings,
    http_client: reqwest::Client,
}

impl Downloader {
    /// Create a new downloader backed by the given shared session.
    pub fn new(session: Arc<Mutex<TidalSession>>, settings: Settings) -> Self {
        let http_client = reqwest::Client::new();
        Self {
            session,
            settings,
            http_client,
        }
    }

    /// Borrow the shared session handle (for callers that need to make
    /// additional API requests).
    pub fn session(&self) -> &Arc<Mutex<TidalSession>> {
        &self.session
    }

    // -----------------------------------------------------------------------
    // Top-level entry point
    // -----------------------------------------------------------------------

    /// Download media identified by a Tidal URL.
    ///
    /// Parses the URL, determines the media type, and dispatches to either
    /// [`download_item`] (for single tracks/videos) or [`download_collection`]
    /// (for albums, playlists, mixes, artists).
    pub async fn download_url(&self, url: &str) -> Result<()> {
        let (media_type, id) = search::parse_media_url(url)?;

        match media_type {
            MediaType::Track | MediaType::Video => {
                self.download_item(media_type, &id).await
            }
            MediaType::Album | MediaType::Playlist | MediaType::Mix => {
                self.download_collection(media_type, &id).await
            }
            MediaType::Artist => {
                let albums = {
                    let sess = self.session.lock().await;
                    let artist_id: u64 = id.parse().context("Invalid artist ID")?;
                    search::get_artist_albums(&sess.request, artist_id).await?
                };
                for album in &albums {
                    if let Err(e) = self
                        .download_collection(MediaType::Album, &album.id.to_string())
                        .await
                    {
                        eprintln!(
                            "Warning: failed to download album '{}' ({}): {e}",
                            album.name, album.id
                        );
                    }
                }
                Ok(())
            }
        }
    }

    // -----------------------------------------------------------------------
    // Single-item download
    // -----------------------------------------------------------------------

    /// Download a single track or video.
    ///
    /// This is the core download pipeline:
    /// 1. Fetch metadata
    /// 2. Compute the destination path
    /// 3. Skip if the file already exists
    /// 4. Fetch the stream manifest
    /// 5. Download and merge segments
    /// 6. Decrypt if necessary
    /// 7. Convert TS -> MP4 for videos (optional)
    /// 8. Extract FLAC from MP4 (optional)
    /// 9. Write metadata tags
    /// 10. Save cover art and lyrics files
    /// 11. Apply download delay
    pub async fn download_item(&self, media_type: MediaType, id: &str) -> Result<()> {
        let numeric_id: u64 = id.parse().context("Invalid media ID")?;

        // Determine if this is a video early so we can branch correctly.
        let is_video = media_type == MediaType::Video;

        // --- 1. Fetch track metadata ---------------------------------------
        let track = self.fetch_track_metadata(media_type.clone(), numeric_id).await?;

        let title = track.title_display();
        let artist = track.artist_name();
        println!("Downloading: {artist} - {title}");

        // --- 2. Build destination path -------------------------------------
        let type_key = match media_type {
            MediaType::Video => "video",
            MediaType::Track => "track",
            MediaType::Album => "album",
            MediaType::Playlist => "playlist",
            MediaType::Mix => "mix",
            MediaType::Artist => "track",
        };
        let template = get_format_template(type_key, &self.settings);

        let media_info = self.build_media_info(&track);
        let relative = format_path_media(
            template,
            &media_info,
            self.settings.album_track_num_pad_min,
        );

        let ext = extension_guess(
            is_video,
            self.settings.video_convert_mp4,
            self.settings.extract_flac,
            &self.settings.quality_audio,
            None,
            track.media_metadata_tags.as_deref().unwrap_or(&Vec::new()),
        );

        let base = expand_tilde(&self.settings.download_base_path);
        let dest_path = PathBuf::from(&base)
            .join(relative)
            .with_extension(ext.trim_start_matches('.'));

        // Ensure the parent directory exists.
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // --- 3. Check if file already exists --------------------------------
        let audio_extensions = ["flac", "m4a", "mp3", "mp4", "ts"];
        if self.settings.skip_existing
            && let Some(_existing) = check_file_exists(&dest_path, &audio_extensions) {
                println!("Skipping (already exists): {}", dest_path.display());
                self.apply_download_delay().await;
                return Ok(());
            }

        // Use a unique suffix to avoid collisions when not skipping.
        let final_path = file_unique_suffix(&dest_path);

        // --- 4. Fetch stream / download URL --------------------------------
        let (manifest, _playback_info) = {
            let sess = self.session.lock().await;
            stream::fetch_track_stream(&sess.request, track.id, &self.settings.quality_audio)
                .await?
        };

        // --- 5. Download segments into a temp file -------------------------
        let temp_dir = final_path
            .parent()
            .ok_or_else(|| anyhow!("Destination path has no parent"))?;
        let temp_file_name = format!(
            ".~{}.part",
            final_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let temp_path = temp_dir.join(&temp_file_name);

        let pb = self.create_progress_bar(manifest.urls.len() as u64, &title);

        segment::download_and_merge(
            &manifest.urls,
            &temp_path,
            &self.http_client,
            self.settings.downloads_simultaneous_per_track_max,
            Some(&pb),
        )
        .await?;

        pb.finish_and_clear();

        // Debug: check temp file
        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
            eprintln!("Debug: temp file size = {} bytes", meta.len());
        } else {
            eprintln!("Debug: temp file not found at {}", temp_path.display());
        }

        // --- 6. Decrypt if encrypted ---------------------------------------
        if manifest.is_encrypted
            && let Some(ref key_id) = manifest.encryption_key {
                let (key, nonce) = decrypt::decrypt_security_token(key_id)
                    .context("Failed to decrypt security token")?;

                let encrypted_data = tokio::fs::read(&temp_path).await?;
                let decrypted = decrypt::decrypt_file(&encrypted_data, &key, &nonce);
                tokio::fs::write(&temp_path, &decrypted).await?;
            }

        // Track the current working file -- may change during video conversion
        // or FLAC extraction.
        let mut working_path = temp_path.clone();

        // --- 7. Video: convert TS -> MP4 -----------------------------------
        if is_video
            && self.settings.video_convert_mp4
            && video::ffmpeg_available(&self.settings.path_binary_ffmpeg)
        {
            let mp4_path = working_path.with_extension("mp4");
            video::convert_ts_to_mp4(
                &working_path,
                &mp4_path,
                &self.settings.path_binary_ffmpeg,
            )?;
            let _ = tokio::fs::remove_file(&working_path).await;
            working_path = mp4_path;
        }

        // --- 8. Extract FLAC from MP4 if applicable -----------------------
        if self.settings.extract_flac
            && !is_video
            && working_path
                .extension()
                .is_some_and(|e| e == "mp4" || e == "m4a")
        {
            // Check if the codec is FLAC inside MP4.
            let codec_is_flac = manifest
                .codecs
                .as_deref()
                .is_some_and(|c| c.to_ascii_lowercase().contains("flac"));

            if codec_is_flac && video::ffmpeg_available(&self.settings.path_binary_ffmpeg) {
                let flac_path = final_path.with_extension("flac");
                video::extract_flac(
                    &working_path,
                    &flac_path,
                    &self.settings.path_binary_ffmpeg,
                )?;
                let _ = tokio::fs::remove_file(&working_path).await;
                working_path = flac_path;
            }
        }

        // --- 9. Write metadata tags ----------------------------------------
        if !is_video {
            let meta = self.build_audio_metadata(&track);
            // Best-effort: do not fail the download if tagging fails.
            if let Err(e) = write_metadata(&working_path, &meta) {
                eprintln!("Warning: failed to write metadata: {e}");
            }
        }

        // --- 10. Move to final destination ---------------------------------
        eprintln!("Debug: working_path = {}", working_path.display());
        eprintln!("Debug: final_path  = {}", final_path.display());
        if working_path != final_path {
            tokio::fs::rename(&working_path, &final_path)
                .await
                .map_err(|e| {
                    anyhow!(
                        "Failed to move file to {}: {e}",
                        final_path.display()
                    )
                })?;
        } else {
            // The temp file may still have the .part extension.
            if working_path.extension().is_some_and(|e| e == "part") {
                let renamed = working_path.with_extension("");
                tokio::fs::rename(&working_path, &renamed).await?;
            }
        }

        println!("Saved: {}", final_path.display());

        // --- 11. Save cover art and lyrics ---------------------------------
        let dest_dir = final_path.parent();

        if self.settings.cover_album_file
            && let Some(dir) = dest_dir
                && let Some(cover_url) = self.track_cover_url(&track) {
                    let cover_path = dir.join("cover.jpg");
                    if !cover_path.exists()
                        && let Err(e) = self.download_cover(&cover_url, dir).await {
                            eprintln!("Warning: failed to download cover: {e}");
                        }
                }

        if self.settings.lyrics_file
            && let Some(dir) = dest_dir {
                let lrc_name = format!(
                    "{}.lrc",
                    final_path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                let lrc_path = dir.join(&lrc_name);
                if !lrc_path.exists()
                    && let Err(e) = self.save_lyrics(track.id, dir).await {
                        eprintln!("Warning: failed to save lyrics: {e}");
                    }
            }

        // --- 12. Apply download delay --------------------------------------
        self.apply_download_delay().await;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Collection download
    // -----------------------------------------------------------------------

    /// Download all items in a collection (album, playlist, or mix).
    ///
    /// Fetches the list of tracks and downloads each one.  If
    /// `settings.playlist_create` is enabled, also writes an `.m3u` playlist
    /// file.
    pub async fn download_collection(&self, media_type: MediaType, id: &str) -> Result<()> {
        let tracks = {
            let sess = self.session.lock().await;
            match media_type {
                MediaType::Album => {
                    let album_id: u64 = id.parse().context("Invalid album ID")?;
                    search::get_album_tracks(&sess.request, album_id).await?
                }
                MediaType::Playlist => {
                    search::get_playlist_tracks(&sess.request, id).await?
                }
                MediaType::Mix => {
                    search::get_mix_items(&sess.request, id).await?
                }
                _ => {
                    return Err(anyhow!(
                        "download_collection called with unsupported media type: {:?}",
                        media_type
                    ));
                }
            }
        };

        let total = tracks.len();
        println!("Downloading {total} tracks...");

        for (i, track) in tracks.iter().enumerate() {
            println!("[{}/{total}]", i + 1);
            if let Err(e) = self
                .download_item(MediaType::Track, &track.id.to_string())
                .await
            {
                eprintln!(
                    "Warning: failed to download track {} ({}): {e}",
                    track.id,
                    track.title_display()
                );
            }
        }

        // Create an M3U playlist file if configured.
        if self.settings.playlist_create
            && let Some(m3u_name) = self.collection_name(&media_type, id).await {
                let base = expand_tilde(&self.settings.download_base_path);
                let m3u_path =
                    PathBuf::from(&base).join(&m3u_name).with_extension("m3u");
                if let Some(parent) = m3u_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                // Write an empty M3U header; the actual file paths are created
                // per-track inside download_item, so we just ensure the M3U
                // file exists as a marker.
                let m3u_content = "#EXTM3U\n";
                let _ = tokio::fs::write(&m3u_path, m3u_content).await;
            }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a [`MediaInfo`] from a track's metadata for path formatting.
    fn build_media_info(&self, track: &Track) -> MediaInfo {
        MediaInfo {
            artist_name: Some(track.artist_name()),
            album_artist: track
                .album
                .as_ref()
                .and_then(|a| a.artist.as_ref().map(|ar| ar.name.clone())),
            track_title: Some(track.title_display()),
            album_title: track.album.as_ref().map(|a| a.name.clone()),
            album_track_num: track.track_num,
            album_num_tracks: track.album.as_ref().and_then(|a| a.num_tracks),
            track_id: Some(track.id),
            album_id: track.album.as_ref().map(|a| a.id),
            track_duration_seconds: track.duration,
            album_year: track.album.as_ref().and_then(|a| a.year_str()),
            track_quality: track.audio_quality.as_ref().map(|q| format!("{q:?}")),
            track_explicit: track.explicit,
            album_explicit: track.album.as_ref().and_then(|a| a.explicit),
            album_num_volumes: track.album.as_ref().and_then(|a| a.num_volumes),
            track_volume_num: track.volume_num,
            isrc: track.isrc.clone(),
            ..Default::default()
        }
    }

    /// Build [`AudioMetadata`] for tag writing.
    fn build_audio_metadata(&self, track: &Track) -> AudioMetadata {
        AudioMetadata {
            title: Some(track.title_display()),
            album: track.album.as_ref().map(|a| a.name.clone()),
            album_artist: track.album.as_ref().map(|a| a.album_artist()),
            artists: Some(
                track
                    .artist_name()
                    .split(", ")
                    .map(String::from)
                    .collect(),
            ),
            copyright: track.copyright.clone(),
            track_number: track.track_num,
            total_tracks: track.album.as_ref().and_then(|a| a.num_tracks),
            disc_number: track.volume_num,
            total_discs: track.album.as_ref().and_then(|a| a.num_volumes),
            date: track
                .album
                .as_ref()
                .and_then(|a| a.release_date.clone()),
            isrc: track.isrc.clone(),
            url: track.share_url.clone(),
            lyrics: None,
            cover_data: None,
            write_replay_gain: self.settings.metadata_replay_gain,
            ..Default::default()
        }
    }

    /// Download cover art to a directory as `cover.jpg`.
    async fn download_cover(&self, cover_url: &str, dest_dir: &Path) -> Result<()> {
        let response = self.http_client.get(cover_url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download cover: HTTP {}",
                response.status()
            ));
        }
        let bytes = response.bytes().await?;
        let cover_path = dest_dir.join("cover.jpg");
        tokio::fs::write(&cover_path, &bytes).await?;
        Ok(())
    }

    /// Fetch and save timed lyrics as an `.lrc` file.
    async fn save_lyrics(&self, track_id: u64, dest_dir: &Path) -> Result<()> {
        let lyrics = {
            let sess = self.session.lock().await;
            search::get_lyrics(&sess.request, track_id).await?
        };

        let lrc_filename = format!("{}.lrc", track_id);
        let lrc_path = dest_dir.join(&lrc_filename);

        let mut content = String::new();

        // Prefer the timed subtitles (LRC format).
        if let Some(subtitles) = &lyrics.subtitles {
            for line in subtitles {
                if let (Some(time_ms), Some(text)) = (&line.time, &line.lrc) {
                    // LRC timestamps are [mm:ss.xx]
                    let total_secs = *time_ms as f64 / 1000.0;
                    let mins = (total_secs / 60.0) as u32;
                    let secs = total_secs - (mins as f64 * 60.0);
                    content.push_str(&format!("[{mins:02}:{secs:05.2}]{text}\n"));
                }
            }
        } else if let Some(text) = &lyrics.text {
            content = text.clone();
        }

        if !content.is_empty() {
            tokio::fs::write(&lrc_path, &content).await?;
        }

        Ok(())
    }

    /// Resolve the cover-art URL for a track (from its album cover UUID).
    fn track_cover_url(&self, track: &Track) -> Option<String> {
        let album = track.album.as_ref()?;
        let cover_uuid = album.cover.as_ref()?;
        let dimension = self.cover_dimension_string();
        Some(format!(
            "https://resources.tidal.com/images/{cover_uuid}/{dimension}.jpg"
        ))
    }

    /// Map [`CoverDimensions`] to the URL dimension string.
    fn cover_dimension_string(&self) -> &'static str {
        match self.settings.metadata_cover_dimension {
            CoverDimensions::Px80 => "80x80",
            CoverDimensions::Px160 => "160x160",
            CoverDimensions::Px320 => "320x320",
            CoverDimensions::Px640 => "640x640",
            CoverDimensions::Px1280 => "1280x1280",
        }
    }

    /// Fetch the track metadata, handling the Track vs Video distinction.
    async fn fetch_track_metadata(
        &self,
        media_type: MediaType,
        id: u64,
    ) -> Result<Track> {
        let sess = self.session.lock().await;
        match media_type {
            MediaType::Track => search::get_track(&sess.request, id).await,
            // For videos, try the track endpoint first (Tidal sometimes
            // returns video data there), then fall back to the video endpoint.
            MediaType::Video => {
                match search::get_track(&sess.request, id).await {
                    Ok(t) => Ok(t),
                    Err(_) => {
                        let path = format!("videos/{id}");
                        let vid: crate::tidal::media::Video =
                            sess.request.get(&path, None).await?;
                        Ok(Track {
                            id: vid.id,
                            title: vid.title.clone(),
                            name: vid.name.clone(),
                            duration: vid.duration,
                            explicit: vid.explicit,
                            available: None,
                            stream_ready: None,
                            artist: vid.artist.clone(),
                            artists: vid.artists.clone(),
                            album: None,
                            audio_quality: None,
                            audio_modes: None,
                            media_metadata_tags: None,
                            isrc: None,
                            copyright: None,
                            version: None,
                            track_num: None,
                            volume_num: None,
                            listen_url: None,
                            share_url: None,
                            full_name: None,
                            bpm: None,
                            replay_gain: None,
                            peak: None,
                        })
                    }
                }
            }
            // For other media types (album track lists already resolved to
            // individual tracks), use the track endpoint directly.
            _ => search::get_track(&sess.request, id).await,
        }
    }

    /// Return a display name for a collection (used for M3U placement).
    async fn collection_name(
        &self,
        media_type: &MediaType,
        id: &str,
    ) -> Option<String> {
        let sess = self.session.lock().await;
        match media_type {
            MediaType::Album => {
                let album_id: u64 = id.parse().ok()?;
                let album = search::get_album(&sess.request, album_id).await.ok()?;
                Some(album.name)
            }
            MediaType::Playlist => {
                let path = format!("playlists/{id}");
                let pl: crate::tidal::media::Playlist =
                    sess.request.get(&path, None).await.ok()?;
                pl.name
            }
            MediaType::Mix => {
                let path = format!("mixes/{id}");
                let mix: crate::tidal::media::Mix =
                    sess.request.get(&path, None).await.ok()?;
                mix.title.or(mix.name)
            }
            _ => None,
        }
    }

    /// Apply a random download delay between `download_delay_sec_min` and
    /// `download_delay_sec_max` if delays are enabled.
    async fn apply_download_delay(&self) {
        if !self.settings.download_delay {
            return;
        }
        let min = self.settings.download_delay_sec_min;
        let max = self.settings.download_delay_sec_max;
        if max <= min {
            return;
        }
        let delay_secs = rand::rng().random_range(min..max);
        tokio::time::sleep(tokio::time::Duration::from_secs_f64(delay_secs)).await;
    }

    /// Create a progress bar for segment downloads.
    fn create_progress_bar(&self, total: u64, title: &str) -> ProgressBar {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("#>-"),
        );
        pb.set_message(title.to_string());
        pb
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Expand a leading `~` in a path to the user's home directory.
///
/// If the path does not start with `~`, it is returned unchanged.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        // Handle "~/..." and "~" (bare tilde).
        if (rest.is_empty() || rest.starts_with('/'))
            && let Some(home) = dirs::home_dir() {
                return format!("{}{}", home.display(), rest);
            }
    }
    path.to_string()
}
