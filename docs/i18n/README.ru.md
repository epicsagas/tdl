<div align="center">

# tdl

> Загрузчик музыки Tidal с потерями — CLI, TUI и GUI

[![GitHub Release](https://img.shields.io/github/v/release/epicsagas/tdl)](https://github.com/epicsagas/tdl/releases)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.de.md">Deutsch</a> | <a href="README.pt.md">Português</a> | <strong>Русский</strong> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

## Быстрый старт

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# Скомпилированный бинарный файл (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # OAuth поток устройства
tdl dl <tidal-url>     # Скачать трек/альбом/плейлист
```

## Функции

| | Функция | Почему это важно |
|--|---------|----------------|
| 🎵 | **Качество без потерь** | Поддержка HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **Три интерфейса** | CLI, TUI и GUI — выберите свой стиль |
| ⚡ | **Параллельная загрузка** | Одновременная загрузка сегментов с повтором |
| 🏷️ | **Теги метаданных** | Теги FLAC/M4A с ReplayGain, текстами, обложками |
| 🔄 | **Вход PKCE** | Безопасный OAuth для качества HiRes |
| 📺 | **Поддержка видео** | Загрузка и конвертация музыкальных видео в MP4 |
| 🎨 | **TUI и GUI** | Просмотр избранного, поиск, интерактивная загрузка |

## Установка

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### Скомпилированный бинарный файл

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### Из исходного кода

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# Бинарный файл: target/release/tdl
```

## Использование

### Вход

Стандартный OAuth (до качества HiFi):

```bash
tdl login
```

Поток PKCE для HiRes Lossless:

```bash
tdl login --pkce
```

### Загрузка

```bash
# Трек
tdl dl https://tidal.com/browse/track/12345

# Альбом / Плейлист / Микс
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# Несколько URL
tdl dl <url1> <url2>

# Из файла
tdl dl --list urls.txt
```

### TUI и GUI

```bash
# Терминальный интерфейс
tdl tui

# GUI (Tauri)
tdl gui
# или просто
tdl
```

### Конфигурация

Файл конфигурации: `~/.tdl/settings.json`

```bash
# Интерактивный мастер
tdl cfg

# Открыть в редакторе
tdl cfg --editor
```

| Настройка | По умолчанию | Описание |
|---------|---------|-------------|
| `download_base_path` | `~/download` | Корневой каталог |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | Дополнять номера треков нулями |
| `playlist_folder` | `true` | Сохранять плейлисты в `Playlists/` |
| `skip_existing` | `true` | Пропускать существующие файлы |
| `extract_flac` | `true` | Извлекать FLAC из M4A/MP4 |
| `video_convert_mp4` | `true` | Конвертировать TS в MP4 |

## Почему tdl?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| Качество звука | ✅ HiRes Lossless | ⚠️ До 320kbps | ⚠️ Переменное |
| Поддержка видео | ✅ Нативная | ❌ | ✅ |
| TUI/GUI | ✅ Оба | ❌ | ❌ |
| Метаданные | ✅ Полные | ⚠️ Базовые | ⚠️ Базовые |
| Тексты | ✅ Синхронизированы | ✅ | ⚠️ Иногда |
| Бесплатный план | ❌ Требуется подписка | ❌ Требуется подписка | ✅ Да |

## Структура вывода

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # когда playlist_folder = true
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## Требования

- **ОС**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (при сборке из исходного кода)
- **FFmpeg**: Опционально — для конвертации видео и извлечения FLAC
- **Tidal**: Подписка Premium, HiFi или HiFi Plus

## Устранение неполадок

<details>
<summary>команда не найдена после установки</summary>

Добавьте путь установки в PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Локальная установка
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>Ошибка FFmpeg не найден</summary>

Установите FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## Вклад

См. [CONTRIBUTING.md](../../CONTRIBUTING.md). PR приветствуются — см. issues с меткой `good first issue`.

## Лицензия

[Apache-2.0](../../LICENSE) © 2025
