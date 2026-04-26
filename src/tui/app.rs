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
use crate::tidal::session::TidalSession;

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

#[derive(Clone, Copy)]
enum AccountAction {
    LoginOAuth,
    LoginPkce,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    screen: Screen,
    settings: Settings,
    // Main menu
    menu_state: ListState,
    // Download
    url_input: String,
    url_cursor: usize,
    download_log: Vec<String>,
    downloading: bool,
    // Account
    logged_in: bool,
    user_id: Option<u64>,
    account_state: ListState,
    account_pending: Option<AccountAction>,
    logout_confirm: bool,
    // Settings
    settings_state: ListState,
    settings_editing: bool,
    settings_edit_buffer: String,
    settings_dropdown: bool,
    settings_dropdown_idx: usize,
    settings_fields: Vec<SettingsField>,
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
        SettingsField { label: "Playlist Create", get: |s| s.playlist_create.to_string(), set: |s, v| s.playlist_create=v=="true", options: None },
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
            menu_state,
            url_input: String::new(),
            url_cursor: 0,
            download_log: Vec::new(),
            downloading: false,
            logged_in,
            user_id,
            account_state,
            account_pending: None,
            logout_confirm: false,
            settings_state,
            settings_editing: false,
            settings_edit_buffer: String::new(),
            settings_dropdown: false,
            settings_dropdown_idx: 0,
            settings_fields: make_settings_fields(),
        }
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

    let title = Paragraph::new(Line::from(
        [
            Span::styled(" tdl ", Style::default().bold().cyan()),
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

    let help = Paragraph::new("Tab switch  ↑↓ navigate  Enter select")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

fn draw_download(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).split(area);

    let input_text = if app.downloading {
        format!("{}█ (downloading...)", app.url_input)
    } else {
        format!("{}█", app.url_input)
    };
    let input_style = if app.downloading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(Line::from(Span::styled(input_text, input_style)))
        .block(Block::default().title(" URL (Enter to download, Tab switch, Esc back) ").borders(Borders::ALL));
    frame.render_widget(input, chunks[0]);

    let log_items: Vec<ListItem> = app
        .download_log
        .iter()
        .rev()
        .take(50)
        .map(|l| {
            let style = if l.starts_with("Error") {
                Style::default().fg(Color::Red)
            } else if l.starts_with("Saved") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(l.clone(), style)))
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
            width: 50.min(area.width - 20),
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
        if let Some(field) = app.settings_fields.get(sel_idx) {
            if let Some(options) = &field.options {
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
                        let marker = if i == app.settings_dropdown_idx {
                            " > "
                        } else {
                            "   "
                        };
                        let style = if i == app.settings_dropdown_idx {
                            Style::default().bold().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        ListItem::new(Line::from(Span::styled(
                            format!("{}{}", marker, opt),
                            style,
                        )))
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
}

fn draw_account(app: &mut App, frame: &mut Frame, area: Rect) {
    let status = if app.logged_in {
        format!("Logged in as user {}", app.user_id.unwrap_or(0))
    } else {
        "Not logged in".to_string()
    };
    let status_color = if app.logged_in { Color::Green } else { Color::Red };

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

fn handle_main(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            let idx = app.menu_state.selected().unwrap_or(0);
            if idx > 0 {
                app.menu_state.select(Some(idx - 1));
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let idx = app.menu_state.selected().unwrap_or(0);
            if idx < 4 {
                app.menu_state.select(Some(idx + 1));
            }
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
        KeyCode::Enter => {
            if !app.url_input.is_empty() && !app.downloading {
                // Download will be triggered from the main loop
            }
        }
        KeyCode::Backspace => {
            if app.url_cursor > 0 {
                app.url_cursor -= 1;
                app.url_input.remove(app.url_cursor);
            }
        }
        KeyCode::Left => {
            if app.url_cursor > 0 {
                app.url_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.url_cursor < app.url_input.len() {
                app.url_cursor += 1;
            }
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
            KeyCode::Backspace => {
                app.settings_edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                app.settings_edit_buffer.push(c);
            }
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
                if app.settings_dropdown_idx > 0 {
                    app.settings_dropdown_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.settings_dropdown_idx + 1 < opt_count {
                    app.settings_dropdown_idx += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(field) = app.settings_fields.get(sel_idx) {
                    if let Some(options) = &field.options {
                        if let Some(&opt) = options.get(app.settings_dropdown_idx) {
                            (field.set)(&mut app.settings, opt);
                        }
                    }
                }
                app.settings_dropdown = false;
            }
            KeyCode::Esc => {
                app.settings_dropdown = false;
            }
            _ => {}
        }
    } else {
        match code {
            KeyCode::Esc => app.screen = Screen::Main,
            KeyCode::Up | KeyCode::Char('k') => {
                let idx = app.settings_state.selected().unwrap_or(0);
                if idx > 0 {
                    app.settings_state.select(Some(idx - 1));
                }
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
                if let Err(e) = app.settings.save() {
                    app.download_log.push(format!("Error saving: {e}"));
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
                app.logout_confirm = false;
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
            if idx > 0 {
                app.account_state.select(Some(idx - 1));
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let idx = app.account_state.selected().unwrap_or(0);
            if idx < 2 {
                app.account_state.select(Some(idx + 1));
            }
        }
        KeyCode::Enter => match app.account_state.selected().unwrap_or(0) {
            0 => app.account_pending = Some(AccountAction::LoginOAuth),
            1 => app.account_pending = Some(AccountAction::LoginPkce),
            2 => {
                if app.logged_in {
                    app.logout_confirm = true;
                }
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

    loop {
        terminal.draw(|f| draw(&mut app, f))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;

            let should_download = app.screen == Screen::Download
                && !app.url_input.is_empty()
                && !app.downloading
                && matches!(ev, Event::Key(k) if k.code == KeyCode::Enter);

            let account_action = app.account_pending.take();

            if !handle_event(&mut app, ev) {
                break;
            }

            // Download
            if should_download {
                let url = app.url_input.clone();
                app.download_log.push(format!("Downloading: {}", url));
                app.downloading = true;
                app.url_input.clear();
                app.url_cursor = 0;

                disable_raw_mode()?;
                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                terminal.show_cursor()?;

                let result: Result<()> = rt.block_on(async {
                    let settings = Settings::load()?;
                    let mut session = TidalSession::new(settings.clone())?;
                    session.login().await?;
                    let session = Arc::new(Mutex::new(session));
                    let dl = Downloader::new(session, settings);
                    dl.download_url(&url).await
                });

                match result {
                    Ok(()) => app.download_log.push(format!("Saved: {}", url)),
                    Err(e) => app.download_log.push(format!("Error: {} - {}", url, e)),
                }
                app.downloading = false;

                enable_raw_mode()?;
                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            }

            // Account action (login)
            if let Some(action) = account_action {
                disable_raw_mode()?;
                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                terminal.show_cursor()?;

                let result: Result<()> = rt.block_on(async {
                    let settings = Settings::load()?;
                    let mut session = TidalSession::new(settings)?;
                    match action {
                        AccountAction::LoginOAuth => session.login().await?,
                        AccountAction::LoginPkce => session.login_pkce().await?,
                    }
                    Ok(())
                });

                match result {
                    Ok(()) => {
                        let token = Token::load().unwrap_or_default();
                        app.logged_in = token.is_valid();
                        app.user_id = token.user_id;
                    }
                    Err(e) => eprintln!("Login failed: {}", e),
                }

                println!("\nPress Enter to return to TUI...");
                let _ = std::io::stdin().read_line(&mut String::new());

                enable_raw_mode()?;
                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            }
        }
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
