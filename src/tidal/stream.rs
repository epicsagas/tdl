use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::Deserialize;
use std::collections::HashMap;

use crate::config::settings::Quality;
use crate::config::settings::QualityVideo;
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

/// Map the application-level `QualityVideo` enum to the string the Tidal API
/// expects in the `videoquality` query parameter.
fn video_quality_to_api_string(quality: &QualityVideo) -> &'static str {
    match quality {
        QualityVideo::P360 => "LOW",
        QualityVideo::P480 => "MEDIUM",
        QualityVideo::P720 => "HIGH",
        QualityVideo::P1080 => "HIGH",
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
    quality: &QualityVideo,
) -> Result<String> {
    let path = format!("videos/{video_id}/urlpostpaywall");

    let mut params = HashMap::new();
    params.insert("urlusagemode".to_string(), "STREAM".to_string());
    params.insert(
        "videoquality".to_string(),
        video_quality_to_api_string(quality).to_string(),
    );
    params.insert("assetpresentation".to_string(), "FULL".to_string());

    let resp: VideoUrlResponse = request.get(&path, Some(params)).await?;

    resp.urls
        .and_then(|urls| urls.into_iter().next())
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

    // Final outputs — populated when the first complete Representation is closed.
    let mut urls: Vec<String> = Vec::new();
    let mut codecs: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let is_encrypted = false;
    let encryption_key: Option<String> = None;
    let mut sample_rate: Option<u32> = None;

    // State machine
    let mut in_period = false;
    let mut in_adaptation_set = false;
    let mut in_representation = false;
    let mut in_segment_timeline = false;
    let mut segment_timeline_in_adapt = false;

    // AdaptationSet-level SegmentTemplate (inherited by each Representation)
    let mut adapt_init_url: Option<String> = None;
    let mut adapt_media_template: Option<String> = None;
    let mut adapt_start_number: u64 = 1;
    let mut adapt_timeline: Vec<(u64, u64)> = Vec::new();

    // Current Representation state (reset on each <Representation>)
    let mut rep_id = String::new();
    let mut rep_init_url: Option<String> = None;
    let mut rep_media_template: Option<String> = None;
    let mut rep_start_number: u64 = 1;
    let mut rep_timeline: Vec<(u64, u64)> = Vec::new();
    let mut rep_base_url: Option<String> = None;
    let mut rep_segment_list: Vec<String> = Vec::new();
    let mut in_segment_list = false;
    let mut in_base_url = false;

    let mut buf = Vec::new();

    // Helper: parse SegmentTemplate attributes into (init, media, start_number).
    // Uses unescape_value() so XML entities like &amp; are decoded to &.
    fn read_segment_template_attrs(e: &quick_xml::events::BytesStart) -> (Option<String>, Option<String>, Option<u64>) {
        let mut init = None;
        let mut media = None;
        let mut start = None;
        for attr in e.attributes().flatten() {
            let unescaped = attr.unescape_value().ok().map(|v| v.into_owned());
            match attr.key.local_name().as_ref() {
                b"initialization" => init = unescaped,
                b"media" => media = unescaped,
                b"startNumber" => {
                    start = unescaped.as_deref().and_then(|s| s.parse().ok());
                }
                _ => {}
            }
        }
        (init, media, start)
    }

    // Helper: parse one <S> element into (duration, repeat).
    fn read_s_attrs(e: &quick_xml::events::BytesStart) -> (u64, u64) {
        let mut d = 0u64;
        let mut r = 0u64;
        for attr in e.attributes().flatten() {
            match attr.key.local_name().as_ref() {
                b"d" => { if let Ok(s) = std::str::from_utf8(&attr.value) { d = s.parse().unwrap_or(0); } }
                b"r" => { if let Ok(s) = std::str::from_utf8(&attr.value) { r = s.parse().unwrap_or(0); } }
                _ => {}
            }
        }
        (d, r)
    }

    // Expand $RepresentationID$ and $Number$ in a template string.
    let expand_tmpl = |tmpl: &str, rep: &str, num: u64| -> String {
        tmpl.replace("$RepresentationID$", rep)
            .replace("$Number$", &num.to_string())
    };
    let expand_static = |s: &str, rep: &str| -> String {
        s.replace("$RepresentationID$", rep)
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local_name_owned = e.name().local_name().as_ref().to_vec();
                let local: &[u8] = &local_name_owned;
                match local {
                    b"Period" => in_period = true,
                    b"AdaptationSet"
                        if in_period => {
                            in_adaptation_set = true;
                        }
                    b"Representation"
                        if in_adaptation_set => {
                            in_representation = true;
                            // Inherit AdaptationSet defaults.
                            rep_init_url = adapt_init_url.clone();
                            rep_media_template = adapt_media_template.clone();
                            rep_start_number = adapt_start_number;
                            rep_timeline = adapt_timeline.clone();
                            rep_id.clear();
                            rep_base_url = None;
                            rep_segment_list.clear();
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => rep_id = String::from_utf8(attr.value.to_vec()).unwrap_or_default(),
                                    b"codecs" => codecs = Some(String::from_utf8(attr.value.to_vec()).unwrap_or_default()),
                                    b"mimeType" => mime_type = Some(String::from_utf8(attr.value.to_vec()).unwrap_or_default()),
                                    b"audioSamplingRate" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            sample_rate = s.parse().ok();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    b"SegmentTemplate" => {
                        let (init, media, start) = read_segment_template_attrs(e);
                        if in_representation {
                            if let Some(v) = init { rep_init_url = Some(v); }
                            if let Some(v) = media { rep_media_template = Some(v); }
                            if let Some(n) = start { rep_start_number = n; }
                        } else if in_adaptation_set {
                            if let Some(v) = init { adapt_init_url = Some(v); }
                            if let Some(v) = media { adapt_media_template = Some(v); }
                            if let Some(n) = start { adapt_start_number = n; }
                        }
                    }
                    b"SegmentTimeline"
                        if (in_representation || in_adaptation_set) => {
                            in_segment_timeline = true;
                            segment_timeline_in_adapt = !in_representation;
                            // Clear Representation-level timeline when starting a new one inside it.
                            if in_representation {
                                rep_timeline.clear();
                            }
                        }
                    b"S"
                        if in_segment_timeline => {
                            let (d, r) = read_s_attrs(e);
                            if segment_timeline_in_adapt {
                                adapt_timeline.push((d, r));
                            } else {
                                rep_timeline.push((d, r));
                            }
                        }
                    b"BaseURL"
                        if in_representation => { in_base_url = true; }
                    b"SegmentList"
                        if in_representation => { in_segment_list = true; }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local_name_owned = e.name().local_name().as_ref().to_vec();
                let local: &[u8] = &local_name_owned;
                match local {
                    b"Representation"
                        // Self-closing <Representation .../> — inherits AdaptationSet template.
                        // urls.is_empty() guard: Tidal manifests contain a single audio stream;
                        // we take the first Representation and skip the rest.
                        if in_adaptation_set && urls.is_empty() => {
                            let mut r_id = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => r_id = String::from_utf8(attr.value.to_vec()).unwrap_or_default(),
                                    b"codecs" => codecs = Some(String::from_utf8(attr.value.to_vec()).unwrap_or_default()),
                                    b"mimeType" => mime_type = Some(String::from_utf8(attr.value.to_vec()).unwrap_or_default()),
                                    b"audioSamplingRate" => {
                                        if let Ok(s) = std::str::from_utf8(&attr.value) {
                                            sample_rate = s.parse().ok();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(ref tmpl) = adapt_media_template {
                                if !adapt_timeline.is_empty() {
                                    if let Some(ref init) = adapt_init_url {
                                        urls.push(expand_static(init, &r_id));
                                    }
                                    let mut seg_num = adapt_start_number;
                                    for (_d, r) in &adapt_timeline {
                                        for _ in 0..=*r {
                                            urls.push(expand_tmpl(tmpl, &r_id, seg_num));
                                            seg_num += 1;
                                        }
                                    }
                                } else {
                                    if let Some(ref init) = adapt_init_url {
                                        urls.push(expand_static(init, &r_id));
                                    }
                                    for i in adapt_start_number..(adapt_start_number + 200) {
                                        urls.push(expand_tmpl(tmpl, &r_id, i));
                                    }
                                }
                            }
                        }
                    b"SegmentTemplate" => {
                        let (init, media, start) = read_segment_template_attrs(e);
                        if in_representation {
                            if let Some(v) = init { rep_init_url = Some(v); }
                            if let Some(v) = media { rep_media_template = Some(v); }
                            if let Some(n) = start { rep_start_number = n; }
                        } else if in_adaptation_set {
                            if let Some(v) = init { adapt_init_url = Some(v); }
                            if let Some(v) = media { adapt_media_template = Some(v); }
                            if let Some(n) = start { adapt_start_number = n; }
                        }
                    }
                    b"S"
                        if in_segment_timeline => {
                            let (d, r) = read_s_attrs(e);
                            if segment_timeline_in_adapt {
                                adapt_timeline.push((d, r));
                            } else {
                                rep_timeline.push((d, r));
                            }
                        }
                    b"SegmentURL"
                        if in_segment_list => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"media"
                                    && let Ok(v) = attr.unescape_value() {
                                        rep_segment_list.push(v.into_owned());
                                    }
                            }
                        }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_base_url
                    && let Ok(decoded) = e.decode() {
                        let raw = decoded.as_ref();
                        let unescaped = quick_xml::escape::unescape(raw)
                            .unwrap_or(std::borrow::Cow::Borrowed(raw));
                        let trimmed = unescaped.trim();
                        if !trimmed.is_empty() {
                            rep_base_url = Some(trimmed.to_string());
                        }
                    }
            }
            Ok(Event::End(ref e)) => {
                let local_name_owned = e.name().local_name().as_ref().to_vec();
                let local: &[u8] = &local_name_owned;
                match local {
                    b"Period" => in_period = false,
                    b"AdaptationSet" => {
                        in_adaptation_set = false;
                        adapt_init_url = None;
                        adapt_media_template = None;
                        adapt_start_number = 1;
                        adapt_timeline.clear();
                    }
                    b"Representation" => {
                        in_representation = false;
                        // Build URLs from this Representation if we haven't yet.
                        // Tidal manifests have a single audio stream; first Representation wins.
                        if urls.is_empty() {
                            if let Some(ref tmpl) = rep_media_template {
                                if !rep_timeline.is_empty() {
                                    if let Some(ref init) = rep_init_url {
                                        urls.push(expand_static(init, &rep_id));
                                    }
                                    let mut seg_num = rep_start_number;
                                    for (_d, r) in &rep_timeline {
                                        for _ in 0..=*r {
                                            urls.push(expand_tmpl(tmpl, &rep_id, seg_num));
                                            seg_num += 1;
                                        }
                                    }
                                } else {
                                    // No timeline — generate up to 200 segments.
                                    if let Some(ref init) = rep_init_url {
                                        urls.push(expand_static(init, &rep_id));
                                    }
                                    for i in rep_start_number..(rep_start_number + 200) {
                                        urls.push(expand_tmpl(tmpl, &rep_id, i));
                                    }
                                }
                            } else if !rep_segment_list.is_empty() {
                                if let Some(ref base) = rep_base_url {
                                    urls.push(base.clone());
                                }
                                urls.append(&mut rep_segment_list);
                            } else if let Some(ref base) = rep_base_url {
                                urls.push(base.clone());
                            }
                        }
                    }
                    b"SegmentTimeline" => {
                        in_segment_timeline = false;
                        segment_timeline_in_adapt = false;
                    }
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
    fn parse_mpd_representation_id_substitution() {
        let xml = r#"<?xml version="1.0"?>
        <MPD>
            <Period>
                <AdaptationSet>
                    <Representation id="audio_flac" codecs="flac" mimeType="audio/flac">
                        <SegmentTemplate initialization="$RepresentationID$/init.mp4" media="$RepresentationID$/seg$Number$.m4s" startNumber="1">
                            <SegmentTimeline>
                                <S d="1000" r="1"/>
                            </SegmentTimeline>
                        </SegmentTemplate>
                    </Representation>
                </AdaptationSet>
            </Period>
        </MPD>"#;
        let manifest = parse_mpd(xml.as_bytes()).unwrap();
        // init + 2 segments (r=1 means 2 total)
        assert_eq!(manifest.urls.len(), 3);
        assert_eq!(manifest.urls[0], "audio_flac/init.mp4");
        assert_eq!(manifest.urls[1], "audio_flac/seg1.m4s");
        assert_eq!(manifest.urls[2], "audio_flac/seg2.m4s");
    }

    #[test]
    fn parse_mpd_adaptation_set_level_segment_template() {
        // SegmentTemplate at AdaptationSet level (common Tidal DASH pattern)
        let xml = r#"<?xml version="1.0"?>
        <MPD>
            <Period>
                <AdaptationSet>
                    <SegmentTemplate initialization="$RepresentationID$/init.mp4" media="$RepresentationID$/seg$Number$.m4s" startNumber="1">
                        <SegmentTimeline>
                            <S d="500" r="0"/>
                        </SegmentTimeline>
                    </SegmentTemplate>
                    <Representation id="rep_aac" codecs="mp4a.40.2" mimeType="audio/mp4"/>
                </AdaptationSet>
            </Period>
        </MPD>"#;
        let manifest = parse_mpd(xml.as_bytes()).unwrap();
        // init + 1 segment
        assert_eq!(manifest.urls.len(), 2);
        assert_eq!(manifest.urls[0], "rep_aac/init.mp4");
        assert_eq!(manifest.urls[1], "rep_aac/seg1.m4s");
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
