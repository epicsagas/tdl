# AGENTS.md

## Commands
- Build: `cargo build --release` | Build (dev): `cargo build` | Build no-GUI: `cargo build --release --no-default-features`
- Test: `cargo test --lib` | Test single: `cargo test --lib <module>` | Test verbose: `cargo test --lib -- --nocapture`
- Lint: `cargo clippy --features gui -- -D warnings` | Format: `cargo fmt --check` / `cargo fmt`
- Run: `cargo run -- [URL]` | TUI: `cargo run -- tui` | GUI: `cargo run -- gui` (or just `cargo run`)
- Check: `cargo check --features gui`
- Gen/Sync: `cargo tauri build --target <target> --bundles <type>` (release artifacts only — no codegen step)

## Project Structure
- `src/main.rs` — Entry point: inits logging, delegates to CLI
- `src/lib.rs` — Module root, feature-gates `gui` module
- `src/cli/app.rs` — CLI subcommand routing via `clap` (login, dl, fav, cfg, tui, gui)
- `src/tui/app.rs` — Ratatui TUI event loop (screens: main, download, settings, account)
- `src/tui/settings_editor.rs` — Standalone TUI settings editor
- `src/gui/app.rs` — Tauri 2 command handlers (auth, search, library, download, settings)
- `src/frontend/` — Single-file HTML/CSS/JS GUI (no framework, no build step)
- `src/config/settings.rs` — `Settings` struct (JSON persistence at `~/.tdl/settings.json`)
- `src/config/token.rs` — `Token` struct (OAuth token storage at `~/.tdl/token.json`)
- `src/config/wizard.rs` — Interactive settings wizard via `inquire`
- `src/tidal/session.rs` — `TidalSession`: OAuth device auth + PKCE login flows
- `src/tidal/request.rs` — `TidalRequest`: HTTP layer with V1/V2/Auth endpoints, retry + backoff
- `src/tidal/media.rs` — API response models (`Track`, `Album`, `Playlist`, `Video`, `Mix`, etc.)
- `src/tidal/stream.rs` — Stream manifest parsing (BTS JSON + MPEG-DASH XML)
- `src/tidal/search.rs` — Search, pagination, URL parsing (`parse_media_url`)
- `src/download/downloader.rs` — `Downloader`: full download pipeline orchestrator
- `src/download/segment.rs` — Parallel segment download with retry and exponential backoff
- `src/download/decrypt.rs` — AES-128-CTR file decryption, security token parsing
- `src/download/video.rs` — M3U8 parsing, TS→MP4 conversion, FLAC extraction via FFmpeg
- `src/metadata/writer.rs` — Audio metadata tagging via `lofty` (FLAC/M4A/MP3)
- `src/pathfmt/format.rs` — Path templating, filename sanitization, unique suffix
- `src/logging.rs` — Daily rolling log to `~/.tdl/logs/` via `tracing`
- `icons/` — App icons for Tauri bundling
- `gen/` — Tauri-generated code (build artifacts)
- `.github/workflows/release.yml` — Multi-platform CI: build + publish + Homebrew tap update

## Code Style
- Rust 2024 edition (requires Rust 1.80+)
- Naming: `snake_case` for functions/variables/modules, `PascalCase` for types/enums/structs
- Error handling: `anyhow::Result` everywhere; `anyhow!()` / `bail!()` for error construction; `.context()` / `.with_context()` for adding context
- Async: `tokio` runtime; `Arc<Mutex<T>>` for shared state across concurrent tasks
- Imports: grouped as `std` → external crates → `crate` (separated by blank lines)
- Serialization: `serde(try_from = "...")` for complex API models with field-name variants; `#[serde(rename_all = "camelCase")]` + explicit `#[serde(alias = "...")]` for simpler ones
- Feature flags: `gui` is default; `#[cfg(feature = "gui")]` guards the Tauri module and `build.rs`
- Logging: `tracing` macros with `EnvFilter`; daily rolling files; NO `println!` in library code (CLI only for user-facing output)
- HTTP: `reqwest` with streaming; `TidalRequest` wraps V1/V2/Auth with automatic retry on 429/5xx

### Golden Path
```rust
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::settings::Settings;
use crate::tidal::session::TidalSession;

pub async fn download_track(session: Arc<Mutex<TidalSession>>, settings: &Settings) -> Result<()> {
    let sess = session.lock().await;
    let manifest = crate::tidal::stream::fetch_track_stream(
        &sess.request, track_id, &settings.quality_audio,
    ).await.context("Failed to fetch stream manifest")?;
    // ... download, decrypt, tag
    Ok(())
}
```

## Testing
- Framework: built-in `#[test]` + `#[cfg(test)] mod tests` per file | Run all: `cargo test --lib` | Coverage: N/A
- File naming: inline `mod tests` within each source file (no separate test files)
- Mocking: N/A — tests use unit-level logic validation (parsing, formatting, encryption roundtrips)
- 78 library tests covering: path formatting, sanitization, segment ID parsing, M3U8 parsing, BTS/MPD manifest parsing, encryption/decryption roundtrips, token validation, URL parsing, quality mapping, settings serialization

## Git Workflow
- Branch strategy: Not observed — single main branch
- Commit format: Not observed — no conventional commit enforcement
- PR requirements: Not observed — no CI checks on PRs; CI only runs on tag push for releases
- Release: tag push (`v*` or `tdl-v*`) triggers `.github/workflows/release.yml` → multi-platform build + publish to public repo + Homebrew tap update

## Boundaries
- Always: run `cargo test --lib` after functional changes; ensure `cargo clippy` passes
- Always: use `anyhow::Result` for fallible functions; add `.context()` to chain error details
- Always: gate GUI-specific code with `#[cfg(feature = "gui")]`
- Always: sanitize filenames via `pathfmt::format::sanitize_filename` before filesystem operations
- Always: use `tracing` macros for logging (not `println!` in library modules)
- Always: respect `Settings` fields (skip_existing, download_delay, concurrent limits) in download paths
- Ask first: before adding new dependencies to `Cargo.toml`
- Ask first: before modifying the OAuth client credential constants in `session.rs`
- Ask first: before changing the Tauri command API surface (`src/gui/app.rs` + `frontend/index.html`)
- Never: hardcode API keys or tokens — use `~/.tdl/` config directory or environment variables
- Never: bypass `TidalSession` to make direct unauthenticated API calls
- Never: remove or weaken the filename sanitization or path traversal checks
- Never: suppress linter warnings inline — fix the root cause instead
- Never: Use `#[allow(...)]` to suppress warnings — fix the root cause; use `#[expect(...)]` only with a tracking issue comment
