<div align="center">

# tdl

> Tidal-Musikdownloader mit verlustfreier Qualität — CLI, TUI und GUI

[![CI](https://github.com/epicsagas/tdl/actions/workflows/ci.yml/badge.svg)](https://github.com/epicsagas/tdl/actions/workflows/ci.yml)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <strong>Deutsch</strong> | <a href="README.pt.md">Português</a> | <a href="README.ru.md">Русский</a> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

## Schnellstart

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Vorkompiliertes Binary (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # OAuth-Geräteablauf
tdl dl <tidal-url>     # Track/Album/Playlist herunterladen
```

## Funktionen

| | Funktion | Warum es wichtig ist |
|--|---------|----------------|
| 🎵 | **Verlustfreie Qualität** | HiRes Lossless (24-bit/192kHz) Unterstützung |
| 🖥️ | **Drei Schnittstellen** | CLI, TUI und GUI — wählen Sie Ihren Stil |
| ⚡ | **Parallele Downloads** | Gleichzeitige Segment-Abfrage mit Wiederholung |
| 🏷️ | **Metadaten-Tags** | FLAC/M4A-Tags mit ReplayGain, Liedtexten, Cover-Art |
| 🔄 | **PKCE-Login** | Sicherer OAuth für HiRes-Qualität |
| 📺 | **Video-Unterstützung** | Musikvideos herunterladen und in MP4 konvertieren |
| 🎨 | **TUI und GUI** | Favoriten durchsuchen, Suchen, interaktives Herunterladen |

## Installation

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Vorkompiliertes Binary

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### Aus Quelle

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binary: target/release/tdl
```

## Verwendung

### Anmelden

Standard OAuth (bis HiFi-Qualität):

```bash
tdl login
```

PKCE-Fluss für HiRes Lossless:

```bash
tdl login --pkce
```

### Herunterladen

```bash
# Track
tdl dl https://tidal.com/browse/track/12345

# Album / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Mehrere URLs
tdl dl <url1> <url2>

# Aus Datei
tdl dl --list urls.txt
```

### TUI und GUI

```bash
# Terminal-Oberfläche
tdl tui

# GUI (Tauri)
tdl gui
# oder einfach
tdl
```

### Konfiguration

Konfigurationsdatei: `~/.tdl/settings.json`

```bash
# Interaktiver Assistent
tdl cfg

# In Editor öffnen
tdl cfg --editor
```

| Einstellung | Standard | Beschreibung |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Stammverzeichnis |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Tracknummern mit Nullen auffüllen |
| `playlist_folder` | `true` | Playlists unter `Playlists/` speichern |
| `skip_existing` | `true` | Vorhandene Dateien überspringen |
| `extract_flac` | `true` | FLAC aus M4A/MP4 extrahieren |
| `video_convert_mp4` | `true` | TS in MP4 konvertieren |

## Warum tdl?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Audioqualität | ✅ HiRes Lossless | ⚠️ Bis zu 320kbps | ⚠️ Variabel |
| Video-Unterstützung | ✅ Nativ | ❌ | ✅ |
| TUI/GUI | ✅ Beides | ❌ | ❌ |
| Metadaten | ✅ Vollständig | ⚠️ Basis | ⚠️ Basis |
| Liedtexte | ✅ Synchronisiert | ✅ | ⚠️ Manchmal |
| Gratis-Plan | ❌ Abonnement erforderlich | ❌ Abonnement erforderlich | ✅ Ja |

## Ausgabestruktur

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # wenn playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Voraussetzungen

- **BS**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (bei Kompilierung aus Quelle)
- **FFmpeg**: Optional — für Videokonvertierung und FLAC-Extraktion
- **Tidal**: Premium-, HiFi- oder HiFi-Plus-Abonnement

## Fehlerbehebung

<details>
<summary>Befehl nach Installation nicht gefunden</summary>

Installationspfad zu PATH hinzufügen:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Lokale Installation
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>FFmpeg nicht gefunden Fehler</summary>

FFmpeg installieren:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Mitwirken

Siehe [CONTRIBUTING.md](../../CONTRIBUTING.md). PRs willkommen — siehe Issues mit `good first issue` Label.

## Lizenz

[Apache-2.0](../../LICENSE) © 2025
