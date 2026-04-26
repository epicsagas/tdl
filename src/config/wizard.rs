use anyhow::Result;
use inquire::{Confirm, CustomType, Select, Text};

use crate::config::settings::{CoverDimensions, Quality, QualityVideo, Settings};

pub fn run(current: &Settings) -> Result<Settings> {
    println!("=== tdl Settings Wizard ===\n");

    // --- Audio quality ---
    let quality_options = vec![
        Quality::Low96k,
        Quality::Low320k,
        Quality::HighLossless,
        Quality::HiResLossless,
    ];
    let quality_labels = [
        "Low 96k (AAC 96kbps)",
        "Low 320k (AAC 320kbps)",
        "High Lossless (FLAC 16-bit/44.1kHz)",
        "HiRes Lossless (FLAC up to 24-bit/192kHz)",
    ];
    let quality_audio = select_enum(
        "Audio quality:",
        &quality_options,
        &quality_labels,
        &current.quality_audio,
    )?;

    // --- Video quality ---
    let video_options = vec![
        QualityVideo::P360,
        QualityVideo::P480,
        QualityVideo::P720,
        QualityVideo::P1080,
    ];
    let video_labels = ["360p", "480p", "720p", "1080p"];
    let quality_video = select_enum(
        "Video quality:",
        &video_options,
        &video_labels,
        &current.quality_video,
    )?;

    // --- Cover dimensions ---
    let cover_options = vec![
        CoverDimensions::Px80,
        CoverDimensions::Px160,
        CoverDimensions::Px320,
        CoverDimensions::Px640,
        CoverDimensions::Px1280,
    ];
    let cover_labels = ["80x80", "160x160", "320x320", "640x640", "1280x1280"];
    let metadata_cover_dimension = select_enum(
        "Cover art dimensions:",
        &cover_options,
        &cover_labels,
        &current.metadata_cover_dimension,
    )?;

    // --- Boolean toggles ---
    let skip_existing = Confirm::new("Skip already downloaded files?")
        .with_default(current.skip_existing)
        .prompt()?;

    let lyrics_embed = Confirm::new("Embed lyrics in audio files?")
        .with_default(current.lyrics_embed)
        .prompt()?;

    let lyrics_file = Confirm::new("Save lyrics as separate .lrc files?")
        .with_default(current.lyrics_file)
        .prompt()?;

    let video_download = Confirm::new("Allow video downloads?")
        .with_default(current.video_download)
        .prompt()?;

    let video_convert_mp4 = Confirm::new("Convert TS videos to MP4 (requires FFmpeg)?")
        .with_default(current.video_convert_mp4)
        .prompt()?;

    let download_delay = Confirm::new("Enable random download delay (anti-ban)?")
        .with_default(current.download_delay)
        .prompt()?;

    let metadata_cover_embed = Confirm::new("Embed cover art in audio files?")
        .with_default(current.metadata_cover_embed)
        .prompt()?;

    let cover_album_file = Confirm::new("Save cover.jpg alongside album tracks?")
        .with_default(current.cover_album_file)
        .prompt()?;

    let extract_flac = Confirm::new("Extract FLAC from MP4 containers (requires FFmpeg)?")
        .with_default(current.extract_flac)
        .prompt()?;

    let symlink_to_track = Confirm::new(
        "Symlink album/playlist tracks to track directory?",
    )
    .with_default(current.symlink_to_track)
    .prompt()?;

    let playlist_create = Confirm::new("Create .m3u playlist files?")
        .with_default(current.playlist_create)
        .prompt()?;

    let metadata_replay_gain = Confirm::new("Write ReplayGain metadata?")
        .with_default(current.metadata_replay_gain)
        .prompt()?;

    // --- Numbers ---
    let downloads_concurrent_max = CustomType::<usize>::new("Max concurrent downloads (1-5):")
        .with_default(current.downloads_concurrent_max)
        .with_error_message("Enter a number between 1 and 5")
        .prompt()?;

    let downloads_simultaneous_per_track_max =
        CustomType::<usize>::new("Max parallel segments per track:")
            .with_default(current.downloads_simultaneous_per_track_max)
            .with_error_message("Enter a positive number")
            .prompt()?;

    let album_track_num_pad_min = CustomType::<u32>::new(
        "Min track number padding (1=no pad, 2=01, 3=001, 4=0001):",
    )
    .with_default(current.album_track_num_pad_min)
    .with_error_message("Enter 1-4")
    .prompt()?;

    let download_delay_sec_min = CustomType::<f64>::new("Min download delay (seconds):")
        .with_default(current.download_delay_sec_min)
        .with_error_message("Enter a valid number")
        .prompt()?;

    let download_delay_sec_max = CustomType::<f64>::new("Max download delay (seconds):")
        .with_default(current.download_delay_sec_max)
        .with_error_message("Enter a valid number")
        .prompt()?;

    // --- Text inputs ---
    let download_base_path = Text::new("Download base path:")
        .with_default(&current.download_base_path)
        .with_help_message("e.g. ~/download or /mnt/music")
        .prompt()?;

    let path_binary_ffmpeg = Text::new("FFmpeg path (leave empty for auto-detect):")
        .with_default(&current.path_binary_ffmpeg)
        .prompt()?;

    // --- Format templates ---
    println!("\n--- Path Templates ---");
    println!("Available: {{artist_name}}, {{album_artist}}, {{track_title}}, {{album_title}},");
    println!("  {{album_track_num}}, {{track_volume_num}}, {{track_volume_num_optional_CD}},");
    println!("  {{track_id}}, {{album_id}}, {{track_quality}}, {{track_explicit}},");
    println!("  {{album_explicit}}, {{isrc}}, {{album_year}}\n");

    let format_album = Text::new("Album path template:")
        .with_default(&current.format_album)
        .prompt()?;

    let format_playlist = Text::new("Playlist path template:")
        .with_default(&current.format_playlist)
        .prompt()?;

    let format_mix = Text::new("Mix path template:")
        .with_default(&current.format_mix)
        .prompt()?;

    let format_track = Text::new("Track path template:")
        .with_default(&current.format_track)
        .prompt()?;

    let format_video = Text::new("Video path template:")
        .with_default(&current.format_video)
        .prompt()?;

    let new_settings = Settings {
        skip_existing,
        lyrics_embed,
        lyrics_file,
        video_download,
        download_delay,
        download_base_path,
        quality_audio,
        quality_video,
        format_album,
        format_playlist,
        format_mix,
        format_track,
        format_video,
        video_convert_mp4,
        path_binary_ffmpeg,
        metadata_cover_dimension,
        metadata_cover_embed,
        cover_album_file,
        extract_flac,
        downloads_simultaneous_per_track_max,
        download_delay_sec_min,
        download_delay_sec_max,
        album_track_num_pad_min,
        downloads_concurrent_max,
        symlink_to_track,
        playlist_create,
        metadata_replay_gain,
    };

    // --- Confirm ---
    println!();
    let save = Confirm::new("Save these settings?")
        .with_default(true)
        .with_help_message("Writes to ~/.config/tdl/settings.json")
        .prompt()?;

    if save {
        new_settings.save()?;
        println!("Settings saved.");
    } else {
        println!("Cancelled.");
    }

    Ok(new_settings)
}

fn select_enum<T: Clone + PartialEq>(
    message: &str,
    options: &[T],
    labels: &[&str],
    current: &T,
) -> Result<T> {
    let starting_cursor = options
        .iter()
        .position(|o| o == current)
        .unwrap_or(0);

    let items: Vec<String> = labels.iter().map(|l| l.to_string()).collect();
    let ans = Select::new(message, items)
        .with_starting_cursor(starting_cursor)
        .prompt()?;

    let idx = labels.iter().position(|l| *l == ans).unwrap_or(0);
    Ok(options[idx].clone())
}
