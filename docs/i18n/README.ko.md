<div align="center">

# tdl

> Tidal 음악 다운로더 - 무손실 음질, CLI/TUI/GUI 지원

[![GitHub Release](https://img.shields.io/github/v/release/epicsagas/tdl)](https://github.com/epicsagas/tdl/releases)
[![Version](https://img.shields.io/crates/v/tdl)](https://crates.io/crates/tdl)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Homebrew](https://img.shields.io/badge/install-homebrew-orange)](#installation)

<p>
<a href="../../README.md"><strong>English</strong></a> | <strong>한국어</strong> | <a href="README.ja.md">日本語</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.de.md">Deutsch</a> | <a href="README.pt.md">Português</a> | <a href="README.ru.md">Русский</a> | <a href="README.it.md">Italiano</a>
</p>

</div>

<img src="../assets/favorites.png" alt="favorites gui" width="100%" />

> **경고: 저작권 음원의 무단 배포는 불법입니다.**
> 이 도구는 개인 용도로만 사용하세요. 다운로드한 콘텐츠를 공유, 재배포 또는 공개적으로 제공해서는 안 됩니다. 아티스트와 저작권법을 존중하세요.

## 빠른 시작

```bash
# macOS / Linux
brew install epicasagas/tap/tdl

# 미리 빌드된 바이너리 (Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.sh | sh

# 소스에서 빌드
cargo install tdl
```

```bash
tdl login              # OAuth 장치 인증
tdl dl <tidal-url>     # 트랙/앨범/플레이리스트 다운로드
```

## 기능

| | 기능 | 설명 |
|--|---------|----------------|
| 🎵 | **무손실 음질** | HiRes Lossless (24-bit/192kHz) 지원 |
| 🖥️ | **세 가지 인터페이스** | CLI, TUI, GUI — 원하는 스타일 선택 |
| ⚡ | **병렬 다운로드** | 재시도와 함께 동시 세그먼트 가져오기 |
| 🏷️ | **메타데이터 태깅** | FLAC/M4A 태그, ReplayGain, 가사, 커버 아트 |
| 🔄 | **PKCE 로그인** | HiRes 음질을 위한 보안 OAuth |
| 📺 | **비디오 지원** | 뮤직 비디오 다운로드 및 MP4로 변환 |
| 🎨 | **TUI 및 GUI** | 즐겨찾기 찾아보기, 검색, 대화형 다운로드 |

## 설치

### Homebrew (macOS/Linux)

```bash
brew install epicsagas/tap/tdl
```

### 미리 빌드된 바이너리

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

##### 소스에서 빌드

```bash
git clone https://github.com/epicsagas/tdl.git
cd tdl
cargo build --release
# 바이너리: target/release/tdl
```

### Cargo

```bash
cargo install --git https://github.com/epicsagas/tdl
```

## 사용법

### 로그인

표준 OAuth (HiFi 음질까지):

```bash
tdl login
```

HiRes Lossless용 PKCE 흐름:

```bash
tdl login --pkce
```

### 다운로드

```bash
# 트랙
tdl dl https://tidal.com/browse/track/12345

# 앨범 / 플레이리스트 / 믹스
tdl dl https://tidal.com/browse/album/67890
tdl dl https://tidal.com/browse/playlist/abc-uuid

# 여러 URL
tdl dl <url1> <url2>

# 파일에서
tdl dl --list urls.txt
```

### TUI 및 GUI

```bash
# 터미널 UI
tdl tui

# GUI (Tauri)
tdl gui
# 또는 그냥
tdl
```

### 설정

설정 파일: `~/.tdl/settings.json`

```bash
# 대화형 마법사
tdl cfg

# 편집기에서 열기
tdl cfg --editor
```

| 설정 | 기본값 | 설명 |
|---------|---------|-------------|
| `download_base_path` | `~/download` | 루트 디렉토리 |
| `quality_audio` | `low_320k` | `low_96k` / `low_320k` / `high_lossless` / `hi_res_lossless` |
| `quality_video` | `p480` | `p360` / `p480` / `p720` / `p1080` |
| `track_num_pad_zero` | `true` | 트랙 번호 0 채우기 |
| `playlist_folder` | `true` | `Playlists/` 아래에 플레이리스트 저장 |
| `skip_existing` | `true` | 기존 파일 건너뛰기 |
| `extract_flac` | `true` | M4A/MP4에서 FLAC 추출 |
| `video_convert_mp4` | `true` | TS를 MP4로 변환 |

## 왜 tdl인가요?

| | tdl | spotdl | yt-dlp |
|-|-----|--------|--------|
| 오디오 음질 | ✅ HiRes Lossless | ⚠️ 최대 320kbps | ⚠️ 가변 |
| 비디오 지원 | ✅ 네이티브 | ❌ | ✅ |
| TUI/GUI | ✅ 둘 다 | ❌ | ❌ |
| 메타데이터 | ✅ 전체 | ⚠️ 기본 | ⚠️ 기본 |
| 가사 | ✅ 동기화됨 | ✅ | ⚠️ 때때로 |
| 무료 플랜 | ❌ 구독 필요 | ❌ 구독 필요 | ✅ 예 |

## 출력 구조

```
{base}/
  {artist}/
    {album}/
      01. Track Title.flac
      02. Another Track.flac
      cover.jpg

  Playlists/               # playlist_folder = true일 때
    My Playlist/
      01. Artist - Title.flac
      My Playlist.m3u
```

## 요구사항

- **OS**: macOS 12+ / Ubuntu 20.04+ / Windows 10+
- **Rust**: 1.80+ (소스에서 빌드할 때)
- **FFmpeg**: 선택사항 — 비디오 변환 및 FLAC 추출용
- **Tidal**: Premium, HiFi, 또는 HiFi Plus 구독

## 문제 해결

<details>
<summary>설치 후 명령을 찾을 수 없음</summary>

설치 경로를 PATH에 추가하세요:

```bash
# Rust/Cargo
export PATH="$HOME/.cargo/bin:$PATH"

# 로컬 설치
export PATH="$HOME/.local/bin:$PATH"
```
</details>

<details>
<summary>FFmpeg를 찾을 수 없음 오류</summary>

FFmpeg를 설치하세요:

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows (Chocolatey)
choco install ffmpeg
```
</details>

## 기여

[CONTRIBUTING.md](../../CONTRIBUTING.md)를 참조하세요. PR 환영합니다 — `good first issue` 라벨이 붙은 이슈를 확인해주세요.

## 라이선스

[Apache-2.0](../../LICENSE) © 2025
