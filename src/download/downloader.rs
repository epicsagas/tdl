use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::RngExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::config::settings::{CoverDimensions, Settings};
use crate::download::decrypt;
use crate::download::segment;
use crate::download::video;
use crate::metadata::writer::{AudioMetadata, write_metadata};
use crate::pathfmt::format::{
    MediaInfo, build_track_path, check_file_exists, extension_guess, file_unique_suffix,
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
    pub cancel: CancellationToken,
}

impl Downloader {
    /// Create a new downloader backed by the given shared session.
    pub fn new(session: Arc<Mutex<TidalSession>>, settings: Settings) -> Self {
        Self::with_cancel(session, settings, CancellationToken::new())
    }

    pub fn with_cancel(session: Arc<Mutex<TidalSession>>, settings: Settings, cancel: CancellationToken) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Linux; Android 12; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/91.0.4472.114 Safari/537.36")
            .build()
            .unwrap_or_default();
        Self {
            session,
            settings,
            http_client,
            cancel,
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
        info!(media_type = ?media_type, id = %id, "Starting download");

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
                info!(album_count = albums.len(), "Artist albums fetched");
                for album in &albums {
                    if let Err(e) = self
                        .download_collection(MediaType::Album, &album.id.to_string())
                        .await
                    {
                        warn!(album_id = album.id, album = %album.name, "Failed to download album: {e}");
                        println!(
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
        self.download_item_with_context(media_type, id, None, None, None).await
    }

    async fn download_item_with_context(
        &self,
        media_type: MediaType,
        id: &str,
        playlist_name: Option<&str>,
        mix_name: Option<&str>,
        track_pb: Option<&ProgressBar>,
    ) -> Result<()> {
        let numeric_id: u64 = id.parse().context("Invalid media ID")?;

        // Determine if this is a video early so we can branch correctly.
        let is_video = media_type == MediaType::Video;

        // --- 1. Fetch track metadata ---------------------------------------
        let track = self.fetch_track_metadata(media_type.clone(), numeric_id).await?;

        let title = track.title_display();
        let artist = track.artist_name();
        info!(track_id = numeric_id, artist = %artist, title = %title, "Track metadata fetched");
        if track_pb.is_none() {
            println!("Downloading: {artist} - {title}");
        }

        // --- 2. Build destination path -------------------------------------
        let mut media_info = self.build_media_info(&track);
        if let Some(name) = playlist_name {
            media_info.playlist_name = Some(name.to_string());
        }
        if let Some(name) = mix_name {
            media_info.mix_name = Some(name.to_string());
        }
        let relative = build_track_path(&media_info, self.settings.track_num_pad_zero);

        let base = expand_tilde(&self.settings.download_base_path);

        // --- 3. Check if file already exists (pre-manifest, multi-ext scan) -
        // Use a stem path without extension; check_file_exists tries all audio exts.
        let stem_path = PathBuf::from(format!("{}/{}", base, relative));
        let audio_extensions = ["flac", "m4a", "mp3", "mp4", "ts"];
        if self.settings.skip_existing
            && let Some(existing) = check_file_exists(&stem_path, &audio_extensions) {
                debug!(path = %existing.display(), "Skipping existing file");
                if let Some(pb) = track_pb {
                    pb.set_message(format!("⏭  {artist} - {title}"));
                    pb.finish();
                } else {
                    println!("Skipping (already exists): {}", existing.display());
                }
                self.apply_download_delay().await;
                return Ok(());
            }

        // --- 4. Fetch stream / download URL --------------------------------
        if is_video {
            let ext = extension_guess(
                true,
                self.settings.video_convert_mp4,
                self.settings.extract_flac,
                &self.settings.quality_audio,
                None,
                track.media_metadata_tags.as_deref().unwrap_or(&Vec::new()),
            );
            let dest_path = PathBuf::from(format!("{}/{}{}", base, relative, ext));
            let final_path = file_unique_suffix(&dest_path);
            if let Some(parent) = final_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            return self.download_video(numeric_id, &track, &final_path).await;
        }

        let (manifest, _playback_info) = {
            let sess = self.session.lock().await;
            stream::fetch_track_stream(&sess.request, track.id, &self.settings.quality_audio)
                .await?
        };
        debug!(
            codec = manifest.codecs.as_deref().unwrap_or("?"),
            segments = manifest.urls.len(),
            encrypted = manifest.is_encrypted,
            "Stream manifest fetched"
        );

        // Now we know the actual codec — pick the correct extension.
        let ext = extension_guess(
            false,
            self.settings.video_convert_mp4,
            self.settings.extract_flac,
            &self.settings.quality_audio,
            manifest.codecs.as_deref(),
            track.media_metadata_tags.as_deref().unwrap_or(&Vec::new()),
        );
        // Append extension directly — do NOT use `.with_extension()` because the
        // track title may contain a dot, which would be misread as a file extension.
        let dest_path = PathBuf::from(format!("{}/{}{}", base, relative, ext));
        debug!(dest = %dest_path.display(), "Destination path resolved");
        println!("  Destination: {}", dest_path.display());
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let final_path = file_unique_suffix(&dest_path);

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

        let seg_total = manifest.urls.len() as u64;
        let owned_pb;
        let pb: &ProgressBar = if let Some(pb) = track_pb {
            pb.set_length(seg_total);
            pb.set_message(format!("↓  {artist} - {title}"));
            pb.set_position(0);
            pb
        } else {
            owned_pb = self.create_progress_bar(seg_total, &title);
            &owned_pb
        };

        segment::download_and_merge(
            &manifest.urls,
            &temp_path,
            &self.http_client,
            self.settings.downloads_simultaneous_per_track_max,
            Some(pb),
        )
        .await?;

        if track_pb.is_none() {
            pb.finish_and_clear();
        }

        // --- 6. Decrypt if encrypted ---------------------------------------
        if manifest.is_encrypted
            && let Some(ref key_id) = manifest.encryption_key {
                debug!("Decrypting encrypted stream");
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

        // --- 8. Extract FLAC from MP4 container if applicable --------------
        // Use manifest codec to detect FLAC-in-MP4 regardless of the temp file
        // extension (which is always ".part" at this stage).
        let codec_is_flac = manifest
            .codecs
            .as_deref()
            .is_some_and(|c| c.to_ascii_lowercase().contains("flac"));

        if self.settings.extract_flac && !is_video && codec_is_flac
            && video::ffmpeg_available(&self.settings.path_binary_ffmpeg)
        {
            let flac_path = final_path.with_extension("flac");
            video::extract_flac(
                &working_path,
                &flac_path,
                &self.settings.path_binary_ffmpeg,
            )?;
            let _ = tokio::fs::remove_file(&working_path).await;
            working_path = flac_path;
        }

        // --- 9. Move to final destination ---------------------------------
        if working_path != final_path {
            tokio::fs::rename(&working_path, &final_path)
                .await
                .map_err(|e| {
                    anyhow!(
                        "Failed to move file to {}: {e}",
                        final_path.display()
                    )
                })?;
        } else if working_path.extension().is_some_and(|e| e == "part") {
            let renamed = working_path.with_extension("");
            tokio::fs::rename(&working_path, &renamed).await?;
        }

        let dest_dir = final_path.parent();

        // --- 10. Cover: read existing cover.jpg or fetch once ---------------
        // cover.jpg is album-scoped; reuse it across tracks in the same folder.
        let cover_bytes: Option<Vec<u8>> = if self.settings.metadata_cover_embed
            || self.settings.cover_album_file
        {
            let cover_path = dest_dir.map(|d| d.join("cover.jpg"));
            if let Some(ref p) = cover_path
                && p.exists()
            {
                tokio::fs::read(p).await.ok()
            } else {
                let bytes = self.fetch_cover_bytes(&track).await;
                if self.settings.cover_album_file
                    && let (Some(p), Some(b)) = (&cover_path, &bytes) {
                        let _ = tokio::fs::write(p, b).await;
                    }
                bytes
            }
        } else {
            None
        };

        // --- 11. Lyrics: fetch once for embed + file -----------------------
        let lyrics_text: Option<String> =
            if !is_video && (self.settings.lyrics_embed || self.settings.lyrics_file) {
                self.fetch_lyrics_text(track.id).await
            } else {
                None
            };

        // --- 12. Write metadata tags ----------------------------------------
        if !is_video {
            let meta = self.build_audio_metadata(
                &track,
                if self.settings.metadata_cover_embed { cover_bytes } else { None },
                if self.settings.lyrics_embed { lyrics_text.clone() } else { None },
            );
            if let Err(e) = write_metadata(&final_path, &meta) {
                warn!(path = %final_path.display(), "Failed to write metadata: {e}");
                println!("Warning: failed to write metadata: {e}");
            }
        }

        info!(track_id = numeric_id, path = %final_path.display(), "Track saved");
        if let Some(pb) = track_pb {
            pb.set_message(format!("✓  {artist} - {title}"));
            pb.finish();
        } else {
            println!("Saved: {}", final_path.display());
        }

        // --- 13. Save .lrc file --------------------------------------------
        if self.settings.lyrics_file
            && let Some(dir) = dest_dir
            && let Some(text) = &lyrics_text
            && !text.is_empty()
        {
            let lrc_path = dir.join(format!(
                "{}.lrc",
                final_path.file_stem().unwrap_or_default().to_string_lossy()
            ));
            if !lrc_path.exists()
                && let Err(e) = tokio::fs::write(&lrc_path, text).await {
                    println!("Warning: failed to save lyrics: {e}");
                }
        }

        // --- 12. Apply download delay --------------------------------------
        self.apply_download_delay().await;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Video download
    // -----------------------------------------------------------------------

    /// Download a video by fetching its m3u8 URL and downloading segments.
    async fn download_video(
        &self,
        video_id: u64,
        track: &Track,
        final_path: &Path,
    ) -> Result<()> {
        // Fetch the m3u8 master URL from the video endpoint.
        let m3u8_url = {
            let sess = self.session.lock().await;
            stream::fetch_video_url(&sess.request, video_id, &self.settings.quality_video).await?
        };

        // Fetch and parse the master playlist to get the media playlist URL.
        let master_body = self.http_client.get(&m3u8_url).send().await?.text().await?;
        let variant_urls = video::parse_m3u8(&master_body)?;

        // If we got a media playlist URL, fetch and parse it for segment URLs.
        let segments = if variant_urls.len() == 1 && !master_body.contains("#EXTINF") {
            // Resolve relative URI against the master playlist URL.
            let media_url = url::Url::parse(&m3u8_url)
                .and_then(|base| base.join(&variant_urls[0]))
                .map(|u| u.to_string())
                .unwrap_or_else(|_| variant_urls[0].clone());

            let media_body = self.http_client.get(&media_url).send().await?.text().await?;
            video::parse_m3u8(&media_body)?
        } else {
            variant_urls
        };

        // Download segments into a temp file.
        let temp_dir = final_path
            .parent()
            .ok_or_else(|| anyhow!("Destination path has no parent"))?;
        let temp_file_name = format!(
            ".~{}.part",
            final_path.file_name().unwrap_or_default().to_string_lossy()
        );
        let temp_path = temp_dir.join(&temp_file_name);

        let pb = self.create_progress_bar(segments.len() as u64, &track.title_display());

        segment::download_and_merge(
            &segments,
            &temp_path,
            &self.http_client,
            self.settings.downloads_simultaneous_per_track_max,
            Some(&pb),
        )
        .await?;

        pb.finish_and_clear();

        let mut working_path = temp_path;

        // Convert TS -> MP4 if ffmpeg is available.
        if self.settings.video_convert_mp4
            && video::ffmpeg_available(&self.settings.path_binary_ffmpeg)
        {
            let mp4_path = working_path.with_extension("mp4");
            video::convert_ts_to_mp4(&working_path, &mp4_path, &self.settings.path_binary_ffmpeg)?;
            let _ = tokio::fs::remove_file(&working_path).await;
            working_path = mp4_path;
        }

        // Move to final destination.
        if working_path != final_path {
            tokio::fs::rename(&working_path, final_path)
                .await
                .map_err(|e| anyhow!("Failed to move file to {}: {e}", final_path.display()))?;
        } else if working_path.extension().is_some_and(|e| e == "part") {
            let renamed = working_path.with_extension("");
            tokio::fs::rename(&working_path, &renamed).await?;
        }

        println!("Saved: {}", final_path.display());
        self.apply_download_delay().await;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Collection download
    // -----------------------------------------------------------------------

    /// Download all items in a collection (album, playlist, or mix).
    ///
    /// Playlist/Mix path behaviour:
    /// - `playlist_folder` ON  → tracks saved under `Playlists/{name}/`, m3u generated
    /// - `playlist_folder` OFF → tracks saved under `{artist}/{album}/` (deduped by skip_existing)
    pub async fn download_collection(&self, media_type: MediaType, id: &str) -> Result<()> {
        // Resolve collection name for playlist/mix context before locking the session.
        let collection_display_name = match media_type {
            MediaType::Playlist | MediaType::Mix => self.collection_name(&media_type, id).await,
            _ => None,
        };

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

        // Determine path context for playlist/mix tracks.
        let (playlist_ctx, mix_ctx): (Option<String>, Option<String>) =
            if self.settings.playlist_folder {
                match media_type {
                    MediaType::Playlist => (collection_display_name.clone(), None),
                    MediaType::Mix => (None, collection_display_name.clone()),
                    _ => (None, None),
                }
            } else {
                (None, None)
            };

        // Build a MultiProgress queue showing all tracks.
        let mp = MultiProgress::new();

        // Header bar.
        let header_style = ProgressStyle::with_template("{msg}").unwrap();
        let header = mp.add(ProgressBar::new(total as u64));
        header.set_style(header_style.clone());
        let collection_label = collection_display_name
            .as_deref()
            .unwrap_or(match media_type {
                MediaType::Album => "Album",
                MediaType::Playlist => "Playlist",
                MediaType::Mix => "Mix",
                _ => "Collection",
            });
        header.set_message(format!("{collection_label}  ({total} tracks)"));

        // One progress bar per track — initially shows the track title as waiting.
        let track_style = ProgressStyle::with_template("  {msg}  {bar:30.cyan/white} {pos}/{len}")
            .unwrap()
            .progress_chars("█░░");
        let waiting_style = ProgressStyle::with_template("  {msg}").unwrap();

        let track_pbs: Vec<ProgressBar> = tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let pb = mp.add(ProgressBar::new(0));
                pb.set_style(waiting_style.clone());
                let num = format!("{:02}", i + 1);
                pb.set_message(format!("·  {num}. {} - {}", t.artist_name(), t.title_display()));
                pb
            })
            .collect();

        // Footer bar.
        let footer = mp.add(ProgressBar::new(total as u64));
        footer.set_style(ProgressStyle::with_template("  {msg}").unwrap());
        footer.set_message(format!("0/{total} completed"));

        let mut completed = 0usize;
        let mut cancelled = false;

        for (i, track) in tracks.iter().enumerate() {
            let pb = &track_pbs[i];
            let num = format!("{:02}", i + 1);
            let label = format!("{num}. {} - {}", track.artist_name(), track.title_display());

            // Check for cancellation before starting next track.
            if self.cancel.is_cancelled() {
                pb.set_style(waiting_style.clone());
                pb.set_message(format!("·  {label}  (cancelled)"));
                pb.finish();
                cancelled = true;
                continue;
            }

            // Switch to active style.
            pb.set_style(track_style.clone());
            pb.set_message(format!("↓  {label}"));

            let result = self
                .download_item_with_context(
                    MediaType::Track,
                    &track.id.to_string(),
                    playlist_ctx.as_deref(),
                    mix_ctx.as_deref(),
                    Some(pb),
                )
                .await;

            if let Err(e) = result {
                pb.set_style(waiting_style.clone());
                pb.set_message(format!("✗  {label}  ({e})"));
                pb.finish();
                warn!(track_id = track.id, title = %track.title_display(), "Track download failed: {e}");
            } else {
                completed += 1;
            }

            let remaining = total - (i + 1);
            if cancelled || self.cancel.is_cancelled() {
                footer.set_message(format!("{completed}/{total} completed  ·  cancelled"));
            } else {
                footer.set_message(format!("{completed}/{total} completed  ·  {remaining} remaining"));
            }
        }

        if cancelled || self.cancel.is_cancelled() {
            footer.set_message(format!("{completed}/{total} completed  ·  cancelled"));
        } else {
            footer.set_message(format!("{completed}/{total} completed"));
        }
        footer.finish();

        // Generate m3u playlist file when playlist_folder is enabled.
        if self.settings.playlist_folder
            && let Some(folder_name) = &collection_display_name
                && matches!(media_type, MediaType::Playlist | MediaType::Mix) {
                    let base = expand_tilde(&self.settings.download_base_path);
                    let pl_dir = PathBuf::from(&base)
                        .join("Playlists")
                        .join(crate::pathfmt::format::sanitize_filename(folder_name));
                    self.write_m3u(&pl_dir, folder_name).await;
                }

        Ok(())
    }

    async fn write_m3u(&self, dir: &Path, name: &str) {
        let audio_exts = ["flac", "m4a", "mp3", "mp4"];
        let mut entries: Vec<PathBuf> = Vec::new();
        let mut rd = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(_) => return,
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let p = entry.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|e| audio_exts.contains(&e))
                .unwrap_or(false)
            {
                entries.push(p);
            }
        }
        entries.sort();

        let mut m3u = String::from("#EXTM3U\n");
        for p in &entries {
            if let Some(fname) = p.file_name().and_then(|f| f.to_str()) {
                m3u.push_str(fname);
                m3u.push('\n');
            }
        }

        let ext = match self.settings.playlist_format {
            crate::config::settings::PlaylistFormat::M3u8 => "m3u8",
            crate::config::settings::PlaylistFormat::M3u => "m3u",
        };
        let m3u_path = dir.join(format!("{}.{ext}", crate::pathfmt::format::sanitize_filename(name)));
        if let Err(e) = tokio::fs::write(&m3u_path, m3u.as_bytes()).await {
            println!("Warning: could not write playlist: {e}");
        } else {
            println!("Playlist: {}", m3u_path.display());
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a [`MediaInfo`] from a track's metadata for path formatting.
    fn build_media_info(&self, track: &Track) -> MediaInfo {
        MediaInfo {
            artist_name: Some(track.artist_name()),
            album_artist: {
                // Folder path must always use the album's primary artist so that all
                // tracks on the same album land in the same directory regardless of
                // per-track featuring credits.
                let from_album = track.album.as_ref().map(|a| {
                    let v = a.primary_artist();
                    if v.is_empty() { track.artist_name() } else { v }
                });
                from_album.or_else(|| Some(track.artist_name()))
            },
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
    fn build_audio_metadata(
        &self,
        track: &Track,
        cover_data: Option<Vec<u8>>,
        lyrics: Option<String>,
    ) -> AudioMetadata {
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
            date: track.album.as_ref().and_then(|a| a.year_str()),
            isrc: track.isrc.clone(),
            url: track.share_url.clone(),
            lyrics,
            cover_data,
            write_replay_gain: self.settings.metadata_replay_gain,
            ..Default::default()
        }
    }

    /// Fetch raw cover image bytes (used for both embedding and cover.jpg).
    async fn fetch_cover_bytes(&self, track: &Track) -> Option<Vec<u8>> {
        let cover_url = self.track_cover_url(track)?;
        let response = {
            let sess = self.session.lock().await;
            sess.request.get_v1_raw(&cover_url).await.ok()?
        };
        if !response.status().is_success() {
            return None;
        }
        response.bytes().await.ok().map(|b| b.to_vec())
    }

    /// Fetch lyrics and format as LRC text (used for both embedding and .lrc file).
    async fn fetch_lyrics_text(&self, track_id: u64) -> Option<String> {
        let lyrics = {
            let sess = self.session.lock().await;
            search::get_lyrics(&sess.request, track_id).await.ok()?
        };

        use crate::tidal::media::LyricsSubtitles;
        let content = match &lyrics.subtitles {
            Some(LyricsSubtitles::Lines(lines)) => {
                let mut s = String::new();
                for line in lines {
                    if let (Some(time_ms), Some(text)) = (&line.time, &line.lrc) {
                        let total_secs = *time_ms as f64 / 1000.0;
                        let mins = (total_secs / 60.0) as u32;
                        let secs = total_secs - (mins as f64 * 60.0);
                        s.push_str(&format!("[{mins:02}:{secs:05.2}]{text}\n"));
                    }
                }
                s
            }
            Some(LyricsSubtitles::Raw(raw)) => raw.clone(),
            None => lyrics.text.clone().unwrap_or_default(),
        };

        if content.is_empty() { None } else { Some(content) }
    }

    /// Resolve the cover-art URL for a track (from its album cover UUID).
    fn track_cover_url(&self, track: &Track) -> Option<String> {
        let album = track.album.as_ref()?;
        let cover_uuid = album.cover.as_ref()?.replace('-', "/");
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
            MediaType::Track => {
                let mut track = search::get_track(&sess.request, id).await?;
                // GET /tracks/{id} returns album with no artists[] — only a single
                // album.artist field that may be comma-joined (e.g. "A, B"). Fetch
                // the full album to get album.artists[] so primary_artist() works.
                if let Some(album) = &track.album {
                    if album.artists.as_ref().map_or(true, |v| v.is_empty()) {
                        if let Ok(full) = search::get_album(&sess.request, album.id).await {
                            track.album = Some(full);
                        }
                    }
                }
                Ok(track)
            }
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
                "  {msg:.bold}\n  {bar:36.white/237} {pos}/{len}  {eta}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█▓░"),
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
