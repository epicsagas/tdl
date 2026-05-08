<div align="center">

# tdl

> Downloader de música Tidal com qualidade sem perdas — CLI, TUI e GUI

[![GitHub Release](https://img.shields.io/github/v/release/epicsagas/tdl)](https://github.com/epicsagas/tdl/releases)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.de.md">Deutsch</a> | <strong>Português</strong> | <a href="README.ru.md">Русский</a> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

> **AVISO: A distribuição não autorizada de música protegida por direitos autorais é ilegal.**
> Esta ferramenta é apenas para uso pessoal. Não compartilhe, redistribua ou publique o conteúdo baixado. Respeite os artistas e as leis de direitos autorais.

## Início Rápido

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Binário pré-compilado (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # Fluxo de dispositivo OAuth
tdl dl <tidal-url>     # Baixar faixa/álbum/playlist
```

## Recursos

| | Recurso | Por que importa |
|--|---------|----------------|
| 🎵 | **Qualidade sem perdas** | Suporte HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **Três interfaces** | CLI, TUI e GUI — escolha seu estilo |
| ⚡ | **Downloads paralelos** | Busca concorrente de segmentos com retentativa |
| 🏷️ | **Marcação de metadados** | Tags FLAC/M4A com ReplayGain, letras, arte de capa |
| 🔄 | **Login PKCE** | OAuth seguro para qualidade HiRes |
| 📺 | **Suporte a vídeo** | Baixa e converte vídeos de música para MP4 |
| 🎨 | **TUI e GUI** | Navegue favoritos, pesquise, baixe de forma interativa |

## Instalação

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Binário Pré-compilado

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### A Partir do Fonte

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Binário: target/release/tdl
```

## Uso

### Login

OAuth padrão (até qualidade HiFi):

```bash
tdl login
```

Fluxo PKCE para HiRes Lossless:

```bash
tdl login --pkce
```

### Baixar

```bash
# Faixa
tdl dl https://tidal.com/browse/track/12345

# Álbum / Playlist / Mix
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Múltiplas URLs
tdl dl <url1> <url2>

# De arquivo
tdl dl --list urls.txt
```

### TUI e GUI

```bash
# Interface de terminal
tdl tui

# GUI (Tauri)
tdl gui
# ou apenas
tdl
```

### Configuração

Arquivo de configuração: `~/.tdl/settings.json`

```bash
# Assistente interativo
tdl cfg

# Abrir no editor
tdl cfg --editor
```

| Configuração | Padrão | Descrição |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Diretório raiz |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Preencher números de faixa com zeros |
| `playlist_folder` | `true` | Salvar playlists sob `Playlists/` |
| `skip_existing` | `true` | Pular arquivos existentes |
| `extract_flac` | `true` | Extrair FLAC de M4A/MP4 |
| `video_convert_mp4` | `true` | Converter TS para MP4 |

## Por que tdl?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Qualidade de áudio | ✅ HiRes Lossless | ⚠️ Até 320kbps | ⚠️ Variável |
| Suporte a vídeo | ✅ Nativo | ❌ | ✅ |
| TUI/GUI | ✅ Ambos | ❌ | ❌ |
| Metadados | ✅ Completo | ⚠️ Básico | ⚠️ Básico |
| Letras | ✅ Sincronizadas | ✅ | ⚠️ Às vezes |
| Plano gratuito | ❌ Requer assinatura | ❌ Requer assinatura | ✅ Sim |

## Estrutura de Saída

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

## Requisitos

- **SO**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (ao compilar a partir do fonte)
- **FFmpeg**: Opcional — para conversão de vídeo e extração FLAC
- **Tidal**: Assinatura Premium, HiFi ou HiFi Plus

## Solução de Problemas

<details>
<summary>comando não encontrado após instalar</summary>

Adicione o caminho de instalação ao seu PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Instalação local
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>Erro FFmpeg não encontrado</summary>

Instale o FFmpeg:

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

Veja [CONTRIBUTING.md](../../CONTRIBUTING.md). PRs bem-vindos — confira issues etiquetados com `good first issue`.

## Licença

[Apache-2.0](../../LICENSE) © 2025
