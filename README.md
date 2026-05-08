<div align="center">

**[English](README.md)** | [한국어](docs/i18n/README.ko.md) | [日本語](docs/i18n/README.ja.md) | [简体中文](docs/i18n/README.zh-CN.md) | [Español](docs/i18n/README.es.md) | [Français](docs/i18n/README.fr.md) | [Deutsch](docs/i18n/README.de.md) | [Português](docs/i18n/README.pt.md) | [Русский](docs/i18n/README.ru.md) | [Italiano](docs/i18n/README.it.md)

# tdl

> Tidal music downloader with lossless quality — CLI, TUI, and GUI

[![CI](https://github.com/epicsagas/tdl/actions/workflows/ci.yml/badge.svg)](https://github.com/epicsagas/tdl/actions/workflows/ci.yml)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

</div>

<img src="docs/assets/favorites.png" alt="favorites gui" width="100%" />

## Quick Start

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Pre-built binary (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Fast path (downloads prebuilt binary when available)
cargo binstall tdl
cargo install cargo-binstall # Install cargo-binstall if missing

# Or just
cargo install tdl

# From source
cargo install tdl
```

```bash
tdl login              # OAuth device flow
tdl dl <tidal-url>     # Download track/album/playlist
```

## Features

| | Feature | Why it matters |
|--|---------|----------------|
| 🎵 | **Lossless quality** | HiRes Lossless (24-bit/192kHz) support |
| 🖥️ | **Three interfaces** | CLI, TUI, and GUI — choose your style |
| ⚡ | **Parallel downloads** | Concurrent segment fetching with retry |
| 🏷️ | **Metadata tagging** | FLAC/M4A tags with ReplayGain, lyrics, cover art |
| 🔄 | **PKCE login** | Secure OAuth for HiRes quality |
| 📺 | **Video support** | Download and convert music videos to MP4 |
| 🎨 | **TUI & GUI** | Browse favorites, search, download interactively |

## Installation

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Pre-built Binary

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### Cargo

```bash
cargo install tdl
```

### From Source

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binary: target/release/tdl
```

## Usage

### Login

Standard OAuth (up to HiFi quality):

```bash
tdl login
```

PKCE flow for HiRes Lossless:

```bash
tdl login --pkce
```

### Download

```bash
# Track
tdl dl https://tidal.com/browse/track/12345

# Album / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Multiple URLs
tdl dl <url1> <url2>

# From file
tdl dl --list urls.txt
```

### TUI & GUI

```bash
# Terminal UI
tdl tui

# GUI (Tauri)
tdl gui
# or just
tdl
```

### Configuration

Settings file: `~/.tdl/settings.json`

```bash
# Interactive wizard
tdl cfg

# Open in editor
tdl cfg --editor
```

| Setting | Default | Description |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Root directory |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Zero-pad track numbers |
| `playlist_folder` | `true` | Save playlists under `Playlists/` |
| `skip_existing` | `true` | Skip existing files |
| `extract_flac` | `true` | Extract FLAC from M4A/MP4 |
| `video_convert_mp4` | `true` | Convert TS to MP4 |

## Output Structure

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # when playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Requirements

- **OS**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (when building from source)
- **FFmpeg**: Optional — for video conversion and FLAC extraction
- **Tidal**: Premium, HiFi, or HiFi Plus subscription

## Troubleshooting

<details>
<summary>command not found after install</summary>

Add the install path to your PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Local install
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>FFmpeg not found error</summary>

Install FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). PRs welcome — check open issues labeled `good first issue`.

## License

[Apache-2.0](LICENSE) © 2025
