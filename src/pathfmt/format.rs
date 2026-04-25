use regex::Regex;
use std::path::{Path, PathBuf};

use crate::config::settings::Quality;

const FILENAME_LENGTH_MAX: usize = 255;
const UNIQUIFY_THRESHOLD: u32 = 99;

#[derive(Debug, Clone, Default)]
pub struct MediaInfo {
    pub artist_name: Option<String>,
    pub album_artist: Option<String>,
    pub track_title: Option<String>,
    pub album_title: Option<String>,
    pub album_track_num: Option<u32>,
    pub album_num_tracks: Option<u32>,
    pub track_id: Option<u64>,
    pub album_id: Option<u64>,
    pub playlist_id: Option<String>,
    pub mix_name: Option<String>,
    pub playlist_name: Option<String>,
    pub track_duration_seconds: Option<u64>,
    pub album_duration_seconds: Option<u64>,
    pub album_year: Option<String>,
    pub track_quality: Option<String>,
    pub track_explicit: Option<bool>,
    pub album_explicit: Option<bool>,
    pub album_num_volumes: Option<u32>,
    pub track_volume_num: Option<u32>,
    pub isrc: Option<String>,
    pub video_quality: Option<String>,
}

pub fn format_path_media(template: &str, info: &MediaInfo, pad_min: u32) -> String {
    let re = Regex::new(r"\{(.+?)\}").unwrap();
    let result = re.replace_all(template, |caps: &regex::Captures| {
        let placeholder = &caps[1];
        format_str_media(placeholder, info, pad_min)
    });
    sanitize_path(&result)
}

pub fn format_str_media(name: &str, info: &MediaInfo, pad_min: u32) -> String {
    match name {
        "artist_name" => info.artist_name.clone().unwrap_or_default(),
        "album_artist" => info.album_artist.clone().unwrap_or_default(),
        "track_title" => info.track_title.clone().unwrap_or_default(),
        "album_title" => info.album_title.clone().unwrap_or_default(),
        "mix_name" => info.mix_name.clone().unwrap_or_default(),
        "playlist_name" => info.playlist_name.clone().unwrap_or_default(),
        "album_track_num" => {
            let num = info.album_track_num.unwrap_or(0);
            if num == 0 {
                return String::new();
            }
            let total = info.album_num_tracks.unwrap_or(0).max(1);
            let width = ((total as f64).log10() as u32 + 1).max(pad_min);
            format!("{:0>width$}", num, width = width as usize)
        }
        "album_num_tracks" => info.album_num_tracks.map(|n| n.to_string()).unwrap_or_default(),
        "track_id" => info.track_id.map(|n| n.to_string()).unwrap_or_default(),
        "album_id" => info.album_id.map(|n| n.to_string()).unwrap_or_default(),
        "playlist_id" => info.playlist_id.clone().unwrap_or_default(),
        "track_duration_seconds" => info
            .track_duration_seconds
            .map(|n| n.to_string())
            .unwrap_or_default(),
        "track_duration_minutes" => info
            .track_duration_seconds
            .map(format_duration)
            .unwrap_or_default(),
        "album_duration_seconds" => info
            .album_duration_seconds
            .map(|n| n.to_string())
            .unwrap_or_default(),
        "album_duration_minutes" => info
            .album_duration_seconds
            .map(format_duration)
            .unwrap_or_default(),
        "album_year" => info.album_year.clone().unwrap_or_default(),
        "track_quality" => info.track_quality.clone().unwrap_or_default(),
        "track_explicit" => info
            .track_explicit
            .and_then(|e| if e { Some(" (Explicit)".into()) } else { None })
            .unwrap_or_default(),
        "album_explicit" => info
            .album_explicit
            .and_then(|e| if e { Some(" (Explicit)".into()) } else { None })
            .unwrap_or_default(),
        "album_num_volumes" => info.album_num_volumes.map(|n| n.to_string()).unwrap_or_default(),
        "track_volume_num" => info.track_volume_num.map(|n| n.to_string()).unwrap_or_default(),
        "track_volume_num_optional" => {
            if info.album_num_volumes.unwrap_or(1) <= 1 {
                String::new()
            } else {
                info.track_volume_num.map(|n| n.to_string()).unwrap_or_default()
            }
        }
        "track_volume_num_optional_CD" => {
            if info.album_num_volumes.unwrap_or(1) <= 1 {
                String::new()
            } else {
                info.track_volume_num
                    .map(|n| format!("CD{}", n))
                    .unwrap_or_default()
            }
        }
        "isrc" => info.isrc.clone().unwrap_or_default(),
        "video_quality" => info.video_quality.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let trimmed = sanitized.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return "_".to_string();
    }
    let result = if trimmed.len() > FILENAME_LENGTH_MAX {
        &trimmed[..FILENAME_LENGTH_MAX]
    } else {
        trimmed
    };
    result.to_string()
}

pub fn sanitize_path(path: &str) -> String {
    path.split('/')
        .map(sanitize_filename)
        .collect::<Vec<_>>()
        .join("/")
}

pub fn file_unique_suffix(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("_");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(Path::new("."));

    for i in 1..=UNIQUIFY_THRESHOLD {
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, i)
        } else {
            format!("{}_{}.{}", stem, i, ext)
        };
        let new_path = parent.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
    }
    path.to_path_buf()
}

pub fn check_file_exists(path: &Path, extensions: &[&str]) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    let stem = path.to_string_lossy();
    let base = stem.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    for ext in extensions {
        let test_path = PathBuf::from(format!("{}.{}", base, ext));
        if test_path.exists() {
            return Some(test_path);
        }
    }
    None
}

pub fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", minutes, secs)
}

pub fn extension_guess(
    is_video: bool,
    video_convert_mp4: bool,
    extract_flac: bool,
    quality: &Quality,
    codec: Option<&str>,
    media_metadata_tags: &[String],
) -> &'static str {
    if is_video {
        return if video_convert_mp4 { ".mp4" } else { ".ts" };
    }

    let is_lossless = matches!(quality, Quality::HighLossless | Quality::HiResLossless);
    let has_hires_tag = media_metadata_tags
        .iter()
        .any(|t| t.to_uppercase().contains("HIRES_LOSSLESS"));

    if extract_flac && matches!(quality, Quality::HiResLossless | Quality::HighLossless) {
        return ".flac";
    }
    if is_lossless && !has_hires_tag {
        return if extract_flac { ".flac" } else { ".m4a" };
    }
    if matches!(quality, Quality::HighLossless) {
        return ".flac";
    }
    if codec.is_some_and(|c| c.eq_ignore_ascii_case("FLAC")) {
        return ".flac";
    }
    ".m4a"
}

pub fn get_format_template<'a>(
    media_type: &str,
    settings: &'a crate::config::settings::Settings,
) -> &'a str {
    match media_type {
        "album" => &settings.format_album,
        "playlist" => &settings.format_playlist,
        "mix" => &settings.format_mix,
        "track" => &settings.format_track,
        "video" => &settings.format_video,
        _ => &settings.format_track,
    }
}
