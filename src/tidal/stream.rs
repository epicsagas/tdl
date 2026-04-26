use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::Deserialize;
use std::collections::HashMap;

use crate::config::settings::Quality;
use crate::tidal::media::{PlaybackInfoResponse, VideoUrlResponse};
use crate::tidal::request::TidalRequest;

// ---------------------------------------------------------------------------
// StreamManifest
// ---------------------------------------------------------------------------

/// Parsed representation of a Tidal stream manifest.
///
/// Agnostic over the underlying format (BTS JSON or MPEG-DASH XML).
#[derive(Debug, Clone)]
pub struct StreamManifest {
    /// Ordered list of segment / file URLs to download.
    pub urls: Vec<String>,
    /// Codec string from the manifest (e.g. "flac", "aac").
    pub codecs: Option<String>,
    /// MIME type from the manifest.
    pub mime_type: Option<String>,
    /// Whether the stream is encrypted.
    pub is_encrypted: bool,
    /// Encryption key ID (present when encrypted).
    pub encryption_key: Option<String>,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Suggested file extension derived from the codec / manifest type.
    pub file_extension: Option<String>,
    /// True when the manifest was MPEG-DASH XML.
    pub is_mpd: bool,
    /// True when the manifest was BTS JSON.
    pub is_bts: bool,
}

// ---------------------------------------------------------------------------
// BTS manifest (JSON)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct BtsManifest {
    #[serde(default)]
    urls: Option<Vec<String>>,
    #[serde(default)]
    codecs: Option<String>,
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default, rename = "encryptionType")]
    encryption_type: Option<String>,
    #[serde(default, rename = "keyId")]
    key_id: Option<String>,
    #[serde(default, rename = "sampleRate")]
    sample_rate: Option<u32>,
}

// ---------------------------------------------------------------------------
// Quality helpers
// ---------------------------------------------------------------------------

/// Map the application-level `Quality` enum to the string the Tidal API
/// expects in the `audioQuality` query parameter.
fn quality_to_api_string(quality: &Quality) -> &'static str {
    match quality {
        Quality::Low96k => "LOW",
        Quality::Low320k => "HIGH",
        Quality::HighLossless => "LOSSLESS",
        Quality::HiResLossless => "HI_RES_LOSSLESS",
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch playback info and decode the stream manifest for a track.
///
/// Returns both the parsed `StreamManifest` and the raw `PlaybackInfoResponse`
/// (which carries replay-gain, bit-depth, sample-rate metadata).
pub async fn fetch_track_stream(
    request: &TidalRequest,
    track_id: u64,
    quality: &Quality,
) -> Result<(StreamManifest, PlaybackInfoResponse)> {
    let path = format!("tracks/{track_id}/playbackinfopostpaywall");

    let mut params = HashMap::new();
    params.insert("playbackmode".to_string(), "STREAM".to_string());
    params.insert(
        "audioquality".to_string(),
        quality_to_api_string(quality).to_string(),
    );
    params.insert("assetpresentation".to_string(), "FULL".to_string());

    let info: PlaybackInfoResponse = request.get(&path, Some(params)).await?;

    let manifest_b64 = info
        .manifest
        .as_ref()
        .ok_or_else(|| anyhow!("No manifest in playback info for track {track_id}"))?;

    let manifest_bytes = base64::engine::general_purpose::STANDARD
        .decode(manifest_b64)
        .context("Failed to base64-decode manifest")?;

    let mime = info
        .manifest_mime_type
        .as_deref()
        .unwrap_or("application/vnd.tidal.bts");

    let stream_manifest = if mime.contains("dash") || mime.contains("mpd") {
        parse_mpd(&manifest_bytes)?
    } else {
        // Default to BTS parsing for "application/vnd.tidal.bts" and anything else.
        parse_bts(&manifest_bytes)?
    };

    Ok((stream_manifest, info))
}

/// Fetch the streaming URL for a video.
pub async fn fetch_video_url(
    request: &TidalRequest,
    video_id: u64,
    quality: &str,
) -> Result<String> {
    let path = format!("videos/{video_id}/urlpostpaywall");

    let mut params = HashMap::new();
    params.insert("urlusagemode".to_string(), "STREAM".to_string());
    params.insert("videoquality".to_string(), quality.to_string());
    params.insert("assetpresentation".to_string(), "FULL".to_string());

    let resp: VideoUrlResponse = request.get(&path, Some(params)).await?;

    resp.url
        .ok_or_else(|| anyhow!("No URL returned for video {video_id}"))
}

// ---------------------------------------------------------------------------
// BTS parsing
// ---------------------------------------------------------------------------

fn parse_bts(data: &[u8]) -> Result<StreamManifest> {
    let bts: BtsManifest =
        serde_json::from_slice(data).context("Failed to parse BTS manifest JSON")?;

    let urls = bts.urls.unwrap_or_default();
    let is_encrypted = bts
        .encryption_type
        .as_ref()
        .is_some_and(|e| e != "NONE" && !e.is_empty());

    let file_extension = guess_extension_from_codecs(bts.codecs.as_deref());

    Ok(StreamManifest {
        urls,
        codecs: bts.codecs,
        mime_type: bts.mime_type,
        is_encrypted,
        encryption_key: bts.key_id,
        sample_rate: bts.sample_rate,
        file_extension,
        is_mpd: false,
        is_bts: true,
    })
}

// ---------------------------------------------------------------------------
// MPD (MPEG-DASH XML) parsing
// ---------------------------------------------------------------------------

fn parse_mpd(data: &[u8]) -> Result<StreamManifest> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_reader(data);

    let mut urls: Vec<String> = Vec::new();
    let mut codecs: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let is_encrypted = false;
    let encryption_key: Option<String> = None;
    let mut sample_rate: Option<u32> = None;

    // State machine for XML walking
    let mut in_period = false;
    let mut in_adaptation_set = false;
    let mut in_representation = false;
    let mut in_segment_timeline = false;

    // SegmentTemplate fields
    let mut init_url: Option<String> = None;
    let mut media_template: Option<String> = None;
    let mut start_number: u64 = 1;

    // SegmentTimeline entries
    let mut timeline_segments: Vec<(u64, u64)> = Vec::new(); // (duration, repeat_count)

    // BaseURL approach
    let mut base_url: Option<String> = None;
    let mut segment_list_media: Vec<String> = Vec::new();
    let mut in_segment_list = false;
    let mut in_base_url = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local_name = e.name().local_name();
                let local: &[u8] = local_name.as_ref();

                match local {
                    b"Period" => in_period = true,
                    b"AdaptationSet" => {
                        if in_period {
                            in_adaptation_set = true;
                        }
                    }
                    b"Representation" => {
                        if in_adaptation_set {
                            in_representation = true;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"codecs" => {
                                        codecs = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"mimeType" => {
                                        mime_type = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"audioSamplingRate" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            sample_rate = s.parse().ok();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"SegmentTemplate" => {
                        if in_representation {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"initialization" => {
                                        init_url = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"media" => {
                                        media_template = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"startNumber" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            start_number = s.parse().unwrap_or(1);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"SegmentTimeline" => {
                        if in_representation {
                            in_segment_timeline = true;
                        }
                    }
                    b"S" => {
                        if in_segment_timeline {
                            let mut duration: u64 = 0;
                            let mut repeat: u64 = 0;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"d" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            duration = s.parse().unwrap_or(0);
                                        }
                                    }
                                    b"r" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            repeat = s.parse().unwrap_or(0);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            timeline_segments.push((duration, repeat));
                        }
                    }
                    b"BaseURL" => {
                        if in_representation {
                            in_base_url = true;
                        }
                    }
                    b"SegmentList" => {
                        if in_representation {
                            in_segment_list = true;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local_name = e.name().local_name();
                let local: &[u8] = local_name.as_ref();

                match local {
                    b"SegmentTemplate" => {
                        if in_representation {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"initialization" => {
                                        init_url = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"media" => {
                                        media_template = Some(
                                            String::from_utf8(attr.value.to_vec())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    b"startNumber" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            start_number = s.parse().unwrap_or(1);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"S" => {
                        if in_segment_timeline {
                            let mut duration: u64 = 0;
                            let mut repeat: u64 = 0;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"d" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            duration = s.parse().unwrap_or(0);
                                        }
                                    }
                                    b"r" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            repeat = s.parse().unwrap_or(0);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            timeline_segments.push((duration, repeat));
                        }
                    }
                    b"SegmentURL" => {
                        if in_segment_list {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"media" {
                                    segment_list_media.push(
                                        String::from_utf8(attr.value.to_vec()).unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_base_url {
                    let text = e.as_ref();
                    let trimmed = std::str::from_utf8(text).unwrap_or("").trim();
                    if !trimmed.is_empty() && base_url.is_none() {
                        base_url = Some(trimmed.to_string());
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = e.name().local_name();
                let local: &[u8] = local_name.as_ref();

                match local {
                    b"Period" => in_period = false,
                    b"AdaptationSet" => in_adaptation_set = false,
                    b"Representation" => in_representation = false,
                    b"SegmentTimeline" => in_segment_timeline = false,
                    b"SegmentList" => in_segment_list = false,
                    b"BaseURL" => in_base_url = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow!("Error parsing MPD XML: {e}"));
            }
            _ => {}
        }
        buf.clear();
    }

    // Build the URL list.
    // Strategy 1: SegmentTemplate with SegmentTimeline
    if let Some(ref tmpl) = media_template {
        if !timeline_segments.is_empty() {
            // Prepend initialization segment.
            if let Some(ref init) = init_url {
                urls.push(init.clone());
            }
            let mut seg_num = start_number;
            for (_duration, repeat) in &timeline_segments {
                let count = (*repeat + 1) as usize;
                for _ in 0..count {
                    urls.push(tmpl.replace("$Number$", &seg_num.to_string()));
                    seg_num += 1;
                }
            }
        } else {
            // SegmentTemplate without timeline: generate sequential segments.
            if let Some(ref init) = init_url {
                urls.push(init.clone());
            }
            // Without a timeline we don't know the count; generate up to 1000.
            for i in start_number..(start_number + 1000) {
                urls.push(tmpl.replace("$Number$", &i.to_string()));
            }
        }
    }
    // Strategy 2: BaseURL + SegmentList
    else if !segment_list_media.is_empty() {
        if let Some(ref base) = base_url {
            urls.push(base.clone());
        }
        urls.extend(segment_list_media);
    }
    // Strategy 3: BaseURL only
    else if let Some(ref base) = base_url {
        urls.push(base.clone());
    }

    if urls.is_empty() {
        return Err(anyhow!("MPD manifest contained no downloadable URLs"));
    }

    let file_extension = guess_extension_from_codecs(codecs.as_deref());

    Ok(StreamManifest {
        urls,
        codecs,
        mime_type,
        is_encrypted,
        encryption_key,
        sample_rate,
        file_extension,
        is_mpd: true,
        is_bts: false,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Heuristic: derive a file extension from the codec string in the manifest.
fn guess_extension_from_codecs(codecs: Option<&str>) -> Option<String> {
    let codecs = codecs?;
    let lower = codecs.to_ascii_lowercase();
    if lower.contains("flac") {
        Some("flac".to_string())
    } else if lower.contains("mp4a") || lower.contains("aac") {
        Some("m4a".to_string())
    } else if lower.contains("mp3") {
        Some("mp3".to_string())
    } else if lower.contains("ec-3") || lower.contains("eac3") || lower.contains("ac4") {
        Some("m4a".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_mapping() {
        assert_eq!(quality_to_api_string(&Quality::Low96k), "LOW");
        assert_eq!(quality_to_api_string(&Quality::Low320k), "HIGH");
        assert_eq!(quality_to_api_string(&Quality::HighLossless), "LOSSLESS");
        assert_eq!(
            quality_to_api_string(&Quality::HiResLossless),
            "HI_RES_LOSSLESS"
        );
    }

    #[test]
    fn parse_bts_basic() {
        let json = r#"{
            "urls": ["https://example.com/track.flac"],
            "codecs": "flac",
            "mimeType": "audio/flac",
            "encryptionType": "NONE",
            "sampleRate": 44100
        }"#;
        let manifest = parse_bts(json.as_bytes()).unwrap();
        assert_eq!(manifest.urls.len(), 1);
        assert_eq!(manifest.codecs.as_deref(), Some("flac"));
        assert!(!manifest.is_encrypted);
        assert!(manifest.is_bts);
        assert!(!manifest.is_mpd);
        assert_eq!(manifest.sample_rate, Some(44100));
    }

    #[test]
    fn parse_bts_encrypted() {
        let json = r#"{
            "urls": ["https://example.com/track.flac"],
            "codecs": "flac",
            "encryptionType": "CENC",
            "keyId": "abc123"
        }"#;
        let manifest = parse_bts(json.as_bytes()).unwrap();
        assert!(manifest.is_encrypted);
        assert_eq!(manifest.encryption_key.as_deref(), Some("abc123"));
    }

    #[test]
    fn parse_mpd_segment_template() {
        let xml = r#"<?xml version="1.0"?>
        <MPD>
            <Period>
                <AdaptationSet>
                    <Representation codecs="flac" mimeType="audio/flac" audioSamplingRate="44100">
                        <SegmentTemplate initialization="init.flac" media="seg$Number$.flac" startNumber="1">
                            <SegmentTimeline>
                                <S d="1000" r="2"/>
                            </SegmentTimeline>
                        </SegmentTemplate>
                    </Representation>
                </AdaptationSet>
            </Period>
        </MPD>"#;
        let manifest = parse_mpd(xml.as_bytes()).unwrap();
        assert!(manifest.is_mpd);
        assert!(!manifest.is_bts);
        // init + 3 segments (r=2 means 3 total)
        assert_eq!(manifest.urls.len(), 4);
        assert_eq!(manifest.urls[0], "init.flac");
        assert_eq!(manifest.urls[1], "seg1.flac");
        assert_eq!(manifest.urls[2], "seg2.flac");
        assert_eq!(manifest.urls[3], "seg3.flac");
        assert_eq!(manifest.sample_rate, Some(44100));
    }

    #[test]
    fn parse_mpd_base_url_segment_list() {
        let xml = r#"<?xml version="1.0"?>
        <MPD>
            <Period>
                <AdaptationSet>
                    <Representation codecs="mp4a.40.2">
                        <BaseURL>https://cdn.example.com/base/</BaseURL>
                        <SegmentList>
                            <SegmentURL media="seg1.ts"/>
                            <SegmentURL media="seg2.ts"/>
                        </SegmentList>
                    </Representation>
                </AdaptationSet>
            </Period>
        </MPD>"#;
        let manifest = parse_mpd(xml.as_bytes()).unwrap();
        assert_eq!(manifest.urls.len(), 3);
        assert_eq!(manifest.urls[0], "https://cdn.example.com/base/");
        assert_eq!(manifest.urls[1], "seg1.ts");
        assert_eq!(manifest.urls[2], "seg2.ts");
    }

    #[test]
    fn guess_extension() {
        assert_eq!(
            guess_extension_from_codecs(Some("flac")),
            Some("flac".to_string())
        );
        assert_eq!(
            guess_extension_from_codecs(Some("mp4a.40.2")),
            Some("m4a".to_string())
        );
        assert_eq!(
            guess_extension_from_codecs(Some("mp3")),
            Some("mp3".to_string())
        );
        assert_eq!(guess_extension_from_codecs(Some("unknown")), None);
        assert_eq!(guess_extension_from_codecs(None), None);
    }
}
