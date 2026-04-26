use anyhow::{anyhow, Result};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use std::path::Path;

/// Metadata to write to an audio file.
#[derive(Debug, Clone, Default)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub artists: Option<Vec<String>>,
    pub copyright: Option<String>,
    pub track_number: Option<u32>,
    pub total_tracks: Option<u32>,
    pub disc_number: Option<u32>,
    pub total_discs: Option<u32>,
    pub date: Option<String>,
    pub composer: Option<String>,
    pub isrc: Option<String>,
    pub lyrics: Option<String>,
    pub url: Option<String>,
    pub cover_data: Option<Vec<u8>>,
    pub album_replay_gain: Option<f64>,
    pub album_peak_amplitude: Option<f64>,
    pub track_replay_gain: Option<f64>,
    pub track_peak_amplitude: Option<f64>,
    pub write_replay_gain: bool,
}

/// Determine the tag type from the file extension.
fn tag_type_from_ext(path: &Path) -> Result<TagType> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| anyhow!("File has no extension: {}", path.display()))?;

    match ext.as_str() {
        "flac" => Ok(TagType::VorbisComments),
        "m4a" | "mp4" => Ok(TagType::Mp4Ilst),
        "mp3" => Ok(TagType::Id3v2),
        other => Err(anyhow!("Unsupported audio format: '{}'", other)),
    }
}

/// Detect the MIME type of cover art from its binary header.
fn cover_mime(data: &[u8]) -> Option<MimeType> {
    if data.len() < 4 {
        return None;
    }
    // PNG signature: 89 50 4E 47
    if data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        return Some(MimeType::Png);
    }
    // JPEG signature: FF D8 FF
    if data[0..3] == [0xFF, 0xD8, 0xFF] {
        return Some(MimeType::Jpeg);
    }
    None
}

/// Format a replay gain value as a string with " dB" suffix.
fn format_replay_gain(value: f64) -> String {
    format!("{:.2} dB", value)
}

/// Format a peak amplitude value as a string.
fn format_peak(value: f64) -> String {
    format!("{:.6}", value)
}

/// Populate a tag with the common metadata fields.
fn populate_tag(tag: &mut Tag, meta: &AudioMetadata, _tag_type: TagType) {
    if let Some(ref v) = meta.title {
        tag.insert_text(ItemKey::TrackTitle, v.clone());
    }
    if let Some(ref v) = meta.album {
        tag.insert_text(ItemKey::AlbumTitle, v.clone());
    }
    if let Some(ref v) = meta.album_artist {
        tag.insert_text(ItemKey::AlbumArtist, v.clone());
    }
    if let Some(ref artists) = meta.artists {
        // For multi-artist support, use TrackArtists (multiple values) alongside TrackArtist.
        if let Some(first) = artists.first() {
            tag.insert_text(ItemKey::TrackArtist, first.clone());
        }
        // Remove any existing TrackArtists entries before pushing new ones.
        tag.retain(|item| item.key() != ItemKey::TrackArtists);
        for artist in artists {
            tag.push_unchecked(TagItem::new(
                ItemKey::TrackArtists,
                ItemValue::Text(artist.clone()),
            ));
        }
    } else {
        // If no artists list, clear TrackArtist to avoid stale data.
        // Only clear if there is no explicit artist override.
    }
    if let Some(ref v) = meta.copyright {
        tag.insert_text(ItemKey::CopyrightMessage, v.clone());
    }
    if let Some(v) = meta.track_number {
        tag.insert_text(ItemKey::TrackNumber, v.to_string());
    }
    if let Some(v) = meta.total_tracks {
        tag.insert_text(ItemKey::TrackTotal, v.to_string());
    }
    if let Some(v) = meta.disc_number {
        tag.insert_text(ItemKey::DiscNumber, v.to_string());
    }
    if let Some(v) = meta.total_discs {
        tag.insert_text(ItemKey::DiscTotal, v.to_string());
    }
    if let Some(ref v) = meta.date {
        // Use RecordingDate for the date field, which maps to TDRC in ID3v2,
        // DATE in VorbisComments, and \u{a9}day in MP4.
        tag.insert_text(ItemKey::RecordingDate, v.clone());
    }
    if let Some(ref v) = meta.composer {
        tag.insert_text(ItemKey::Composer, v.clone());
    }
    if let Some(ref v) = meta.isrc {
        tag.insert_text(ItemKey::Isrc, v.clone());
    }
    if let Some(ref v) = meta.lyrics {
        tag.insert_text(ItemKey::Lyrics, v.clone());
    }
    if let Some(ref v) = meta.url {
        // Use AudioSourceUrl as a generic URL tag.
        tag.insert_text(ItemKey::AudioSourceUrl, v.clone());
    }

    // Cover art
    if let Some(ref cover_data) = meta.cover_data {
        // Remove existing front cover pictures to avoid duplicates.
        tag.remove_picture_type(PictureType::CoverFront);

        let _mime = cover_mime(cover_data);
        let pic = Picture::unchecked(cover_data.clone()).build();
        tag.push_picture(pic);
    }

    // Replay gain (only written when explicitly requested)
    if meta.write_replay_gain {
        if let Some(v) = meta.album_replay_gain {
            tag.insert_text(
                ItemKey::ReplayGainAlbumGain,
                format_replay_gain(v),
            );
        }
        if let Some(v) = meta.album_peak_amplitude {
            tag.insert_text(
                ItemKey::ReplayGainAlbumPeak,
                format_peak(v),
            );
        }
        if let Some(v) = meta.track_replay_gain {
            tag.insert_text(
                ItemKey::ReplayGainTrackGain,
                format_replay_gain(v),
            );
        }
        if let Some(v) = meta.track_peak_amplitude {
            tag.insert_text(
                ItemKey::ReplayGainTrackPeak,
                format_peak(v),
            );
        }
    }
}

/// Write metadata to a FLAC file (Vorbis Comments).
fn write_flac(path: &Path, meta: &AudioMetadata) -> Result<()> {
    write_common(path, meta, TagType::VorbisComments)
}

/// Write metadata to an MP4/M4A file (iTunes-style atoms).
fn write_mp4(path: &Path, meta: &AudioMetadata) -> Result<()> {
    write_common(path, meta, TagType::Mp4Ilst)
}

/// Write metadata to an MP3 file (ID3v2 tags).
fn write_mp3(path: &Path, meta: &AudioMetadata) -> Result<()> {
    write_common(path, meta, TagType::Id3v2)
}

/// Common writer that opens the file, ensures the right tag type exists,
/// populates it, and saves.
fn write_common(path: &Path, meta: &AudioMetadata, tag_type: TagType) -> Result<()> {
    let mut tagged_file = Probe::open(path)
        .map_err(|e| anyhow!("Failed to open file '{}': {}", path.display(), e))?
        .read()
        .map_err(|e| anyhow!("Failed to read file '{}': {}", path.display(), e))?;

    // Ensure a tag of the correct type exists.
    let needs_insert = tagged_file.tag(tag_type).is_none();
    if needs_insert {
        tagged_file.insert_tag(Tag::new(tag_type));
    }

    let tag = tagged_file
        .tag_mut(tag_type)
        .ok_or_else(|| anyhow!("Tag not found after insertion for '{}'", path.display()))?;

    populate_tag(tag, meta, tag_type);

    tag.save_to_path(path, WriteOptions::new())
        .map_err(|e| anyhow!("Failed to save tags to '{}': {}", path.display(), e))?;

    Ok(())
}

/// Write metadata to an audio file based on its extension.
pub fn write_metadata(path: &Path, meta: &AudioMetadata) -> Result<()> {
    let tag_type = tag_type_from_ext(path)?;

    match tag_type {
        TagType::VorbisComments => write_flac(path, meta),
        TagType::Mp4Ilst => write_mp4(path, meta),
        TagType::Id3v2 => write_mp3(path, meta),
        other => Err(anyhow!(
            "Unsupported tag type {:?} for file '{}'",
            other,
            path.display()
        )),
    }
}
