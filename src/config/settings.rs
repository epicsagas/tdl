use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    pub track_num_pad_zero: bool,
    pub playlist_folder: bool,
    pub video_convert_mp4: bool,
    pub path_binary_ffmpeg: String,
    pub metadata_cover_dimension: CoverDimensions,
    pub metadata_cover_embed: bool,
    pub cover_album_file: bool,
    pub extract_flac: bool,
    pub downloads_simultaneous_per_track_max: usize,
    pub download_delay_sec_min: f64,
    pub download_delay_sec_max: f64,
    pub downloads_concurrent_max: usize,
    pub symlink_to_track: bool,
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
            track_num_pad_zero: true,
            playlist_folder: true,
            video_convert_mp4: true,
            path_binary_ffmpeg: String::new(),
            metadata_cover_dimension: CoverDimensions::default(),
            metadata_cover_embed: true,
            cover_album_file: true,
            extract_flac: true,
            downloads_simultaneous_per_track_max: 20,
            download_delay_sec_min: 3.0,
            download_delay_sec_max: 5.0,
            downloads_concurrent_max: 3,
            symlink_to_track: false,
            metadata_replay_gain: true,
        }
    }
}

impl Settings {
    /// Returns the configuration directory path: `~/.tdl/`
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".tdl")
    }

    /// Returns the full path to the settings file: `~/.tdl/settings.json`
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    /// Load settings from the JSON configuration file.
    ///
    /// If the file does not exist, returns default settings.
    /// After loading, auto-detects FFmpeg if the path is not configured.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        let mut settings = if !path.exists() {
            Self::default()
        } else {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read settings from {}", path.display()))?;
            serde_json::from_str(&contents)
                .with_context(|| format!("Failed to parse settings from {}", path.display()))?
        };

        settings.resolve_ffmpeg();
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

    /// Return the effective FFmpeg path: the configured path if set and exists,
    /// otherwise the auto-detected path, or empty string if not found.
    pub fn ffmpeg_path(&self) -> &str {
        if !self.path_binary_ffmpeg.is_empty() {
            return &self.path_binary_ffmpeg;
        }
        ""
    }

    /// Auto-detect FFmpeg if not already configured.
    ///
    /// Checks in order: configured path, system PATH (`which`), common install locations.
    fn resolve_ffmpeg(&mut self) {
        if !self.path_binary_ffmpeg.is_empty() && Path::new(&self.path_binary_ffmpeg).exists() {
            return;
        }

        // Try system PATH via `which`
        if let Ok(output) = Command::new("which").arg("ffmpeg").output() {
            if output.status.success() {
                let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !p.is_empty() && Path::new(&p).exists() {
                    self.path_binary_ffmpeg = p;
                    return;
                }
            }
        }

        // Try common locations
        let candidates = [
            "/opt/homebrew/bin/ffmpeg",   // macOS Homebrew (Apple Silicon)
            "/usr/local/bin/ffmpeg",      // macOS Homebrew (Intel) / Linux
            "/opt/local/bin/ffmpeg",      // macOS MacPorts
            "/usr/bin/ffmpeg",            // Linux distro package
            "/snap/bin/ffmpeg",           // Ubuntu Snap
        ];
        for candidate in &candidates {
            if Path::new(candidate).exists() {
                self.path_binary_ffmpeg = candidate.to_string();
                return;
            }
        }
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
        assert!(s.track_num_pad_zero);
        assert!(s.playlist_folder);
        assert!(s.video_convert_mp4);
        assert!(s.path_binary_ffmpeg.is_empty());
        assert_eq!(s.metadata_cover_dimension, CoverDimensions::Px320);
        assert!(s.metadata_cover_embed);
        assert!(s.cover_album_file);
        assert!(s.extract_flac);
        assert_eq!(s.downloads_simultaneous_per_track_max, 20);
        assert_eq!(s.download_delay_sec_min, 3.0);
        assert_eq!(s.download_delay_sec_max, 5.0);
        assert_eq!(s.downloads_concurrent_max, 3);
        assert!(!s.symlink_to_track);
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
        assert_eq!(original.track_num_pad_zero, restored.track_num_pad_zero);
        assert_eq!(original.playlist_folder, restored.playlist_folder);
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
