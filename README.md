# tdl

Tidal music downloader written in Rust.

## Features

- OAuth device authorization & PKCE login (required for HiRes Lossless)
- Download tracks, albums, playlists, mixes, and videos
- AES-128-CTR file decryption
- BTS JSON and MPEG-DASH (MPD) manifest parsing
- Parallel segment downloading with retry and exponential backoff
- FLAC, M4A metadata tagging (lofty)
- ReplayGain tag support
- Cover art embedding and standalone `cover.jpg`
- Synced lyrics (`.lrc`) export and embedding
- TS to MP4 conversion via FFmpeg
- FLAC extraction from MP4 containers
- Browse and download favorites (tracks, albums, artists, videos)
- Three interfaces: CLI, TUI, and GUI (Tauri)

## Requirements

- Rust 1.80+
- FFmpeg (optional — required for video conversion and FLAC extraction)
- A Tidal subscription

## Installation

```sh
cargo build --release
# binary: target/release/tdl
```

The `gui` feature is included by default. To build without it:

```sh
cargo build --release --no-default-features
```

## Usage

### Login

Standard OAuth device flow:

```sh
tdl login
```

For HiRes Lossless (PKCE flow):

```sh
tdl login --pkce
```

### Logout

```sh
tdl logout
```

### Download

```sh
# Single track
tdl dl https://tidal.com/browse/track/12345

# Album
tdl dl https://tidal.com/browse/album/67890

# Playlist or mix
tdl dl https://tidal.com/browse/playlist/abc-uuid
tdl dl https://tidal.com/browse/mix/abc123

# Artist (all albums)
tdl dl https://tidal.com/browse/artist/999

# Multiple URLs
tdl dl https://tidal.com/browse/track/111 https://tidal.com/browse/album/222

# From a file (one URL per line, # comments supported)
tdl dl --list urls.txt
```

### Favorites

```sh
tdl fav tracks
tdl fav albums
tdl fav artists
tdl fav videos
```

### Settings

```sh
# Interactive wizard
tdl cfg

# Open settings file in $EDITOR
tdl cfg --editor
```

Settings are stored at `~/.tdl/settings.json`.

### TUI

```sh
tdl tui
```

### GUI

```sh
tdl gui
# or just
tdl
```

## Output structure

All files are saved under `download_base_path` (default `~/download`):

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               ← only when playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Configuration

Settings file: `~/.tdl/settings.json`

| Setting | Default | Description |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Root directory for all downloads |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Zero-pad track numbers (`01`, `02`, …) |
| `playlist_folder` | `true` | Save playlists/mixes under `Playlists/` and generate `.m3u` |
| `skip_existing` | `true` | Skip files that already exist |
| `download_delay` | `true` | Add a random delay between track downloads |
| `download_delay_sec_min` | `3.0` | Minimum delay (seconds) |
| `download_delay_sec_max` | `5.0` | Maximum delay (seconds) |
| `downloads_concurrent_max` | `3` | Max concurrent collection downloads |
| `downloads_simultaneous_per_track_max` | `20` | Max parallel segments per track |
| `extract_flac` | `true` | Extract FLAC from M4A/MP4 containers |
| `video_download` | `true` | Enable video downloads |
| `video_convert_mp4` | `true` | Convert TS video to MP4 via FFmpeg |
| `path_binary_ffmpeg` | `""` | FFmpeg path (empty = auto-detect) |
| `metadata_cover_embed` | `true` | Embed cover art in audio files |
| `metadata_cover_dimension` | `px320` | Cover resolution: `px80` / `px160` / `px320` / `px640` / `px1280` |
| `cover_album_file` | `true` | Save `cover.jpg` in each album folder |
| `lyrics_embed` | `false` | Embed synced lyrics in audio files |
| `lyrics_file` | `false` | Save `.lrc` lyrics files |
| `metadata_replay_gain` | `true` | Write ReplayGain tags |
| `symlink_to_track` | `false` | Symlink collection tracks to track directory |

## License

[APACHE-2](LICENSE)
