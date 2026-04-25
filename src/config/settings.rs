use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Low96k,
    #[default]
    Low320k,
    HighLossless,
    HiResLossless,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QualityVideo {
    P360,
    #[default]
    P480,
    P720,
    P1080,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CoverDimensions {
    Px80,
    Px160,
    #[default]
    Px320,
    Px640,
    Px1280,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub skip_existing: bool,
    pub lyrics_embed: bool,
    pub lyrics_file: bool,
    pub video_download: bool,
    pub download_delay: bool,
    pub download_base_path: String,
    pub quality_audio: Quality,
    pub quality_video: QualityVideo,
    pub format_album: String,
    pub format_playlist: String,
    pub format_mix: String,
    pub format_track: String,
    pub format_video: String,
    pub video_convert_mp4: bool,
    pub path_binary_ffmpeg: String,
    pub metadata_cover_dimension: CoverDimensions,
    pub metadata_cover_embed: bool,
    pub cover_album_file: bool,
    pub extract_flac: bool,
    pub downloads_simultaneous_per_track_max: usize,
    pub download_delay_sec_min: f64,
    pub download_delay_sec_max: f64,
    pub album_track_num_pad_min: u32,
    pub downloads_concurrent_max: usize,
    pub symlink_to_track: bool,
    pub playlist_create: bool,
    pub metadata_replay_gain: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            skip_existing: true,
            lyrics_embed: false,
            lyrics_file: false,
            video_download: true,
            download_delay: true,
            download_base_path: "~/download".to_string(),
            quality_audio: Quality::default(),
            quality_video: QualityVideo::default(),
            format_album: "Albums/{album_artist} - {album_title}{album_explicit}/{track_volume_num_optional}{album_track_num}. {artist_name} - {track_title}{album_explicit}".to_string(),
            format_playlist: "Playlists/{playlist_name}/{artist_name} - {track_title}".to_string(),
            format_mix: "Mix/{mix_name}/{artist_name} - {track_title}".to_string(),
            format_track: "Tracks/{artist_name} - {track_title}{track_explicit}".to_string(),
            format_video: "Videos/{artist_name} - {track_title}{track_explicit}".to_string(),
            video_convert_mp4: true,
            path_binary_ffmpeg: String::new(),
            metadata_cover_dimension: CoverDimensions::default(),
            metadata_cover_embed: true,
            cover_album_file: true,
            extract_flac: true,
            downloads_simultaneous_per_track_max: 20,
            download_delay_sec_min: 3.0,
            download_delay_sec_max: 5.0,
            album_track_num_pad_min: 1,
            downloads_concurrent_max: 3,
            symlink_to_track: false,
            playlist_create: false,
            metadata_replay_gain: true,
        }
    }
}

impl Settings {
    /// Returns the configuration directory path: `~/.config/tidal-dl-ng/`
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("tidal-dl-ng")
    }

    /// Returns the full path to the settings file: `~/.config/tidal-dl-ng/settings.json`
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    /// Load settings from the JSON configuration file.
    ///
    /// If the file does not exist, returns default settings.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read settings from {}", path.display()))?;

        let settings: Settings = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse settings from {}", path.display()))?;

        Ok(settings)
    }

    /// Save settings to the JSON configuration file.
    ///
    /// Creates the configuration directory if it does not exist.
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();

        if !dir.exists() {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create config directory {}", dir.display()))?;
        }

        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize settings to JSON")?;

        fs::write(&path, &json)
            .with_context(|| format!("Failed to write settings to {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_python() {
        let s = Settings::default();
        assert!(s.skip_existing);
        assert!(!s.lyrics_embed);
        assert!(!s.lyrics_file);
        assert!(s.video_download);
        assert!(s.download_delay);
        assert_eq!(s.download_base_path, "~/download");
        assert_eq!(s.quality_audio, Quality::Low320k);
        assert_eq!(s.quality_video, QualityVideo::P480);
        assert!(s.video_convert_mp4);
        assert!(s.path_binary_ffmpeg.is_empty());
        assert_eq!(s.metadata_cover_dimension, CoverDimensions::Px320);
        assert!(s.metadata_cover_embed);
        assert!(s.cover_album_file);
        assert!(s.extract_flac);
        assert_eq!(s.downloads_simultaneous_per_track_max, 20);
        assert_eq!(s.download_delay_sec_min, 3.0);
        assert_eq!(s.download_delay_sec_max, 5.0);
        assert_eq!(s.album_track_num_pad_min, 1);
        assert_eq!(s.downloads_concurrent_max, 3);
        assert!(!s.symlink_to_track);
        assert!(!s.playlist_create);
        assert!(s.metadata_replay_gain);
    }

    #[test]
    fn roundtrip_serialization() {
        let original = Settings::default();
        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(original.skip_existing, restored.skip_existing);
        assert_eq!(original.quality_audio, restored.quality_audio);
        assert_eq!(original.quality_video, restored.quality_video);
        assert_eq!(
            original.metadata_cover_dimension,
            restored.metadata_cover_dimension
        );
        assert_eq!(original.format_album, restored.format_album);
        assert_eq!(original.download_base_path, restored.download_base_path);
    }

    #[test]
    fn quality_serde_roundtrip() {
        let q = Quality::HiResLossless;
        let json = serde_json::to_string(&q).unwrap();
        assert_eq!(json, "\"hi_res_lossless\"");
        let back: Quality = serde_json::from_str(&json).unwrap();
        assert_eq!(q, back);
    }

    #[test]
    fn cover_dimensions_serde_roundtrip() {
        let d = CoverDimensions::Px1280;
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"px1280\"");
        let back: CoverDimensions = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
