use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::settings::Settings;
use crate::config::token::Token;
use crate::download::downloader::Downloader;
use crate::tidal::session::{self as tidal_session, TidalSession};

// ---------------------------------------------------------------------------
// Screen enum
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Main,
    Download,
    Settings,
    Account,
}

impl Screen {
    fn next(self) -> Self {
        match self {
            Screen::Main => Screen::Download,
            Screen::Download => Screen::Settings,
            Screen::Settings => Screen::Account,
            Screen::Account => Screen::Main,
        }
    }

    fn prev(self) -> Self {
        match self {
            Screen::Main => Screen::Account,
            Screen::Download => Screen::Main,
            Screen::Settings => Screen::Download,
            Screen::Account => Screen::Settings,
        }
    }
}

// ---------------------------------------------------------------------------
// Background download communication
// ---------------------------------------------------------------------------

enum DownloadMsg {
    Log(String),
    SessionReady(Arc<Mutex<TidalSession>>),
    Done,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    screen: Screen,
    settings: Settings,
    // Shared session (reused across downloads / logins)
    session: Option<Arc<Mutex<TidalSession>>>,
    // Main menu
    menu_state: ListState,
    // Download
    url_input: String,
    url_cursor: usize,
    download_log: Vec<(String, LogLevel)>,
    downloading: bool,
    download_rx: Option<std::sync::mpsc::Receiver<DownloadMsg>>,
    // Account / PKCE
    logged_in: bool,
    user_id: Option<u64>,
    account_state: ListState,
    pkce_state: PkceFlowState,
    logout_confirm: bool,
    // Settings
    settings_state: ListState,
    settings_editing: bool,
    settings_edit_buffer: String,
    settings_dropdown: bool,
    settings_dropdown_idx: usize,
    settings_fields: Vec<SettingsField>,
}

#[derive(Clone, Copy)]
enum LogLevel {
    Info,
    Success,
    Error,
}

#[derive(Default)]
enum PkceFlowState {
    #[default]
    Idle,
    // Waiting for user to paste redirect URL; store verifier from the original auth URL.
    AwaitingRedirect {
        auth_url: String,
        verifier: String,
        unique_key: String,
        input: String,
        cursor: usize,
    },
}

struct SettingsField {
    label: &'static str,
    get: fn(&Settings) -> String,
    set: fn(&mut Settings, &str),
    options: Option<&'static [&'static str]>,
}

fn make_settings_fields() -> Vec<SettingsField> {
    use crate::config::settings::{CoverDimensions, Quality, QualityVideo};
    vec![
        SettingsField {
            label: "Audio Quality",
            get: |s| serde_json::to_value(&s.quality_audio).unwrap().as_str().unwrap().to_string(),
            set: |s, v| s.quality_audio = match v { "low_96k"=>Quality::Low96k, "low_320k"=>Quality::Low320k, "high_lossless"=>Quality::HighLossless, "hi_res_lossless"=>Quality::HiResLossless, _=>return },
            options: Some(&["low_96k", "low_320k", "high_lossless", "hi_res_lossless"]),
        },
        SettingsField {
            label: "Video Quality",
            get: |s| serde_json::to_value(&s.quality_video).unwrap().as_str().unwrap().to_string(),
            set: |s, v| s.quality_video = match v { "p360"=>QualityVideo::P360, "p480"=>QualityVideo::P480, "p720"=>QualityVideo::P720, "p1080"=>QualityVideo::P1080, _=>return },
            options: Some(&["p360", "p480", "p720", "p1080"]),
        },
        SettingsField { label: "Skip Existing", get: |s| s.skip_existing.to_string(), set: |s, v| s.skip_existing = v == "true", options: None },
        SettingsField { label: "Download Delay", get: |s| s.download_delay.to_string(), set: |s, v| s.download_delay = v == "true", options: None },
        SettingsField { label: "Delay Min", get: |s| format!("{:.1}", s.download_delay_sec_min), set: |s, v| { if let Ok(n)=v.parse::<f64>() { s.download_delay_sec_min=n; } }, options: None },
        SettingsField { label: "Delay Max", get: |s| format!("{:.1}", s.download_delay_sec_max), set: |s, v| { if let Ok(n)=v.parse::<f64>() { s.download_delay_sec_max=n; } }, options: None },
        SettingsField { label: "Concurrent Max", get: |s| s.downloads_concurrent_max.to_string(), set: |s, v| { if let Ok(n)=v.parse::<usize>() { s.downloads_concurrent_max=n.clamp(1,5); } }, options: None },
        SettingsField { label: "Download Path", get: |s| s.download_base_path.clone(), set: |s, v| s.download_base_path=v.to_string(), options: None },
        SettingsField { label: "FFmpeg Path", get: |s| s.path_binary_ffmpeg.clone(), set: |s, v| s.path_binary_ffmpeg=v.to_string(), options: None },
        SettingsField { label: "Video Convert MP4", get: |s| s.video_convert_mp4.to_string(), set: |s, v| s.video_convert_mp4=v=="true", options: None },
        SettingsField {
            label: "Cover Size",
            get: |s| serde_json::to_value(&s.metadata_cover_dimension).unwrap().as_str().unwrap().to_string(),
            set: |s,v| s.metadata_cover_dimension=match v{"px80"=>CoverDimensions::Px80,"px160"=>CoverDimensions::Px160,"px320"=>CoverDimensions::Px320,"px640"=>CoverDimensions::Px640,"px1280"=>CoverDimensions::Px1280,_=>return},
            options: Some(&["px80", "px160", "px320", "px640", "px1280"]),
        },
        SettingsField { label: "Cover Embed", get: |s| s.metadata_cover_embed.to_string(), set: |s, v| s.metadata_cover_embed=v=="true", options: None },
        SettingsField { label: "Extract FLAC", get: |s| s.extract_flac.to_string(), set: |s, v| s.extract_flac=v=="true", options: None },
        SettingsField { label: "Lyrics Embed", get: |s| s.lyrics_embed.to_string(), set: |s, v| s.lyrics_embed=v=="true", options: None },
        SettingsField { label: "Lyrics File", get: |s| s.lyrics_file.to_string(), set: |s, v| s.lyrics_file=v=="true", options: None },
        SettingsField { label: "ReplayGain", get: |s| s.metadata_replay_gain.to_string(), set: |s, v| s.metadata_replay_gain=v=="true", options: None },
        SettingsField { label: "Playlist Folder", get: |s| s.playlist_folder.to_string(), set: |s, v| s.playlist_folder=v=="true", options: None },
        SettingsField { label: "Track Num Zero Pad", get: |s| s.track_num_pad_zero.to_string(), set: |s, v| s.track_num_pad_zero=v=="true", options: None },
    ]
}

impl App {
    fn new(settings: Settings) -> Self {
        let token = Token::load().unwrap_or_default();
        let logged_in = token.is_valid();
        let user_id = token.user_id;

        let mut menu_state = ListState::default();
        menu_state.select(Some(0));
        let mut settings_state = ListState::default();
        settings_state.select(Some(0));
        let mut account_state = ListState::default();
        account_state.select(Some(0));

        Self {
            screen: Screen::Main,
            settings,
            session: None,
            menu_state,
            url_input: String::new(),
            url_cursor: 0,
            download_log: Vec::new(),
            downloading: false,
            download_rx: None,
            logged_in,
            user_id,
            account_state,
            pkce_state: PkceFlowState::Idle,
            logout_confirm: false,
            settings_state,
            settings_editing: false,
            settings_edit_buffer: String::new(),
            settings_dropdown: false,
            settings_dropdown_idx: 0,
            settings_fields: make_settings_fields(),
        }
    }

    fn log(&mut self, msg: impl Into<String>, level: LogLevel) {
        self.download_log.push((msg.into(), level));
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw(app: &mut App, frame: &mut Frame) {
    let size = frame.area();
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(size);

    draw_header(app, frame, chunks[0]);

    match app.screen {
        Screen::Main => draw_main(app, frame, chunks[1]),
        Screen::Download => draw_download(app, frame, chunks[1]),
        Screen::Settings => draw_settings(app, frame, chunks[1]),
        Screen::Account => draw_account(app, frame, chunks[1]),
    }
}

fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    let tabs = [
        ("Main", Screen::Main),
        ("Download", Screen::Download),
        ("Settings", Screen::Settings),
        ("Account", Screen::Account),
    ];
    let spans: Vec<Span> = tabs
        .iter()
        .enumerate()
        .flat_map(|(i, (name, screen))| {
            let style = if app.screen == *screen {
                Style::default().bold().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            if i < tabs.len() - 1 {
                vec![Span::styled(format!(" {} ", name), style), Span::raw(" | ")]
            } else {
                vec![Span::styled(format!(" {} ", name), style)]
            }
        })
        .collect();

    let login_indicator = if app.logged_in {
        Span::styled(" ● ", Style::default().fg(Color::Green))
    } else {
        Span::styled(" ○ ", Style::default().fg(Color::DarkGray))
    };

    let title = Paragraph::new(Line::from(
        [
            Span::styled(" tdl ", Style::default().bold().cyan()),
            login_indicator,
            Span::raw("  "),
        ]
        .into_iter()
        .chain(spans)
        .collect::<Vec<Span>>(),
    ))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn draw_main(app: &mut App, frame: &mut Frame, area: Rect) {
    let items = vec![
        ListItem::new(Line::from(Span::styled("Download", Style::default().fg(Color::Yellow)))),
        ListItem::new(Line::from(Span::styled("Settings", Style::default().fg(Color::Yellow)))),
        ListItem::new(Line::from(Span::styled("Account", Style::default().fg(Color::Yellow)))),
        ListItem::new(Line::from(Span::raw(""))),
        ListItem::new(Line::from(Span::styled("Quit", Style::default().fg(Color::Red)))),
    ];
    let list = List::new(items)
        .block(Block::default().title(" Main Menu ").borders(Borders::ALL))
        .highlight_style(Style::default().bold().white())
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut app.menu_state);
}

fn draw_download(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).split(area);

    let status = if app.downloading { " (downloading...)" } else { "" };
    let input_text = format!("{}{}_", app.url_input, status);
    let input_style = if app.downloading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(Line::from(Span::styled(input_text, input_style)))
        .block(Block::default().title(" URL (Enter download, Tab switch, Esc back) ").borders(Borders::ALL));
    frame.render_widget(input, chunks[0]);

    let log_items: Vec<ListItem> = app
        .download_log
        .iter()
        .rev()
        .take(area.height as usize)
        .map(|(msg, level)| {
            let style = match level {
                LogLevel::Error => Style::default().fg(Color::Red),
                LogLevel::Success => Style::default().fg(Color::Green),
                LogLevel::Info => Style::default().fg(Color::White),
            };
            ListItem::new(Line::from(Span::styled(msg.clone(), style)))
        })
        .collect();

    let log = List::new(log_items).block(
        Block::default()
            .title(format!(" Log ({}) ", app.download_log.len()))
            .borders(Borders::ALL),
    );
    frame.render_widget(log, chunks[1]);
}

fn draw_settings(app: &mut App, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = app
        .settings_fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let value = (field.get)(&app.settings);
            let is_selected = app.settings_state.selected() == Some(i);
            let label_style = if is_selected && !app.settings_editing && !app.settings_dropdown {
                Style::default().bold().fg(Color::Yellow)
            } else {
                Style::default()
            };
            let value_style = if is_selected && app.settings_editing {
                Style::default().bold().fg(Color::Green)
            } else if is_selected {
                Style::default().bold().white()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let hint = if field.options.is_some() { " ◂" } else { "" };
            let label = format!("{:<20}", field.label);
            ListItem::new(Line::from(vec![
                Span::styled(label, label_style),
                Span::raw(": "),
                Span::styled(format!("{}{}", value, hint), value_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Settings (Enter edit, s save, Tab switch, Esc back) ").borders(Borders::ALL))
        .highlight_style(Style::default())
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut app.settings_state);

    if app.settings_editing {
        let popup = Rect {
            x: area.x + 10,
            y: area.y + 3 + app.settings_state.selected().unwrap_or(0) as u16,
            width: 50.min(area.width.saturating_sub(20)),
            height: 3,
        };
        let input = Paragraph::new(Line::from(vec![
            Span::styled(&app.settings_edit_buffer, Style::default().fg(Color::Yellow)),
            Span::styled("█", Style::default().white()),
        ]))
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Green)));
        frame.render_widget(ratatui::widgets::Clear, popup);
        frame.render_widget(input, popup);
    }

    if app.settings_dropdown {
        let sel_idx = app.settings_state.selected().unwrap_or(0);
        if let Some(field) = app.settings_fields.get(sel_idx)
            && let Some(options) = &field.options {
                let max_opt_len = options.iter().map(|o| o.len()).max().unwrap_or(10);
                let popup_w = (max_opt_len as u16 + 6).max(22);
                let popup_h = (options.len() as u16 + 2).min(area.height.saturating_sub(3));
                let row_y = area.y + 2 + sel_idx as u16;
                let popup = Rect {
                    x: area.x + 24,
                    y: row_y,
                    width: popup_w.min(area.width.saturating_sub(24)),
                    height: popup_h,
                };

                let items: Vec<ListItem> = options
                    .iter()
                    .enumerate()
                    .map(|(i, opt)| {
                        let marker = if i == app.settings_dropdown_idx { " > " } else { "   " };
                        let style = if i == app.settings_dropdown_idx {
                            Style::default().bold().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        ListItem::new(Line::from(Span::styled(format!("{}{}", marker, opt), style)))
                    })
                    .collect();

                let list = List::new(items).block(
                    Block::default()
                        .title(format!(" {} ", field.label))
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Cyan)),
                );
                frame.render_widget(ratatui::widgets::Clear, popup);
                frame.render_widget(list, popup);
            }
    }
}

fn draw_account(app: &mut App, frame: &mut Frame, area: Rect) {
    let status = if app.logged_in {
        format!("Logged in as user {}", app.user_id.unwrap_or(0))
    } else {
        "Not logged in".to_string()
    };
    let status_color = if app.logged_in { Color::Green } else { Color::Red };

    // PKCE redirect URL input overlay
    if let PkceFlowState::AwaitingRedirect { auth_url, input, .. } = &app.pkce_state {
        let chunks = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let url_para = Paragraph::new(vec![
            Line::from(Span::styled("Open this URL in a browser:", Style::default().fg(Color::Yellow))),
            Line::from(Span::styled(auth_url.clone(), Style::default().fg(Color::Cyan))),
        ])
        .block(Block::default().title(" PKCE Login ").borders(Borders::ALL));
        frame.render_widget(url_para, chunks[0]);

        let redirect_input = Paragraph::new(Line::from(vec![
            Span::styled(input.clone(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().white()),
        ]))
        .block(Block::default().title(" Paste redirect URL then Enter (Esc cancel) ").borders(Borders::ALL).style(Style::default().fg(Color::Green)));
        frame.render_widget(redirect_input, chunks[1]);
        return;
    }

    let items = vec![
        ListItem::new(Line::from(Span::styled(
            "Login (OAuth device flow)",
            Style::default().fg(Color::Yellow),
        ))),
        ListItem::new(Line::from(Span::styled(
            "Login with PKCE (HiRes Lossless)",
            Style::default().fg(Color::Yellow),
        ))),
        ListItem::new(Line::from(Span::styled(
            if app.logged_in { "Logout" } else { "Logout (not logged in)" },
            if app.logged_in { Style::default().fg(Color::Red) } else { Style::default().fg(Color::DarkGray) },
        ))),
    ];

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Account [{}] ", status))
                .title_style(Style::default().fg(status_color))
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bold().white())
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut app.account_state);

    let help = Paragraph::new("Tab switch  ↑↓ navigate  Enter select  Esc back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);

    if app.logout_confirm {
        let popup = centered_rect(30, 3, area);
        let msg = Paragraph::new("  Logout? (y/n)")
            .block(
                Block::default()
                    .title(" Confirm ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Yellow)),
            );
        frame.render_widget(ratatui::widgets::Clear, popup);
        frame.render_widget(msg, popup);
    }
}

// ---------------------------------------------------------------------------
// Event handling
// ---------------------------------------------------------------------------

fn handle_event(app: &mut App, event: Event) -> bool {
    if let Event::Key(key) = event {
        // PKCE redirect URL input intercepts all keys
        if matches!(app.pkce_state, PkceFlowState::AwaitingRedirect { .. }) {
            return handle_pkce_input(app, key.code);
        }

        // Global Tab / Shift+Tab navigation
        match key.code {
            KeyCode::Tab => {
                app.settings_editing = false;
                app.settings_edit_buffer.clear();
                app.settings_dropdown = false;
                app.screen = app.screen.next();
                return true;
            }
            KeyCode::BackTab => {
                app.settings_editing = false;
                app.settings_edit_buffer.clear();
                app.settings_dropdown = false;
                app.screen = app.screen.prev();
                return true;
            }
            _ => {}
        }

        match app.screen {
            Screen::Main => handle_main(app, key.code),
            Screen::Download => handle_download(app, key.code),
            Screen::Settings => handle_settings(app, key.code),
            Screen::Account => handle_account(app, key.code),
        }
    } else {
        true
    }
}

fn handle_pkce_input(app: &mut App, code: KeyCode) -> bool {
    let PkceFlowState::AwaitingRedirect { input, cursor, .. } = &mut app.pkce_state else {
        return true;
    };
    match code {
        KeyCode::Esc => {
            app.pkce_state = PkceFlowState::Idle;
        }
        KeyCode::Enter => {
            // Taken by the caller after this returns, via account_pending mechanism
            // We signal completion by returning a special marker: set pkce_state to Idle
            // but store the input in a side-channel via a temporary log entry.
            // Cleaner: use a dedicated field.
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                *cursor -= 1;
                input.remove(*cursor);
            }
        }
        KeyCode::Left => {
            if *cursor > 0 { *cursor -= 1; }
        }
        KeyCode::Right => {
            if *cursor < input.len() { *cursor += 1; }
        }
        KeyCode::Char(c) => {
            input.insert(*cursor, c);
            *cursor += 1;
        }
        _ => {}
    }
    true
}

fn handle_main(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            let idx = app.menu_state.selected().unwrap_or(0);
            if idx > 0 { app.menu_state.select(Some(idx - 1)); }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let idx = app.menu_state.selected().unwrap_or(0);
            if idx < 4 { app.menu_state.select(Some(idx + 1)); }
        }
        KeyCode::Enter => match app.menu_state.selected().unwrap_or(0) {
            0 => app.screen = Screen::Download,
            1 => app.screen = Screen::Settings,
            2 => app.screen = Screen::Account,
            4 => return false,
            _ => {}
        },
        KeyCode::Char('q') | KeyCode::Esc => return false,
        _ => {}
    }
    true
}

fn handle_download(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => app.screen = Screen::Main,
        KeyCode::Enter => {} // handled in main loop
        KeyCode::Backspace => {
            if app.url_cursor > 0 {
                app.url_cursor -= 1;
                app.url_input.remove(app.url_cursor);
            }
        }
        KeyCode::Left => {
            if app.url_cursor > 0 { app.url_cursor -= 1; }
        }
        KeyCode::Right => {
            if app.url_cursor < app.url_input.len() { app.url_cursor += 1; }
        }
        KeyCode::Char(c) => {
            app.url_input.insert(app.url_cursor, c);
            app.url_cursor += 1;
        }
        _ => {}
    }
    true
}

fn handle_settings(app: &mut App, code: KeyCode) -> bool {
    if app.settings_editing {
        match code {
            KeyCode::Enter => {
                let idx = app.settings_state.selected().unwrap_or(0);
                if let Some(field) = app.settings_fields.get(idx) {
                    (field.set)(&mut app.settings, &app.settings_edit_buffer);
                }
                app.settings_editing = false;
                app.settings_edit_buffer.clear();
            }
            KeyCode::Esc => {
                app.settings_editing = false;
                app.settings_edit_buffer.clear();
            }
            KeyCode::Backspace => { app.settings_edit_buffer.pop(); }
            KeyCode::Char(c) => { app.settings_edit_buffer.push(c); }
            _ => {}
        }
    } else if app.settings_dropdown {
        let sel_idx = app.settings_state.selected().unwrap_or(0);
        let opt_count = app
            .settings_fields
            .get(sel_idx)
            .and_then(|f| f.options)
            .map(|o| o.len())
            .unwrap_or(0);

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if app.settings_dropdown_idx > 0 { app.settings_dropdown_idx -= 1; }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.settings_dropdown_idx + 1 < opt_count { app.settings_dropdown_idx += 1; }
            }
            KeyCode::Enter => {
                if let Some(field) = app.settings_fields.get(sel_idx)
                    && let Some(options) = &field.options
                        && let Some(&opt) = options.get(app.settings_dropdown_idx) {
                            (field.set)(&mut app.settings, opt);
                        }
                app.settings_dropdown = false;
            }
            KeyCode::Esc => { app.settings_dropdown = false; }
            _ => {}
        }
    } else {
        match code {
            KeyCode::Esc => app.screen = Screen::Main,
            KeyCode::Up | KeyCode::Char('k') => {
                let idx = app.settings_state.selected().unwrap_or(0);
                if idx > 0 { app.settings_state.select(Some(idx - 1)); }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let idx = app.settings_state.selected().unwrap_or(0);
                if idx < app.settings_fields.len() - 1 {
                    app.settings_state.select(Some(idx + 1));
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let idx = app.settings_state.selected().unwrap_or(0);
                if let Some(field) = app.settings_fields.get(idx) {
                    let val = (field.get)(&app.settings);
                    if val == "true" || val == "false" {
                        let new_val = if val == "true" { "false" } else { "true" };
                        (field.set)(&mut app.settings, new_val);
                    } else if let Some(options) = &field.options {
                        app.settings_dropdown_idx =
                            options.iter().position(|&o| o == val).unwrap_or(0);
                        app.settings_dropdown = true;
                    } else {
                        app.settings_editing = true;
                        app.settings_edit_buffer = val;
                    }
                }
            }
            KeyCode::Char('s') => {
                match app.settings.save() {
                    Ok(()) => app.log("Settings saved.", LogLevel::Success),
                    Err(e) => app.log(format!("Error saving settings: {e}"), LogLevel::Error),
                }
            }
            _ => {}
        }
    }
    true
}

fn handle_account(app: &mut App, code: KeyCode) -> bool {
    if app.logout_confirm {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let _ = Token::delete();
                app.logged_in = false;
                app.user_id = None;
                app.session = None;
                app.logout_confirm = false;
                app.log("Logged out.", LogLevel::Info);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.logout_confirm = false;
            }
            _ => {}
        }
        return true;
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            let idx = app.account_state.selected().unwrap_or(0);
            if idx > 0 { app.account_state.select(Some(idx - 1)); }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let idx = app.account_state.selected().unwrap_or(0);
            if idx < 2 { app.account_state.select(Some(idx + 1)); }
        }
        KeyCode::Enter => match app.account_state.selected().unwrap_or(0) {
            0 => {} // OAuth: handled in main loop
            1 => {} // PKCE start: handled in main loop
            2 => {
                if app.logged_in { app.logout_confirm = true; }
            }
            _ => {}
        },
        KeyCode::Esc => app.screen = Screen::Main,
        _ => {}
    }
    true
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run_tui(settings: &Settings) -> Result<()> {
    run_tui_with_rt(settings, tokio::runtime::Handle::current())
}

pub fn run_tui_with_rt(settings: &Settings, rt: tokio::runtime::Handle) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(settings.clone());

    'main: loop {
        // Drain background download messages.
        {
            let mut msgs: Vec<DownloadMsg> = Vec::new();
            let mut done = false;
            if let Some(rx) = &app.download_rx {
                loop {
                    match rx.try_recv() {
                        Ok(msg) => msgs.push(msg),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => { done = true; break; }
                    }
                }
            }
            for msg in msgs {
                match msg {
                    DownloadMsg::Log(text) => {
                        let level = if text.starts_with("Error") || text.starts_with("Warning") {
                            LogLevel::Error
                        } else if text.starts_with("Saved") {
                            LogLevel::Success
                        } else {
                            LogLevel::Info
                        };
                        app.log(text, level);
                    }
                    DownloadMsg::SessionReady(session) => {
                        app.session = Some(session);
                    }
                    DownloadMsg::Done => { done = true; }
                }
            }
            if done {
                app.downloading = false;
                app.download_rx = None;
            }
        }

        terminal.draw(|f| draw(&mut app, f))?;

        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        let ev = event::read()?;

        // --- PKCE Enter handling (needs redirect_url before state changes) ---
        if let Event::Key(k) = &ev
            && k.code == KeyCode::Enter
                && let PkceFlowState::AwaitingRedirect { input, verifier, unique_key, .. } = &app.pkce_state {
                    let redirect_url = input.clone();
                    let verifier = verifier.clone();
                    let unique_key = unique_key.clone();
                    app.pkce_state = PkceFlowState::Idle;

                    let (tx, rx) = std::sync::mpsc::channel::<DownloadMsg>();
                    app.downloading = true;
                    app.download_rx = Some(rx);
                    app.log("Completing PKCE login...", LogLevel::Info);

                    let tx2 = tx.clone();
                    rt.spawn(async move {
                        let result: Result<Arc<Mutex<TidalSession>>, anyhow::Error> = async {
                            let settings = Settings::load()?;
                            let mut sess = TidalSession::new(settings)?;
                            sess.pkce_exchange_code(&redirect_url, &verifier, &unique_key)
                                .await
                                .map_err(|e| anyhow::anyhow!("PKCE exchange failed: {e}"))?;
                            let session = Arc::new(Mutex::new(sess));
                            Ok(session)
                        }.await;

                        match result {
                            Ok(session) => {
                                let _ = tx2.send(DownloadMsg::SessionReady(session));
                                let _ = tx2.send(DownloadMsg::Log("PKCE login successful.".into()));
                            }
                            Err(e) => { let _ = tx2.send(DownloadMsg::Log(format!("Error: PKCE login failed: {e}"))); }
                        }
                        let _ = tx2.send(DownloadMsg::Done);
                    });

                    let _ = handle_event(&mut app, ev);
                    continue 'main;
                }

        // --- Account action: OAuth login ---
        if let Event::Key(k) = &ev
            && k.code == KeyCode::Enter
                && app.screen == Screen::Account
                && !app.logout_confirm
                && !matches!(app.pkce_state, PkceFlowState::AwaitingRedirect { .. })
            {
                let selected = app.account_state.selected().unwrap_or(0);

                if selected == 0 {
                    // OAuth device flow — runs in background, prints URL to log
                    let session_slot = app.session.clone();
                    let (tx, rx) = std::sync::mpsc::channel::<DownloadMsg>();
                    app.downloading = true;
                    app.download_rx = Some(rx);
                    app.log("Starting OAuth login...", LogLevel::Info);

                    let tx2 = tx.clone();
                    rt.spawn(async move {
                        let result: Result<Arc<Mutex<TidalSession>>, anyhow::Error> = async {
                            let settings = Settings::load()?;
                            let session = if let Some(s) = session_slot {
                                s
                            } else {
                                Arc::new(Mutex::new(TidalSession::new(settings.clone())?))
                            };
                            {
                                let mut sess = session.lock().await;
                                let tx3 = tx2.clone();
                                sess.login_with_url_handler(move |url, code| {
                                    let _ = tx3.send(DownloadMsg::Log(
                                        format!("Visit: {url}  Code: {code}")
                                    ));
                                }).await?;
                            }
                            Ok(session)
                        }.await;

                        match result {
                            Ok(session) => {
                                let _ = tx2.send(DownloadMsg::SessionReady(session));
                                let _ = tx2.send(DownloadMsg::Log("Login successful.".into()));
                            }
                            Err(e) => { let _ = tx2.send(DownloadMsg::Log(format!("Error: Login failed: {e}"))); }
                        }
                        let _ = tx2.send(DownloadMsg::Done);
                    });

                    handle_event(&mut app, ev);
                    continue 'main;
                }

                if selected == 1 {
                    // PKCE start — build URL and store verifier for later exchange
                    let result: Result<(String, String, String)> = (|| {
                        let settings = Settings::load()?;
                        let sess = TidalSession::new(settings)?;
                        let (auth_url, verifier, unique_key) = sess.pkce_build_auth_url();
                        Ok((auth_url, verifier, unique_key))
                    })();

                    match result {
                        Ok((auth_url, verifier, unique_key)) => {
                            app.pkce_state = PkceFlowState::AwaitingRedirect {
                                auth_url,
                                verifier,
                                unique_key,
                                input: String::new(),
                                cursor: 0,
                            };
                        }
                        Err(e) => app.log(format!("Error: {e}"), LogLevel::Error),
                    }
                    handle_event(&mut app, ev);
                    continue 'main;
                }
            }

        // --- Download trigger ---
        let should_download = app.screen == Screen::Download
            && !app.url_input.is_empty()
            && !app.downloading
            && matches!(ev, Event::Key(k) if k.code == KeyCode::Enter);

        if !handle_event(&mut app, ev) {
            break;
        }

        if should_download {
            let url = std::mem::take(&mut app.url_input);
            app.url_cursor = 0;
            app.downloading = true;
            app.log(format!("Queued: {url}"), LogLevel::Info);

            let (tx, rx) = std::sync::mpsc::channel::<DownloadMsg>();
            app.download_rx = Some(rx);

            // Ensure we have a session before spawning.
            let session = if let Some(s) = &app.session {
                Arc::clone(s)
            } else {
                // Create and login synchronously so errors surface immediately.
                let result: Result<Arc<Mutex<TidalSession>>> = rt.block_on(async {
                    let settings = Settings::load()?;
                    let mut sess = TidalSession::new(settings)?;
                    sess.login().await?;
                    let session = Arc::new(Mutex::new(sess));
                    Ok(session)
                });
                match result {
                    Ok(s) => {
                        app.session = Some(Arc::clone(&s));
                        s
                    }
                    Err(e) => {
                        app.log(format!("Error: login failed: {e}"), LogLevel::Error);
                        app.downloading = false;
                        continue;
                    }
                }
            };

            let settings = app.settings.clone();
            let tx2 = tx.clone();
            rt.spawn(async move {
                let dl = Downloader::new(Arc::clone(&session), settings);

                // Redirect stdout lines to the channel by running and capturing println output.
                // Since Downloader uses println!, we intercept by wrapping the call and
                // reading back from the log. For now we pass through directly.
                let result = dl.download_url(&url).await;
                match result {
                    Ok(()) => { let _ = tx2.send(DownloadMsg::Log(format!("Saved: {url}"))); }
                    Err(e) => { let _ = tx2.send(DownloadMsg::Log(format!("Error: {url} — {e}"))); }
                }
                let _ = tx2.send(DownloadMsg::Done);
            });
        }

        // Update login status from token on each event cycle.
        let token = Token::load().unwrap_or_default();
        app.logged_in = token.is_valid();
        app.user_id = token.user_id;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
