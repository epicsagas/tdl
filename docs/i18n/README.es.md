<div align="center">

# tdl

> Descargador de música de Tidal con calidad sin pérdida — CLI, TUI y GUI

[![CI](https://github.com/epicsagas/tdl/actions/workflows/ci.yml/badge.svg)](https://github.com/epicsagas/tdl/actions/workflows/ci.yml)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

## Inicio Rápido

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Binario precompilado (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Desde fuente
cargo install tdl
```

```bash
tdl login              # Flujo de dispositivo OAuth
tdl dl <tidal-url>     # Descargar pista/álbum/playlist
```

## Características

| | Característica | Por qué importa |
|--|---------|----------------|
| 🎵 | **Calidad sin pérdida** | Soporte HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **Tres interfaces** | CLI, TUI y GUI — elige tu estilo |
| ⚡ | **Descargas paralelas** | Obtención concurrente de segmentos con reintentos |
| 🏷️ | **Etiquetado de metadatos** | Etiquetas FLAC/M4A con ReplayGain, letras, arte de portada |
| 🔄 | **Login PKCE** | OAuth seguro para calidad HiRes |
| 📺 | **Soporte de video** | Descarga y convierte videos musicales a MP4 |
| 🎨 | **TUI y GUI** | Navega favoritos, busca, descarga de forma interactiva |

## Instalación

### Homebrew (macOS/Linux)

```bash
brew install epicasagas/tap/tdl
```

### Binario Precompilado

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

### Desde Fuente

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binario: target/release/tdl
```

## Uso

### Iniciar Sesión

OAuth estándar (hasta calidad HiFi):

```bash
tdl login
```

Flujo PKCE para HiRes Lossless:

```bash
tdl login --pkce
```

### Descargar

```bash
# Pista
tdl dl https://tidal.com/browse/track/12345

# Álbum / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Múltiples URLs
tdl dl <url1> <url2>

# Desde archivo
tdl dl --list urls.txt
```

### TUI y GUI

```bash
# Interfaz de terminal
tdl tui

# GUI (Tauri)
tdl gui
# o simplemente
tdl
```

### Configuración

Archivo de configuración: `~/.tdl/settings.json`

```bash
# Asistente interactivo
tdl cfg

# Abrir en editor
tdl cfg --editor
```

| Configuración | Por defecto | Descripción |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Directorio raíz |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Rellenar números de pista con ceros |
| `playlist_folder` | `true` | Guardar playlists bajo `Playlists/` |
| `skip_existing` | `true` | Saltar archivos existentes |
| `extract_flac` | `true` | Extraer FLAC de M4A/MP4 |
| `video_convert_mp4` | `true` | Convertir TS a MP4 |

## ¿Por qué tdl?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Calidad de audio | ✅ HiRes Lossless | ⚠️ Hasta 320kbps | ⚠️ Variable |
| Soporte de video | ✅ Nativo | ❌ | ✅ |
| TUI/GUI | ✅ Ambos | ❌ | ❌ |
| Metadatos | ✅ Completo | ⚠️ Básico | ⚠️ Básico |
| Letras | ✅ Sincronizadas | ✅ | ⚠️ A veces |
| Plan gratuito | ❌ Requiere suscripción | ❌ Requiere suscripción | ✅ Sí |

## Estructura de Salida

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # cuando playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Requisitos

- **SO**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (al construir desde fuente)
- **FFmpeg**: Opcional — para conversión de video y extracción FLAC
- **Tidal**: Suscripción Premium, HiFi o HiFi Plus

## Solución de Problemas

<details>
<summary>comando no encontrado después de instalar</summary>

Añade la ruta de instalación a tu PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Instalación local
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>Error de FFmpeg no encontrado</summary>

Instala FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Contribuir

Ver [CONTRIBUTING.md](../../CONTRIBUTING.md). PRs bienvenidos — revisa issues etiquetados con `good first issue`.

## Licencia

[Apache-2.0](../../LICENSE) © 2025
