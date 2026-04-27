# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build (GUI is the default feature)
cargo build
cargo build --release

# Run
cargo run -- [URL]          # download a URL
cargo run -- tui            # launch TUI
cargo run -- gui            # launch GUI (same as default)
cargo run --features gui    # explicit GUI feature

# Test
cargo test --lib            # all 78 library tests
cargo test --lib tidal      # specific module
cargo test -- --nocapture   # show stdout from tests

# Check without linking
cargo check --features gui
```

All data (settings, token) lives in `~/.tdl/`. Config path is determined by `Settings::config_dir()` → `dirs::home_dir().join(".tdl")`.

## Architecture

### Interface split

Three UI modes share the same `Downloader` + `TidalSession` core:

- **CLI** (`src/cli/app.rs`) — subcommand routing via `clap`
- **TUI** (`src/tui/app.rs`) — Ratatui event loop; async ops bridged via `tokio::runtime::Handle::block_on`
- **GUI** (`src/gui/app.rs` + `frontend/index.html`) — Tauri 2 command handlers; single HTML file frontend (no build step needed)

GUI is feature-gated: `cargo build --features gui`. The `default` feature already includes `gui`.

### Download pipeline

```
URL → parse_media_url → MediaType + ID
    → TidalSession::login (token validation / refresh / device-auth)
    → fetch metadata (Track/Album/Playlist)
    → format_path_media (template substitution + sanitize + unique suffix)
    → fetch_track_stream → manifest (BTS JSON or MPEG-DASH)
    → segment::download_and_merge (parallel, FuturesUnordered, retry backoff)
    → decrypt::decrypt_file (AES-128-CTR, if encrypted)
    → video::convert_to_mp4 (FFmpeg, optional)
    → metadata::writer::write_metadata (lofty)
    → download cover art + save .lrc lyrics (optional)
```

### Session / auth

`TidalSession` owns the HTTP client wrapper (`TidalRequest`) and the `Token`. It is wrapped in `Arc<Mutex<TidalSession>>` and shared between concurrent download tasks.

Two login flows in `src/tidal/session.rs`:
- **Device auth** (standard): polls `/token` until user visits verification URL
- **PKCE** (HiRes Lossless): generates code_verifier/challenge, user pastes redirect URL back

The OAuth client credentials are double-base64-encoded constants in `session.rs` and decoded at runtime.

### Key structs

| Struct | Location | Purpose |
|---|---|---| 
| `TidalSession` | `tidal/session.rs` | Auth, token lifecycle |
| `TidalRequest` | `tidal/request.rs` | HTTP layer (V1/V2/Auth), retry logic |
| `Downloader` | `download/downloader.rs` | Orchestrates the full download pipeline |
| `Settings` | `config/settings.rs` | All user preferences, path templates |
| `Track` / `Album` | `tidal/media.rs` | API response models |

### API layer (`src/tidal/request.rs`)

- `get_v1` — adds `sessionId` + `countryCode` + `limit=10000` query params automatically
- `get_v2` — no session params; used for newer endpoints
- `get_v1_raw(url)` — arbitrary URL with auth headers; used for authenticated CDN resources (cover art)
- `send_with_retry` — exponential backoff on 429 / 5xx

### Path templating (`src/pathfmt/format.rs`)

Templates like `Albums/{album_artist} - {album_title}/{album_track_num}. {artist_name} - {track_title}` are expanded by `format_path_media`. Filenames are sanitized (illegal chars removed, 255-char limit) and made unique by appending `_1`, `_2`, etc.

Default `format_track` puts single tracks under `Albums/{album_artist} - {album_title}/` to avoid `cover.jpg` collisions across tracks in the same folder.

### Media model deserialization (`src/tidal/media.rs`)

`Album` uses `#[serde(try_from = "AlbumDeserialize")]` for a manual conversion layer that handles API field name variants (e.g. `numberOfTracks` / `numTracks`). `Track` uses `#[serde(rename_all = "camelCase")]` with explicit `alias` attributes for fields the API returns under alternate names (`trackNumber` / `trackNum`, `volumeNumber` / `volumeNum`).

### Frontend (`frontend/index.html`)

Single-file HTML/CSS/JS — no framework, no build step. Communicates with Rust via `window.__TAURI__.core.invoke(cmd, args)`. Themes are applied via `data-theme` attribute on `<html>` and persisted in `localStorage`. Track table rendering is centralized in `renderTrackTable(tracks, areaId)`.

## Feature flags

| Flag | Effect |
|---|---|
| `gui` (default) | Enables Tauri, build.rs calls `tauri_build::build()`, adds `opener` crate |

Without `--features gui`, the `run_gui()` function prints an error and exits.
