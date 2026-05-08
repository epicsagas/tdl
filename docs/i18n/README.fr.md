<div align="center">

# tdl

> Téléchargeur de musique Tidal avec qualité sans perte — CLI, TUI et GUI

[![GitHub Release](https://img.shields.io/github/v/release/epicsagas/tdl)](https://github.com/epicsagas/tdl/releases)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <strong>Français</strong> | <a href="README.de.md">Deutsch</a> | <a href="README.pt.md">Português</a> | <a href="README.ru.md">Русский</a> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

## Démarrage Rapide

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Binaire préconstruit (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # Flux d'appareil OAuth
tdl dl <tidal-url>     # Télécharger piste/album/playlist
```

## Fonctionnalités

| | Fonctionnalité | Pourquoi c'est important |
|--|---------|----------------|
| 🎵 | **Qualité sans perte** | Support HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **Trois interfaces** | CLI, TUI et GUI — choisissez votre style |
| ⚡ | **Téléchargements parallèles** | Récupération concurrente de segments avec réessai |
| 🏷️ | **Balises de métadonnées** | Balises FLAC/M4A avec ReplayGain, paroles, pochettes |
| 🔄 | **Connexion PKCE** | OAuth sécurisé pour la qualité HiRes |
| 📺 | **Support vidéo** | Télécharge et convertit les vidéos musicales en MP4 |
| 🎨 | **TUI et GUI** | Parcourir les favoris, rechercher, télécharger de manière interactive |

## Installation

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Binaire Préconstruit

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### Depuis Source

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binaire: target/release/tdl
```

## Utilisation

### Connexion

OAuth standard (jusqu'à qualité HiFi):

```bash
tdl login
```

Flux PKCE pour HiRes Lossless:

```bash
tdl login --pkce
```

### Télécharger

```bash
# Piste
tdl dl https://tidal.com/browse/track/12345

# Album / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Plusieurs URLs
tdl dl <url1> <url2>

# Depuis fichier
tdl dl --list urls.txt
```

### TUI et GUI

```bash
# Interface terminal
tdl tui

# GUI (Tauri)
tdl gui
# ou simplement
tdl
```

### Configuration

Fichier de configuration: `~/.tdl/settings.json`

```bash
# Assistant interactif
tdl cfg

# Ouvrir dans éditeur
tdl cfg --editor
```

| Paramètre | Par défaut | Description |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Répertoire racine |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Remplir les numéros de piste avec des zéros |
| `playlist_folder` | `true` | Sauvegarder les playlists sous `Playlists/` |
| `skip_existing` | `true` | Sauter les fichiers existants |
| `extract_flac` | `true` | Extraire FLAC de M4A/MP4 |
| `video_convert_mp4` | `true` | Convertir TS en MP4 |

## Pourquoi tdl ?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Qualité audio | ✅ HiRes Lossless | ⚠️ Jusqu'à 320kbps | ⚠️ Variable |
| Support vidéo | ✅ Natif | ❌ | ✅ |
| TUI/GUI | ✅ Les deux | ❌ | ❌ |
| Métadonnées | ✅ Complet | ⚠️ Basique | ⚠️ Basique |
| Paroles | ✅ Synchronisées | ✅ | ⚠️ Parfois |
| Plan gratuit | ❌ Nécessite abonnement | ❌ Nécessite abonnement | ✅ Oui |

## Structure de Sortie

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # quand playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Configuration Requise

- **OS**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (lors de la construction depuis source)
- **FFmpeg**: Optionnel — pour conversion vidéo et extraction FLAC
- **Tidal**: Abonnement Premium, HiFi ou HiFi Plus

## Dépannage

<details>
<summary>commande non trouvée après installation</summary>

Ajoutez le chemin d'installation à votre PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Installation locale
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>Erreur FFmpeg non trouvé</summary>

Installez FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Contribuer

Voir [CONTRIBUTING.md](../../CONTRIBUTING.md). PRs bienvenus — consultez les issues étiquetées `good first issue`.

## Licence

[Apache-2.0](../../LICENSE) © 2025
