<div align="center">

[![CI](https://github.com/epicsagas/tdl/actions/workflows/ci.yml/badge.svg)](https://github.com/epicsagas/tdl/actions/workflows/ci.yml)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.de.md">Deutsch</a> | <a href="README.pt.md">Português</a> | <a href="README.ru.md">Русский</a> | <strong>Italiano</strong>
</p>

# tdl

> Downloader musicale Tidal con qualità senza perdita — CLI, TUI e GUI

</div>

## Avvio Rapido

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Binario precompilato (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # Flusso dispositivo OAuth
tdl dl <tidal-url>     # Scarica traccia/album/playlist
```

## Funzionalità

| | Funzionalità | Perché è importante |
|--|---------|----------------|
| 🎵 | **Qualità senza perdita** | Supporto HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **Tre interfacce** | CLI, TUI e GUI — scegli il tuo stile |
| ⚡ | **Download paralleli** | Recupero simultaneo dei segmenti con riprov |
| 🏷️ | **Tag metadati** | Tag FLAC/M4A con ReplayGain, testi, copertine |
| 🔄 | **Login PKCE** | OAuth sicuro per qualità HiRes |
| 📺 | **Supporto video** | Scarica e converte video musicali in MP4 |
| 🎨 | **TUI e GUI** | Sfoglia preferiti, cerca, download interattivo |

## Installazione

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Binario Precompilato

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### Dal Codice Sorgente

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binario: target/release/tdl
```

## Utilizzo

### Accesso

OAuth standard (fino a qualità HiFi):

```bash
tdl login
```

Flusso PKCE per HiRes Lossless:

```bash
tdl login --pkce
```

### Download

```bash
# Traccia
tdl dl https://tidal.com/browse/track/12345

# Album / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# URL multipli
tdl dl <url1> <url2>

# Da file
tdl dl --list urls.txt
```

### TUI e GUI

```bash
# Interfaccia terminale
tdl tui

# GUI (Tauri)
tdl gui
# o semplicemente
tdl
```

### Configurazione

File di configurazione: `~/.tdl/settings.json`

```bash
# Procedura guidata interattiva
tdl cfg

# Apri nell'editor
tdl cfg --editor
```

| Impostazione | Predefinito | Descrizione |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Directory radice |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Riempimento zero numeri traccia |
| `playlist_folder` | `true` | Salva playlist sotto `Playlists/` |
| `skip_existing` | `true` | Salta file esistenti |
| `extract_flac` | `true` | Estrai FLAC da M4A/MP4 |
| `video_convert_mp4` | `true` | Converti TS in MP4 |

## Perché tdl?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Qualità audio | ✅ HiRes Lossless | ⚠️ Fino a 320kbps | ⚠️ Variabile |
| Supporto video | ✅ Nativo | ❌ | ✅ |
| TUI/GUI | ✅ Entrambi | ❌ | ❌ |
| Metadati | ✅ Completi | ⚠️ Base | ⚠️ Base |
| Testi | ✅ Sincronizzati | ✅ | ⚠️ A volte |
| Piano gratuito | ❌ Richiede abbonamento | ❌ Richiede abbonamento | ✅ Sì |

## Struttura di Output

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # quando playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Requisiti

- **OS**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (durante la compilazione dal codice sorgente)
- **FFmpeg**: Opzionale — per conversione video ed estrazione FLAC
- **Tidal**: Abbonamento Premium, HiFi o HiFi Plus

## Risoluzione dei Problemi

<details>
<summary>comando non trovato dopo l'installazione</summary>

Aggiungi il percorso di installazione al tuo PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Installazione locale
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>Errore FFmpeg non trovato</summary>

Installa FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Contribuire

Vedi [CONTRIBUTING.md](../../CONTRIBUTING.md). PR benvenuti — controlla issue etichettati `good first issue`.

## Licenza

[Apache-2.0](../../LICENSE) © 2025
