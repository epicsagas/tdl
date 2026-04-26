# tdl

Tidal music downloader written in Rust. A CLI port of the Python [tidal-dl-ng](https://github.com/exislow/tidal-dl-ng) project.

## Features

- OAuth device authorization & PKCE login (HiRes Lossless)
- Download tracks, albums, playlists, mixes, and videos
- AES-256-CBC token decryption + AES-128-CTR file decryption
- BTS JSON and MPEG-DASH (MPD) manifest parsing
- Parallel segment downloading with retry and exponential backoff
- FLAC, M4A, and MP3 metadata tagging (lofty)
- ReplayGain tag support
- Cover art embedding and standalone file
- LRC lyrics file export
- Configurable path templates with `{placeholder}` substitution
- TS to MP4 conversion via FFmpeg
- FLAC extraction from MP4 containers
- Download favorites (tracks, albums, artists, videos)

## Requirements

- Rust 1.70+ (edition 2021)
- FFmpeg (optional, for video conversion and FLAC extraction)
- A Tidal account

## Installation

```sh
cargo install --path .
```

Or build a release binary:

```sh
cargo build --release
# binary at target/release/tdl
```

## Usage

### Login

Standard OAuth device flow:

```sh
tdl login
```

For HiRes Lossless (PKCE):

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
tdl dl https://tidal.com/track/12345

# Album
tdl dl https://tidal.com/album/67890

# Multiple URLs
tdl dl https://tidal.com/track/111 https://tidal.com/album/222

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
# Interactive settings editor
tdl cfg

# Show a specific setting
tdl cfg quality_audio

# Edit settings in your editor
tdl cfg --editor
```

Settings are stored at `~/.config/tdl/settings.json`.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `skip_existing` | `true` | Skip already-downloaded files |
| `quality_audio` | `low_320k` | Audio quality: `low_96k`, `low_320k`, `high_lossless`, `hi_res_lossless` |
| `quality_video` | `p480` | Video quality: `p360`, `p480`, `p720`, `p1080` |
| `download_base_path` | `~/download` | Base download directory |
| `download_delay` | `true` | Add random delay between downloads |
| `download_delay_sec_min` | `3.0` | Minimum delay in seconds |
| `download_delay_sec_max` | `5.0` | Maximum delay in seconds |
| `downloads_simultaneous_per_track_max` | `20` | Max parallel segments per track |
| `extract_flac` | `true` | Extract FLAC from MP4 when codec is FLAC |
| `video_convert_mp4` | `true` | Convert TS video to MP4 via FFmpeg |
| `metadata_cover_embed` | `true` | Embed cover art in audio files |
| `metadata_cover_dimension` | `px320` | Cover resolution: `px80` to `px1280` |
| `cover_album_file` | `true` | Save `cover.jpg` alongside tracks |
| `lyrics_embed` | `false` | Embed lyrics in audio files |
| `lyrics_file` | `false` | Save `.lrc` lyrics files |
| `metadata_replay_gain` | `true` | Write ReplayGain tags |
| `playlist_create` | `false` | Create `.m3u` playlist files |
| `path_binary_ffmpeg` | `""` | Custom FFmpeg path (empty = system PATH) |

### Path Templates

Customize output paths with placeholders:

| Placeholder | Description |
|-------------|-------------|
| `{artist_name}` | Track artist |
| `{album_artist}` | Album artist |
| `{track_title}` | Track title |
| `{album_title}` | Album name |
| `{album_track_num}` | Track number |
| `{track_volume_num}` | Disc/volume number |
| `{track_volume_num_optional_CD}` | Disc prefix (e.g. `CD 1/`) |
| `{track_id}` | Tidal track ID |
| `{album_id}` | Tidal album ID |
| `{track_quality}` | Audio quality |
| `{track_explicit}` | " (Explicit)" if explicit |
| `{album_explicit}` | " (Explicit)" if album is explicit |
| `{isrc}` | ISRC code |
| `{album_year}` | Release year |

## License

MIT
