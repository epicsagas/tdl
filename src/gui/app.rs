use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::process::Command;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tauri::{Emitter, Manager, State};
use tracing::{info, error};

use crate::config::settings::Settings;
use crate::config::token::Token;
use crate::download::downloader::Downloader;
use crate::pathfmt::format::{build_track_path, check_file_exists};
use crate::tidal::media::MediaType;
use crate::tidal::search;
use crate::tidal::session::{self as tidal_session, TidalSession};

struct PkceState {
    session: TidalSession,
    code_verifier: String,
    client_unique_key: String,
}

pub struct AppState {
    pub session: Arc<Mutex<Option<Arc<Mutex<TidalSession>>>>>,
    pkce: Arc<Mutex<Option<PkceState>>>,
    /// Maps queue-id (string timestamp from JS) to a cancellation token.
    cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

async fn ensure_session(
    state: &State<'_, AppState>,
) -> Result<Arc<Mutex<TidalSession>>, String> {
    {
        let guard = state.session.lock().await;
        if let Some(session) = guard.as_ref() {
            return Ok(Arc::clone(session));
        }
    }
    let settings = Settings::load().map_err(|e| e.to_string())?;
    let mut session = TidalSession::new(settings).map_err(|e| e.to_string())?;
    session.login().await.map_err(|e| e.to_string())?;
    let session = Arc::new(Mutex::new(session));
    tidal_session::install_auto_refresh(&session);
    {
        let mut guard = state.session.lock().await;
        *guard = Some(Arc::clone(&session));
    }
    Ok(session)
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_settings() -> Result<serde_json::Value, String> {
    let settings = Settings::load().map_err(|e| e.to_string())?;
    serde_json::to_value(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(settings: serde_json::Value) -> Result<(), String> {
    let existing = Settings::load().map_err(|e| e.to_string())?;
    let mut merged = serde_json::to_value(&existing).map_err(|e| e.to_string())?;
    if let (serde_json::Value::Object(existing_map), serde_json::Value::Object(incoming)) =
        (&mut merged, settings)
    {
        for (key, value) in incoming {
            existing_map.insert(key, value);
        }
    }
    let s: Settings = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    s.save().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_login_status() -> Result<serde_json::Value, String> {
    let token = Token::load().unwrap_or_default();
    Ok(serde_json::json!({
        "logged_in": token.is_valid(),
        "user_id": token.user_id,
        "is_pkce": token.is_pkce,
    }))
}

#[tauri::command]
async fn login(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let settings = Settings::load().map_err(|e| e.to_string())?;
    let mut session = TidalSession::new(settings).map_err(|e| e.to_string())?;
    session
        .login_with_url_handler(|url, code| {
            let _ = app.emit("login-url", serde_json::json!({ "url": url, "code": code }));
            let _ = open_url(url);
        })
        .await
        .map_err(|e| e.to_string())?;
    let session = Arc::new(Mutex::new(session));
    {
        let mut guard = state.session.lock().await;
        *guard = Some(session);
    }
    Ok(())
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    Command::new("open").arg(url).spawn()?.wait().map(|_| ())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open").arg(url).spawn()?.wait().map(|_| ())?;
    #[cfg(target_os = "windows")]
    Command::new("cmd").args(["/c", "start", "", url]).spawn()?.wait().map(|_| ())?;
    Ok(())
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state.session.lock().await;
        *guard = None;
    }
    Token::delete().map_err(|e| e.to_string())
}

#[tauri::command]
async fn login_pkce_start(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings = Settings::load().map_err(|e| e.to_string())?;
    let session = TidalSession::new(settings).map_err(|e| e.to_string())?;
    let (auth_url, code_verifier, client_unique_key) = session.pkce_build_auth_url();

    {
        let mut guard = state.pkce.lock().await;
        *guard = Some(PkceState { session, code_verifier, client_unique_key });
    }

    let _ = open_url(&auth_url);
    Ok(auth_url)
}

#[tauri::command]
async fn login_pkce_submit(
    state: State<'_, AppState>,
    redirect_url: String,
) -> Result<(), String> {
    let mut pkce_guard = state.pkce.lock().await;
    let pkce = pkce_guard.take().ok_or("PKCE login not started")?;

    let PkceState { mut session, code_verifier, client_unique_key } = pkce;
    session
        .pkce_exchange_code(&redirect_url, &code_verifier, &client_unique_key)
        .await
        .map_err(|e| e.to_string())?;

    let session = Arc::new(Mutex::new(session));
    {
        let mut guard = state.session.lock().await;
        *guard = Some(session);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[tauri::command]
async fn search_media(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u64>,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let results = search::TidalSearch::new(&sess.request)
        .search(
            &query,
            &["tracks", "albums", "artists", "videos", "playlists"],
            limit,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&results).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let user_id = sess.token.user_id.ok_or("Not logged in")?;
    let tracks = search::get_favorite_tracks(&sess.request, user_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&tracks).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_favorite_albums(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let user_id = sess.token.user_id.ok_or("Not logged in")?;
    let albums = search::get_favorite_albums(&sess.request, user_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&albums).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user_playlists(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let user_id = sess.token.user_id.ok_or("Not logged in")?;
    let playlists = search::get_user_playlists(&sess.request, user_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&playlists).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Track listing
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_album_tracks(
    state: State<'_, AppState>,
    album_id: u64,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let tracks = search::get_album_tracks(&sess.request, album_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&tracks).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let tracks = search::get_playlist_tracks(&sess.request, &playlist_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&tracks).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_mix_items(
    state: State<'_, AppState>,
    mix_id: String,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let tracks = search::get_mix_items(&sess.request, &mix_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&tracks).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_artist_albums(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let albums = search::get_artist_albums(&sess.request, artist_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&albums).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_track_audio_info(
    state: State<'_, AppState>,
    track_id: u64,
) -> Result<serde_json::Value, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let settings = Settings::load().map_err(|e| e.to_string())?;

    let (manifest, playback_info) = crate::tidal::stream::fetch_track_stream(
        &sess.request,
        track_id,
        &settings.quality_audio,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "bitDepth":   playback_info.bit_depth,
        "sampleRate": playback_info.sample_rate.or(manifest.sample_rate),
        "codec":      manifest.codecs,
        "mimeType":   manifest.mime_type,
    }))
}

#[tauri::command]
async fn get_track_local_path(
    state: State<'_, AppState>,
    track_id: u64,
) -> Result<Option<String>, String> {
    let session = ensure_session(&state).await?;
    let sess = session.lock().await;
    let settings = Settings::load().map_err(|e| e.to_string())?;

    let track = search::get_track(&sess.request, track_id)
        .await
        .map_err(|e| e.to_string())?;

    let artist_name = track.artist_name();
    // Folder path always uses the album's primary artist — same logic as build_media_info.
    let album_artist = track.album.as_ref()
        .map(|a| { let v = a.primary_artist(); if v.is_empty() { artist_name.clone() } else { v } })
        .unwrap_or_else(|| artist_name.clone());

    let info = crate::pathfmt::format::MediaInfo {
        artist_name: Some(artist_name),
        album_artist: Some(album_artist),
        track_title: Some(track.title_display()),
        album_title: track.album.as_ref().map(|a| a.name.clone()),
        album_track_num: track.track_num,
        album_num_tracks: track.album.as_ref().and_then(|a| a.num_tracks),
        track_id: Some(track.id),
        album_id: track.album.as_ref().map(|a| a.id),
        track_duration_seconds: track.duration,
        album_year: track.album.as_ref().and_then(|a| a.year_str()),
        track_quality: track.audio_quality.as_ref().map(|q| format!("{q:?}")),
        track_explicit: track.explicit,
        album_explicit: track.album.as_ref().and_then(|a| a.explicit),
        album_num_volumes: track.album.as_ref().and_then(|a| a.num_volumes),
        track_volume_num: track.volume_num,
        isrc: track.isrc.clone(),
        ..Default::default()
    };

    let relative = build_track_path(&info, settings.track_num_pad_zero);
    let base = settings.download_base_path.replace('~', &dirs::home_dir()
        .unwrap_or_default()
        .to_string_lossy());
    let stem = PathBuf::from(format!("{}/{}", base, relative));
    let audio_extensions = ["flac", "m4a", "mp3", "mp4"];

    Ok(check_file_exists(&stem, &audio_extensions)
        .map(|p| p.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------------------
// Download with progress events
// ---------------------------------------------------------------------------

#[tauri::command]
async fn download_url(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    url: String,
    queue_id: String,
) -> Result<(), String> {
    let token = CancellationToken::new();
    {
        let mut tokens = state.cancel_tokens.lock().await;
        tokens.insert(queue_id.clone(), token.clone());
    }

    let result = do_download(&state, &app_handle, &url, &queue_id, &token).await;

    {
        let mut tokens = state.cancel_tokens.lock().await;
        tokens.remove(&queue_id);
    }

    result
}

#[tauri::command]
async fn cancel_download(state: State<'_, AppState>, queue_id: String) -> Result<(), String> {
    let tokens = state.cancel_tokens.lock().await;
    if let Some(token) = tokens.get(&queue_id) {
        token.cancel();
    }
    Ok(())
}

async fn do_download(
    state: &State<'_, AppState>,
    app_handle: &tauri::AppHandle,
    url: &str,
    queue_id: &str,
    cancel: &CancellationToken,
) -> Result<(), String> {
    info!(url = %url, queue_id = %queue_id, "Download requested");
    let session = ensure_session(state).await?;
    let mut settings = Settings::load().map_err(|e| e.to_string())?;
    // GUI serializes downloads in JS with its own delay; disable Rust-side delay
    // so invoke() resolves promptly and JS can update the queue status immediately.
    settings.download_delay = false;
    let downloader = Downloader::with_cancel(Arc::clone(&session), settings, cancel.clone());
    let (media_type, id) = search::parse_media_url(url).map_err(|e| e.to_string())?;

    macro_rules! cancelled {
        () => {
            if cancel.is_cancelled() {
                app_handle
                    .emit("download-cancelled", serde_json::json!({"url": url, "queueId": queue_id}))
                    .ok();
                return Ok(());
            }
        };
    }

    match media_type {
        MediaType::Track | MediaType::Video => {
            cancelled!();
            app_handle
                .emit("download-start", serde_json::json!({"url": url, "queueId": queue_id}))
                .ok();
            let result = downloader.download_item(media_type, &id).await;
            match result {
                Ok(()) => {
                    info!(url = %url, queue_id = %queue_id, "Download complete");
                    app_handle
                        .emit("download-complete", serde_json::json!({"url": url, "queueId": queue_id}))
                        .ok();
                }
                Err(e) => {
                    error!(url = %url, queue_id = %queue_id, "Download failed: {e}");
                    app_handle
                        .emit(
                            "download-error",
                            serde_json::json!({"url": url, "queueId": queue_id, "error": e.to_string()}),
                        )
                        .ok();
                    return Err(e.to_string());
                }
            }
        }
        MediaType::Album | MediaType::Playlist | MediaType::Mix => {
            let tracks = {
                let sess = session.lock().await;
                match media_type {
                    MediaType::Album => {
                        let aid: u64 =
                            id.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                        search::get_album_tracks(&sess.request, aid).await
                    }
                    MediaType::Playlist => {
                        search::get_playlist_tracks(&sess.request, &id).await
                    }
                    MediaType::Mix => search::get_mix_items(&sess.request, &id).await,
                    _ => Err(anyhow::anyhow!("Unsupported type")),
                }
                .map_err(|e| e.to_string())?
            };

            let total = tracks.len();
            let track_list: Vec<serde_json::Value> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| serde_json::json!({
                    "index": i,
                    "title": format!("{:02}. {} - {}", i + 1, t.artist_name(), t.title_display()),
                }))
                .collect();

            app_handle
                .emit(
                    "download-start",
                    serde_json::json!({
                        "url": url,
                        "queueId": queue_id,
                        "total": total,
                        "tracks": track_list,
                    }),
                )
                .ok();

            for (i, track) in tracks.iter().enumerate() {
                cancelled!();
                let title = track.title_display();
                app_handle
                    .emit(
                        "download-progress",
                        serde_json::json!({
                            "url": url,
                            "queueId": queue_id,
                            "current": i + 1,
                            "total": total,
                            "trackIndex": i,
                            "track": title,
                        }),
                    )
                    .ok();

                match downloader
                    .download_item(MediaType::Track, &track.id.to_string())
                    .await
                {
                    Ok(()) => {
                        app_handle
                            .emit(
                                "track-done",
                                serde_json::json!({
                                    "queueId": queue_id,
                                    "trackIndex": i,
                                    "status": "ok",
                                }),
                            )
                            .ok();
                    }
                    Err(e) => {
                        app_handle
                            .emit(
                                "track-done",
                                serde_json::json!({
                                    "queueId": queue_id,
                                    "trackIndex": i,
                                    "status": "error",
                                    "error": e.to_string(),
                                }),
                            )
                            .ok();
                        app_handle
                            .emit(
                                "download-error",
                                serde_json::json!({"url": url, "queueId": queue_id, "track": title, "error": e.to_string()}),
                            )
                            .ok();
                    }
                }
            }

            if !cancel.is_cancelled() {
                app_handle
                    .emit("download-complete", serde_json::json!({"url": url, "queueId": queue_id}))
                    .ok();
            } else {
                app_handle
                    .emit("download-cancelled", serde_json::json!({"url": url, "queueId": queue_id}))
                    .ok();
            }
        }
        MediaType::Artist => {
            let albums = {
                let sess = session.lock().await;
                let aid: u64 =
                    id.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                search::get_artist_albums(&sess.request, aid)
                    .await
                    .map_err(|e| e.to_string())?
            };

            app_handle
                .emit(
                    "download-start",
                    serde_json::json!({"url": url, "queueId": queue_id, "total": albums.len()}),
                )
                .ok();

            for (i, album) in albums.iter().enumerate() {
                cancelled!();
                app_handle
                    .emit(
                        "download-progress",
                        serde_json::json!({
                            "url": url,
                            "queueId": queue_id,
                            "current": i + 1,
                            "total": albums.len(),
                            "track": album.name.clone(),
                        }),
                    )
                    .ok();

                if let Err(e) = downloader
                    .download_collection(MediaType::Album, &album.id.to_string())
                    .await
                {
                    app_handle
                        .emit(
                            "download-error",
                            serde_json::json!({
                                "url": url,
                                "queueId": queue_id,
                                "track": album.name.clone(),
                                "error": e.to_string(),
                            }),
                        )
                        .ok();
                }
            }

            if !cancel.is_cancelled() {
                app_handle
                    .emit("download-complete", serde_json::json!({"url": url, "queueId": queue_id}))
                    .ok();
            } else {
                app_handle
                    .emit("download-cancelled", serde_json::json!({"url": url, "queueId": queue_id}))
                    .ok();
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

pub fn run_gui() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .manage(AppState {
            session: Arc::new(Mutex::new(None)),
            pkce: Arc::new(Mutex::new(None)),
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_login_status,
            login,
            logout,
            login_pkce_start,
            login_pkce_submit,
            search_media,
            get_favorite_tracks,
            get_favorite_albums,
            get_user_playlists,
            get_album_tracks,
            get_playlist_tracks,
            get_mix_items,
            get_artist_albums,
            download_url,
            cancel_download,
            get_track_local_path,
            get_track_audio_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
