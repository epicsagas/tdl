use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

/// Parse an M3U8 playlist to extract segment URLs.
///
/// Handles two playlist types:
/// - **Master playlist** (contains `#EXT-X-STREAM-INF`): selects the variant with
///   the highest bandwidth, then returns its URI as a single-element vec so the
///   caller can fetch and parse the resulting media playlist.
/// - **Media playlist** (contains `#EXTINF`): returns all segment URIs directly.
pub fn parse_m3u8(m3u8_content: &str) -> Result<Vec<String>> {
    let bytes = m3u8_content.as_bytes();

    // Use m3u8_rs to detect and parse the playlist type.
    let playlist = m3u8_rs::parse_playlist(bytes)
        .map_err(|e| anyhow!("Failed to parse M3U8 playlist: {e:?}"))
        .map(|(_, pl)| pl)?;

    match playlist {
        m3u8_rs::Playlist::MasterPlaylist(master) => {
            if master.variants.is_empty() {
                return Err(anyhow!("Master playlist contains no variant streams"));
            }

            // Select the variant with the highest bandwidth.
            let best = master
                .variants
                .iter()
                .filter(|v| !v.is_i_frame)
                .max_by_key(|v| v.bandwidth)
                .ok_or_else(|| anyhow!("No usable video variants in master playlist"))?;

            // The best variant's URI may itself be a media playlist that needs
            // fetching and re-parsing. Return it so the caller can resolve it.
            Ok(vec![best.uri.clone()])
        }
        m3u8_rs::Playlist::MediaPlaylist(media) => {
            let urls: Vec<String> = media.segments.iter().map(|s| s.uri.clone()).collect();

            if urls.is_empty() {
                return Err(anyhow!("Media playlist contains no segments"));
            }

            Ok(urls)
        }
    }
}

/// Select the best resolution from a master M3U8 playlist's alternative media.
#[allow(dead_code)]
///
/// Returns the URI of the alternative with the highest bandwidth, falling back
/// to `None` when no alternatives carry a URI.
fn select_best_resolution(variants: &[m3u8_rs::AlternativeMedia]) -> Option<String> {
    // AlternativeMedia entries do not carry bandwidth information directly,
    // but the convention is to pick the last-listed (typically highest-quality)
    // entry that has a URI.
    variants.iter().rev().find_map(|alt| alt.uri.clone())
}

/// Convert TS video to MP4 using FFmpeg.
///
/// Runs: `ffmpeg -y -i <input> -codec copy -map 0 -loglevel quiet <output>`
///
/// If `ffmpeg_path` is empty, `"ffmpeg"` is resolved from the system PATH.
pub fn convert_ts_to_mp4(input: &Path, output: &Path, ffmpeg_path: &str) -> Result<()> {
    let ffmpeg = if ffmpeg_path.is_empty() {
        "ffmpeg"
    } else {
        ffmpeg_path
    };

    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-codec",
            "copy",
            "-map",
            "0",
            "-loglevel",
            "quiet",
            &output.to_string_lossy(),
        ])
        .status()
        .map_err(|e| anyhow!("Failed to execute ffmpeg: {e}"))?;

    if !status.success() {
        return Err(anyhow!(
            "ffmpeg exited with status {} when converting {} to {}",
            status.code().unwrap_or(-1),
            input.display(),
            output.display()
        ));
    }

    Ok(())
}

/// Extract FLAC audio from an MP4 container using FFmpeg.
///
/// Runs: `ffmpeg -y -i <input> -map 0 -movflags use_metadata_tags -acodec copy
///        -map_metadata 0:g -loglevel quiet <output>`
///
/// If `ffmpeg_path` is empty, `"ffmpeg"` is resolved from the system PATH.
pub fn extract_flac(input: &Path, output: &Path, ffmpeg_path: &str) -> Result<()> {
    let ffmpeg = if ffmpeg_path.is_empty() {
        "ffmpeg"
    } else {
        ffmpeg_path
    };

    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-map",
            "0",
            "-movflags",
            "use_metadata_tags",
            "-acodec",
            "copy",
            "-map_metadata",
            "0:g",
            "-loglevel",
            "quiet",
            &output.to_string_lossy(),
        ])
        .status()
        .map_err(|e| anyhow!("Failed to execute ffmpeg: {e}"))?;

    if !status.success() {
        return Err(anyhow!(
            "ffmpeg exited with status {} when extracting FLAC from {} to {}",
            status.code().unwrap_or(-1),
            input.display(),
            output.display()
        ));
    }

    Ok(())
}

/// Check if FFmpeg is available.
///
/// If `ffmpeg_path` is a non-empty string, checks whether that path exists.
/// Otherwise, checks whether `"ffmpeg"` can be found on the system PATH.
pub fn ffmpeg_available(ffmpeg_path: &str) -> bool {
    if !ffmpeg_path.is_empty() {
        Path::new(ffmpeg_path).exists()
    } else {
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_media_playlist() {
        let m3u8 = concat!(
            "#EXTM3U\n",
            "#EXT-X-VERSION:3\n",
            "#EXT-X-TARGETDURATION:10\n",
            "#EXTINF:9.9,\n",
            "https://cdn.example.com/seg1.ts\n",
            "#EXTINF:9.9,\n",
            "https://cdn.example.com/seg2.ts\n",
            "#EXTINF:9.9,\n",
            "https://cdn.example.com/seg3.ts\n",
            "#EXT-X-ENDLIST\n",
        );

        let urls = parse_m3u8(m3u8).unwrap();
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://cdn.example.com/seg1.ts");
        assert_eq!(urls[2], "https://cdn.example.com/seg3.ts");
    }

    #[test]
    fn parse_master_playlist_selects_best_bandwidth() {
        let m3u8 = concat!(
            "#EXTM3U\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\n",
            "https://cdn.example.com/360p.m3u8\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=2800000,RESOLUTION=1280x720\n",
            "https://cdn.example.com/720p.m3u8\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080\n",
            "https://cdn.example.com/1080p.m3u8\n",
        );

        let urls = parse_m3u8(m3u8).unwrap();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://cdn.example.com/1080p.m3u8");
    }

    #[test]
    fn parse_master_playlist_empty_variants_fails() {
        let m3u8 = "#EXTM3U\n";
        let result = parse_m3u8(m3u8);
        assert!(result.is_err());
    }

    #[test]
    fn parse_media_playlist_empty_segments_fails() {
        let m3u8 = concat!(
            "#EXTM3U\n",
            "#EXT-X-VERSION:3\n",
            "#EXT-X-TARGETDURATION:10\n",
            "#EXT-X-ENDLIST\n",
        );
        let result = parse_m3u8(m3u8);
        assert!(result.is_err());
    }

    #[test]
    fn select_best_resolution_returns_last_with_uri() {
        let variants = vec![
            m3u8_rs::AlternativeMedia {
                uri: Some("audio_128.m3u8".to_string()),
                ..Default::default()
            },
            m3u8_rs::AlternativeMedia {
                uri: Some("audio_320.m3u8".to_string()),
                ..Default::default()
            },
        ];
        let result = select_best_resolution(&variants);
        assert_eq!(result, Some("audio_320.m3u8".to_string()));
    }

    #[test]
    fn select_best_resolution_no_uri_returns_none() {
        let variants = vec![m3u8_rs::AlternativeMedia {
            uri: None,
            ..Default::default()
        }];
        let result = select_best_resolution(&variants);
        assert!(result.is_none());
    }

    #[test]
    fn ffmpeg_available_with_empty_path_checks_system() {
        // This test just verifies the function does not panic.
        // The result depends on whether ffmpeg is installed on the test machine.
        let _ = ffmpeg_available("");
    }

    #[test]
    fn ffmpeg_available_with_nonexistent_path_returns_false() {
        assert!(!ffmpeg_available("/nonexistent/path/to/ffmpeg"));
    }
}
