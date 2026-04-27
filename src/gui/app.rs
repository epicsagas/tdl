use std::collections::HashMap;
use std::sync::Arc;
use std::process::Command;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tauri::{Emitter, State};

use crate::config::settings::Settings;
use crate::config::token::Token;
use crate::download::downloader::Downloader;
use crate::tidal::media::MediaType;
use crate::tidal::search;
use crate::tidal::session::TidalSession;

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
            eprintln!("[tdl-gui] login url_handler called: {}", url);
            let emit_result = app.emit("login-url", serde_json::json!({ "url": url, "code": code }));
            eprintln!("[tdl-gui] emit result: {:?}", emit_result);
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
    let session = ensure_session(state).await?;
    let settings = Settings::load().map_err(|e| e.to_string())?;
    let downloader = Downloader::new(Arc::clone(&session), settings);
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
                    app_handle
                        .emit("download-complete", serde_json::json!({"url": url, "queueId": queue_id}))
                        .ok();
                }
                Err(e) => {
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
            app_handle
                .emit(
                    "download-start",
                    serde_json::json!({"url": url, "queueId": queue_id, "total": total}),
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
                            "track": title,
                        }),
                    )
                    .ok();

                if let Err(e) = downloader
                    .download_item(MediaType::Track, &track.id.to_string())
                    .await
                {
                    app_handle
                        .emit(
                            "download-error",
                            serde_json::json!({"url": url, "queueId": queue_id, "track": title, "error": e.to_string()}),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
