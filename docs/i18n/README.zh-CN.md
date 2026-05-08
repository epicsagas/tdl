<div align="center">
# tdl

> Tidal 音乐下载器 — 无损音质，支持 CLI/TUI/GUI

[![GitHub Release](https://img.shields.io/github/v/release/epicsagas/tdl)](https://github.com/epicsagas/tdl/releases)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](../../LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <a href="README.ko.md">한국어</a> | <a href="README.ja.md">日本語</a> | <strong>简体中文</strong> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.de.md">Deutsch</a> | <a href="README.pt.md">Português</a> | <a href="README.ru.md">Русский</a> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

> **警告：未经授权分发受版权保护的音乐是违法行为。**
> 本工具仅供个人使用。严禁分享、再分发或公开发布下载的内容。请尊重艺术家和版权法。

## 快速开始

```bash
# macOS / Linux
brew install epicsagas/tap/tdl

# 预构建二进制文件 (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Cargo
cargo install --git https://github.com/epicsagas/tdl
```

```bash
tdl login              # OAuth 设备认证
tdl dl <tidal-url>     # 下载单曲/专辑/歌单
```

## 功能

| | 功能 | 说明 |
|--|---------|----------------|
| 🎵 | **无损音质** | 支持 HiRes Lossless (24-bit/192kHz) |
| 🖥️ | **三种界面** | CLI、TUI 和 GUI — 选择您喜欢的风格 |
| ⚡ | **并行下载** | 带重试的并发分段获取 |
| 🏷️ | **元数据标记** | FLAC/M4A 标签、ReplayGain、歌词、封面艺术 |
| 🔄 | **PKCE 登录** | HiRes 音质的安全 OAuth |
| 📺 | **视频支持** | 下载音乐视频并转换为 MP4 |
| 🎨 | **TUI 和 GUI** | 浏览收藏、搜索、交互式下载 |

## 安装

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### 预构建二进制文件

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex
```

### 从源码构建

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# 二进制文件: target/release/tdl
```

## 使用方法

### 登录

标准 OAuth (最高 HiFi 音质):

```bash
tdl login
```

HiRes Lossless 的 PKCE 流程:

```bash
tdl login --pkce
```

### 下载

```bash
# 单曲
tdl dl https://tidal.com/browse/track/12345

# 专辑 / 歌单 / 混音
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# 多个 URL
tdl dl <url1> <url2>

# 从文件
tdl dl --list urls.txt
```

### TUI 和 GUI

```bash
# 终端 UI
tdl tui

# GUI (Tauri)
tdl gui
# 或者直接
tdl
```

### 配置

设置文件: `~/.tdl/settings.json`

```bash
# 交互式向导
tdl cfg

# 在编辑器中打开
tdl cfg --editor
```

| 设置 | 默认值 | 说明 |
|---------|---------|-------------|
| `download_base_path` | `~/download` | 根目录 |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | 曲号零填充 |
| `playlist_folder` | `true` | 在 `Playlists/` 下保存歌单 |
| `skip_existing` | `true` | 跳过现有文件 |
| `extract_flac` | `true` | 从 M4A/MP4 提取 FLAC |
| `video_convert_mp4` | `true` | 将 TS 转换为 MP4 |

## 为什么选择 tdl？

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| 音频音质 | ✅ HiRes Lossless | ⚠️ 最高 320kbps | ⚠️ 可变 |
| 视频支持 | ✅ 原生 | ❌ | ✅ |
| TUI/GUI | ✅ 两者都支持 | ❌ | ❌ |
| 元数据 | ✅ 完整 | ⚠️ 基本 | ⚠️ 基本 |
| 歌词 | ✅ 同步歌词 | ✅ | ⚠️ 有时 |
| 免费计划 | ❌ 需要订阅 | ❌ 需要订阅 | ✅ 可以 |

## 输出结构

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # 当 playlist_folder = true 时
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## 系统要求

- **操作系统**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (从源码构建时)
- **FFmpeg**: 可选 — 用于视频转换和 FLAC 提取
- **Tidal**: Premium、HiFi 或 HiFi Plus 订阅

## 故障排除

<details>
<summary>安装后找不到命令</summary>

将安装路径添加到 PATH:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# 本地安装
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>找不到 FFmpeg 错误</summary>

安装 FFmpeg:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## 贡献

请参阅 [CONTRIBUTING.md](../../CONTRIBUTING.md)。欢迎 PR — 查看标记为 `good first issue` 的开放问题。

## 许可证

[Apache-2.0](../../LICENSE) © 2025
