use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::process::Command;

use crate::config::settings::Settings;
use crate::config::token::Token;
use crate::download::downloader::Downloader;
use crate::tidal::media::MediaType;
use crate::tidal::search::{
    get_favorite_albums, get_favorite_artists, get_favorite_tracks, get_favorite_videos,
};
use crate::tidal::session::TidalSession;

#[derive(Parser)]
#[command(name = "tdl", version, about = "Tidal music downloader")]
pub struct Cli {
    /// URLs to download (starting with https://)
    #[arg(value_name = "URL")]
    urls: Vec<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log in to Tidal
    Login {
        /// Use PKCE auth for HiRes Lossless
        #[arg(long)]
        pkce: bool,
    },
    /// Log out from Tidal
    Logout,
    /// Show or change settings
    Cfg {
        /// Setting key to get/set
        key: Option<String>,
        /// Setting value to set
        value: Option<String>,
        /// Open settings file in editor
        #[arg(short, long)]
        editor: bool,
        /// Interactive settings wizard
        #[arg(short, long)]
        wizard: bool,
    },
    /// Download media from URLs
    Dl {
        /// URLs to download
        urls: Vec<String>,
        /// Read URLs from a file (one per line)
        #[arg(short, long)]
        list: Option<String>,
    },
    /// Download favorites
    #[command(subcommand)]
    Fav(FavCommands),
    /// Launch graphical interface
    Gui,
    /// Launch terminal UI
    Tui,
}

#[derive(Subcommand)]
pub enum FavCommands {
    /// Download favorite tracks
    Tracks,
    /// Download favorite albums
    Albums,
    /// Download favorite artists
    Artists,
    /// Download favorite videos
    Videos,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Bare URLs starting with https:// → download
    if !cli.urls.is_empty() {
        return handle_download(cli.urls, None).await;
    }

    match cli.command {
        // No subcommand and no URLs → launch TUI
        None => {
            let settings = Settings::load()?;
            crate::tui::run_tui_with_rt(&settings, tokio::runtime::Handle::current())?;
        }

        // ---------------------------------------------------------------
        // Login
        // ---------------------------------------------------------------
        Some(Commands::Login { pkce }) => {
            let settings = Settings::load()?;
            let mut session = TidalSession::new(settings)?;
            if pkce {
                session.login_pkce().await?;
            } else {
                session.login().await?;
            }
        }

        // ---------------------------------------------------------------
        // Logout
        // ---------------------------------------------------------------
        Some(Commands::Logout) => {
            Token::delete()?;
            println!("Logged out successfully.");
        }

        // ---------------------------------------------------------------
        // Config
        // ---------------------------------------------------------------
        Some(Commands::Cfg {
            key,
            value,
            editor,
            wizard: _,
        }) => {
            // --editor: open the settings file in the user's default editor
            if editor {
                let path = Settings::config_path();
                let editor_var = std::env::var("EDITOR").unwrap_or_else(|_| {
                    if cfg!(target_os = "windows") {
                        "notepad".to_string()
                    } else {
                        "vi".to_string()
                    }
                });
                let status = Command::new(&editor_var)
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to launch editor '{editor_var}'"))?;
                if !status.success() {
                    bail!("Editor exited with non-zero status");
                }
                return Ok(());
            }

            let settings = Settings::load()?;

            match (key, value) {
                // No args: launch interactive TUI settings editor.
                (None, None) => {
                    let _ = crate::tui::run_settings_editor(&settings)?;
                }
                // One arg: print that setting's value.
                (Some(k), None) => {
                    let json = serde_json::to_value(&settings)?;
                    match json.get(&k) {
                        Some(val) => println!("{val}"),
                        None => bail!("Unknown setting: {k}"),
                    }
                }
                // Two args: set the value.
                (Some(_k), Some(_v)) => {
                    bail!(
                        "Setting individual values is not yet supported. \
                         Use `tdl cfg --editor` to edit the settings file directly."
                    );
                }
                (None, Some(_)) => unreachable!(),
            }
        }

        // ---------------------------------------------------------------
        // Download (dl subcommand for --list support)
        // ---------------------------------------------------------------
        Some(Commands::Dl { urls, list }) => {
            handle_download(urls, list).await?;
        }

        // ---------------------------------------------------------------
        // Favorites
        // ---------------------------------------------------------------
        Some(Commands::Fav(cmd)) => {
            let settings = Settings::load()?;
            let mut session = TidalSession::new(settings.clone())?;
            session.login().await?;

            let user_id = session
                .token
                .user_id
                .ok_or_else(|| anyhow::anyhow!("Not logged in (no user ID in token)"))?;

            let session = Arc::new(Mutex::new(session));
            let downloader = Downloader::new(session, settings);

            match cmd {
                FavCommands::Tracks => {
                    let tracks = {
                        let sess = downloader.session().lock().await;
                        get_favorite_tracks(&sess.request, user_id).await?
                    };
                    let total = tracks.len() as u64;
                    let pb = indicatif::ProgressBar::new(total);
                    pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("{msg} [{bar:40}] {pos}/{len} ({eta})")?
                            .progress_chars("=>-"),
                    );
                    pb.set_message("Favorite tracks");
                    for track in &tracks {
                        let artist = track.artist_name();
                        let title = track.title_display();
                        match downloader
                            .download_item(MediaType::Track, &track.id.to_string())
                            .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                pb.println(format!(
                                    "Error downloading {artist} - {title}: {e}"
                                ));
                            }
                        }
                        pb.inc(1);
                    }
                    pb.finish_with_message("Done");
                }
                FavCommands::Albums => {
                    let albums = {
                        let sess = downloader.session().lock().await;
                        get_favorite_albums(&sess.request, user_id).await?
                    };
                    let total = albums.len() as u64;
                    let pb = indicatif::ProgressBar::new(total);
                    pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("{msg} [{bar:40}] {pos}/{len} ({eta})")?
                            .progress_chars("=>-"),
                    );
                    pb.set_message("Favorite albums");
                    for album in &albums {
                        match downloader
                            .download_collection(MediaType::Album, &album.id.to_string())
                            .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                pb.println(format!(
                                    "Error downloading album '{}': {e}",
                                    album.name
                                ));
                            }
                        }
                        pb.inc(1);
                    }
                    pb.finish_with_message("Done");
                }
                FavCommands::Artists => {
                    let artists = {
                        let sess = downloader.session().lock().await;
                        get_favorite_artists(&sess.request, user_id).await?
                    };
                    println!(
                        "Found {} favorite artists. Artist downloads are not yet supported.",
                        artists.len()
                    );
                }
                FavCommands::Videos => {
                    let videos = {
                        let sess = downloader.session().lock().await;
                        get_favorite_videos(&sess.request, user_id).await?
                    };
                    let total = videos.len() as u64;
                    let pb = indicatif::ProgressBar::new(total);
                    pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("{msg} [{bar:40}] {pos}/{len} ({eta})")?
                            .progress_chars("=>-"),
                    );
                    pb.set_message("Favorite videos");
                    for vid in &videos {
                        let title = vid
                            .title
                            .as_deref()
                            .or(vid.name.as_deref())
                            .unwrap_or("(unknown)");
                        match downloader
                            .download_item(MediaType::Video, &vid.id.to_string())
                            .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                pb.println(format!(
                                    "Error downloading video '{title}': {e}"
                                ));
                            }
                        }
                        pb.inc(1);
                    }
                    pb.finish_with_message("Done");
                }
            }
        }

        // ---------------------------------------------------------------
        // GUI
        // ---------------------------------------------------------------
        Some(Commands::Gui) => {
            run_gui();
        }

        // ---------------------------------------------------------------
        // TUI
        // ---------------------------------------------------------------
        Some(Commands::Tui) => {
            let settings = Settings::load()?;
            crate::tui::run_tui_with_rt(&settings, tokio::runtime::Handle::current())?;
        }
    }

    Ok(())
}

async fn handle_download(urls: Vec<String>, list: Option<String>) -> Result<()> {
    let mut all_urls = urls;

    if let Some(list_path) = list {
        let contents =
            fs::read_to_string(&list_path).with_context(|| {
                format!("Failed to read URL list from {}", list_path)
            })?;
        for line in contents.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                all_urls.push(trimmed.to_string());
            }
        }
    }

    if all_urls.is_empty() {
        bail!("No URLs provided. Pass URLs as arguments or use --list <file>.");
    }

    let settings = Settings::load()?;
    let mut session = TidalSession::new(settings.clone())?;
    session.login().await?;

    let session = Arc::new(Mutex::new(session));
    let downloader = Downloader::new(session, settings);

    let total = all_urls.len() as u64;
    let pb = indicatif::ProgressBar::new(total);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg} [{bar:40}] {pos}/{len} ({eta})")?
            .progress_chars("=>-"),
    );
    pb.set_message("Downloading");

    for url in &all_urls {
        match downloader.download_url(url).await {
            Ok(()) => {}
            Err(e) => {
                pb.println(format!("Error downloading {}: {e}", url));
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done");
    Ok(())
}

#[cfg(feature = "gui")]
pub fn run_gui() {
    crate::gui::run_gui();
}

#[cfg(not(feature = "gui"))]
pub fn run_gui() {
    eprintln!("GUI support not compiled. Rebuild with: cargo build --features gui");
}
