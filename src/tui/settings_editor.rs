use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::config::settings::{CoverDimensions, PlaylistFormat, Quality, QualityVideo, Settings};

// ---------------------------------------------------------------------------
// Setting field descriptor
// ---------------------------------------------------------------------------

enum SettingKind {
    Bool,
    Enum { options: Vec<String> },
    Number,
    Text,
}

struct SettingField {
    label: &'static str,
    description: &'static str,
    kind: SettingKind,
    get: fn(&Settings) -> String,
    set: fn(&mut Settings, &str),
}

fn settings_fields() -> Vec<SettingField> {
    vec![
        // --- Download ---
        SettingField {
            label: "Quality Audio",
            description: "Audio download quality",
            kind: SettingKind::Enum {
                options: vec![
                    "low_96k".into(),
                    "low_320k".into(),
                    "high_lossless".into(),
                    "hi_res_lossless".into(),
                ],
            },
            get: |s| serde_json::to_value(&s.quality_audio).unwrap().as_str().unwrap().to_string(),
            set: |s, v| {
                s.quality_audio = match v {
                    "low_96k" => Quality::Low96k,
                    "low_320k" => Quality::Low320k,
                    "high_lossless" => Quality::HighLossless,
                    "hi_res_lossless" => Quality::HiResLossless,
                    _ => return,
                };
            },
        },
        SettingField {
            label: "Quality Video",
            description: "Video download quality",
            kind: SettingKind::Enum {
                options: vec!["p360".into(), "p480".into(), "p720".into(), "p1080".into()],
            },
            get: |s| serde_json::to_value(&s.quality_video).unwrap().as_str().unwrap().to_string(),
            set: |s, v| {
                s.quality_video = match v {
                    "p360" => QualityVideo::P360,
                    "p480" => QualityVideo::P480,
                    "p720" => QualityVideo::P720,
                    "p1080" => QualityVideo::P1080,
                    _ => return,
                };
            },
        },
        SettingField {
            label: "Skip Existing",
            description: "Skip download if file already exists",
            kind: SettingKind::Bool,
            get: |s| s.skip_existing.to_string(),
            set: |s, v| s.skip_existing = v == "true",
        },
        SettingField {
            label: "Download Delay",
            description: "Random delay between downloads (anti-ban)",
            kind: SettingKind::Bool,
            get: |s| s.download_delay.to_string(),
            set: |s, v| s.download_delay = v == "true",
        },
        SettingField {
            label: "Delay Min (sec)",
            description: "Minimum download delay in seconds",
            kind: SettingKind::Number,
            get: |s| format!("{:.1}", s.download_delay_sec_min),
            set: |s, v| {
                if let Ok(val) = v.parse::<f64>() {
                    s.download_delay_sec_min = val;
                }
            },
        },
        SettingField {
            label: "Delay Max (sec)",
            description: "Maximum download delay in seconds",
            kind: SettingKind::Number,
            get: |s| format!("{:.1}", s.download_delay_sec_max),
            set: |s, v| {
                if let Ok(val) = v.parse::<f64>() {
                    s.download_delay_sec_max = val;
                }
            },
        },
        SettingField {
            label: "Concurrent Max",
            description: "Max concurrent downloads (1-5)",
            kind: SettingKind::Number,
            get: |s| s.downloads_concurrent_max.to_string(),
            set: |s, v| {
                if let Ok(val) = v.parse::<usize>() {
                    s.downloads_concurrent_max = val.clamp(1, 5);
                }
            },
        },
        SettingField {
            label: "Segments Per Track",
            description: "Max parallel segment downloads per track",
            kind: SettingKind::Number,
            get: |s| s.downloads_simultaneous_per_track_max.to_string(),
            set: |s, v| {
                if let Ok(val) = v.parse::<usize>() {
                    s.downloads_simultaneous_per_track_max = val;
                }
            },
        },
        // --- Paths ---
        SettingField {
            label: "Download Base Path",
            description: "Where to store downloaded media",
            kind: SettingKind::Text,
            get: |s| s.download_base_path.clone(),
            set: |s, v| s.download_base_path = v.to_string(),
        },
        SettingField {
            label: "FFmpeg Path",
            description: "Path to FFmpeg binary (empty = auto-detect)",
            kind: SettingKind::Text,
            get: |s| s.path_binary_ffmpeg.clone(),
            set: |s, v| s.path_binary_ffmpeg = v.to_string(),
        },
        SettingField {
            label: "Track Num Zero Pad",
            description: "Zero-pad track numbers (e.g. 01, 02)",
            kind: SettingKind::Bool,
            get: |s| s.track_num_pad_zero.to_string(),
            set: |s, v| s.track_num_pad_zero = v == "true",
        },
        // --- Video ---
        SettingField {
            label: "Video Download",
            description: "Allow video downloads",
            kind: SettingKind::Bool,
            get: |s| s.video_download.to_string(),
            set: |s, v| s.video_download = v == "true",
        },
        SettingField {
            label: "Video Convert MP4",
            description: "Convert TS videos to MP4 (requires FFmpeg)",
            kind: SettingKind::Bool,
            get: |s| s.video_convert_mp4.to_string(),
            set: |s, v| s.video_convert_mp4 = v == "true",
        },
        // --- Metadata ---
        SettingField {
            label: "Cover Dimension",
            description: "Embedded cover art resolution",
            kind: SettingKind::Enum {
                options: vec![
                    "px80".into(),
                    "px160".into(),
                    "px320".into(),
                    "px640".into(),
                    "px1280".into(),
                ],
            },
            get: |s| {
                serde_json::to_value(&s.metadata_cover_dimension)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            },
            set: |s, v| {
                s.metadata_cover_dimension = match v {
                    "px80" => CoverDimensions::Px80,
                    "px160" => CoverDimensions::Px160,
                    "px320" => CoverDimensions::Px320,
                    "px640" => CoverDimensions::Px640,
                    "px1280" => CoverDimensions::Px1280,
                    _ => return,
                };
            },
        },
        SettingField {
            label: "Cover Embed",
            description: "Embed cover art in audio files",
            kind: SettingKind::Bool,
            get: |s| s.metadata_cover_embed.to_string(),
            set: |s, v| s.metadata_cover_embed = v == "true",
        },
        SettingField {
            label: "Cover Album File",
            description: "Save cover.jpg alongside album tracks",
            kind: SettingKind::Bool,
            get: |s| s.cover_album_file.to_string(),
            set: |s, v| s.cover_album_file = v == "true",
        },
        SettingField {
            label: "Extract FLAC",
            description: "Extract FLAC from MP4 containers",
            kind: SettingKind::Bool,
            get: |s| s.extract_flac.to_string(),
            set: |s, v| s.extract_flac = v == "true",
        },
        SettingField {
            label: "Lyrics Embed",
            description: "Embed lyrics in audio files",
            kind: SettingKind::Bool,
            get: |s| s.lyrics_embed.to_string(),
            set: |s, v| s.lyrics_embed = v == "true",
        },
        SettingField {
            label: "Lyrics File",
            description: "Save lyrics as separate .lrc files",
            kind: SettingKind::Bool,
            get: |s| s.lyrics_file.to_string(),
            set: |s, v| s.lyrics_file = v == "true",
        },
        SettingField {
            label: "ReplayGain",
            description: "Write ReplayGain metadata",
            kind: SettingKind::Bool,
            get: |s| s.metadata_replay_gain.to_string(),
            set: |s, v| s.metadata_replay_gain = v == "true",
        },
        // --- Other ---
        SettingField {
            label: "Symlink to Track",
            description: "Symlink album/playlist tracks to track dir",
            kind: SettingKind::Bool,
            get: |s| s.symlink_to_track.to_string(),
            set: |s, v| s.symlink_to_track = v == "true",
        },
        SettingField {
            label: "Playlist Folder",
            description: "Save playlists under Playlists/ folder and generate playlist file",
            kind: SettingKind::Bool,
            get: |s| s.playlist_folder.to_string(),
            set: |s, v| s.playlist_folder = v == "true",
        },
        SettingField {
            label: "Playlist Format",
            description: "Playlist file format: m3u8 (UTF-8, recommended) or m3u (Apple Music compatible)",
            kind: SettingKind::Enum {
                options: vec!["m3u8".into(), "m3u".into()],
            },
            get: |s| match s.playlist_format {
                PlaylistFormat::M3u8 => "m3u8".to_string(),
                PlaylistFormat::M3u => "m3u".to_string(),
            },
            set: |s, v| {
                s.playlist_format = match v {
                    "m3u8" => PlaylistFormat::M3u8,
                    "m3u" => PlaylistFormat::M3u,
                    _ => return,
                };
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    settings: Settings,
    fields: Vec<SettingField>,
    list_state: ListState,
    editing: bool,
    edit_buffer: String,
    enum_options: Vec<String>,
    enum_index: usize,
    status_message: String,
    is_pkce: bool,
}

impl App {
    fn new(settings: Settings, is_pkce: bool) -> Self {
        let fields = settings_fields();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            settings,
            fields,
            list_state,
            editing: false,
            edit_buffer: String::new(),
            enum_options: Vec::new(),
            enum_index: 0,
            status_message: String::new(),
            is_pkce,
        }
    }

    fn current_field(&self) -> Option<&SettingField> {
        let idx = self.list_state.selected()?;
        self.fields.get(idx)
    }

    fn start_edit(&mut self) {
        let idx = match self.list_state.selected() {
            Some(i) => i,
            None => return,
        };
        let field = match self.fields.get(idx) {
            Some(f) => f,
            None => return,
        };

        match &field.kind {
            SettingKind::Bool => {
                let val = (field.get)(&self.settings);
                let new_val = if val == "true" { "false" } else { "true" };
                let label = field.label;
                (field.set)(&mut self.settings, new_val);
                let new_val = (field.get)(&self.settings);
                self.status_message = format!("{} = {}", label, new_val);
            }
            SettingKind::Enum { options } => {
                let current = (field.get)(&self.settings);
                let idx = options.iter().position(|o| o == &current).unwrap_or(0);
                self.editing = true;
                self.enum_options = options.clone();
                self.enum_index = idx;
                // HiRes 옵션 이름에 (PKCE only) 표시 추가
                if field.label == "Quality Audio" {
                    self.enum_options = options
                        .iter()
                        .map(|o| {
                            if o == "hi_res_lossless" {
                                format!("{} (PKCE only)", o)
                            } else {
                                o.clone()
                            }
                        })
                        .collect();
                    self.enum_index = idx;
                }
            }
            SettingKind::Number | SettingKind::Text => {
                self.editing = true;
                self.edit_buffer = (field.get)(&self.settings);
            }
        }
    }

    fn confirm_edit(&mut self) {
        if !self.editing {
            return;
        }
        let idx = match self.list_state.selected() {
            Some(i) => i,
            None => return,
        };
        let field = match self.fields.get(idx) {
            Some(f) => f,
            None => return,
        };

        match &field.kind {
            SettingKind::Enum { .. } => {
                if let Some(opt) = self.enum_options.get(self.enum_index) {
                    // HiRes는 PKCE 로그인 없이 선택 불가
                    if opt.starts_with("hi_res_lossless") && !self.is_pkce {
                        self.status_message =
                            "HiRes Lossless requires PKCE login. Run: tdl login --pkce".to_string();
                        self.editing = false;
                        self.enum_options.clear();
                        return;
                    }
                    let label = field.label;
                    // enum_options에 "(PKCE only)" suffix가 붙어 있을 수 있으므로 원본 key만 추출
                    let raw_opt = opt.split_once(' ').map(|(k, _)| k).unwrap_or(opt);
                    (field.set)(&mut self.settings, raw_opt);
                    self.status_message = format!("{} = {}", label, raw_opt);
                }
            }
            _ => {
                let label = field.label;
                let buf = self.edit_buffer.clone();
                (field.set)(&mut self.settings, &buf);
                self.status_message = format!("{} = {}", label, buf);
            }
        }
        self.editing = false;
        self.edit_buffer.clear();
        self.enum_options.clear();
    }

    fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
        self.enum_options.clear();
        self.status_message.clear();
    }

    fn move_up(&mut self) {
        if self.editing && !self.enum_options.is_empty() {
            if self.enum_index > 0 {
                self.enum_index -= 1;
            }
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        if idx > 0 {
            self.list_state.select(Some(idx - 1));
        }
    }

    fn move_down(&mut self) {
        if self.editing && !self.enum_options.is_empty() {
            if self.enum_index < self.enum_options.len() - 1 {
                self.enum_index += 1;
            }
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        if idx < self.fields.len() - 1 {
            self.list_state.select(Some(idx + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn draw(app: &mut App, frame: &mut Frame) {
    let size = frame.area();

    // Layout: title | main list | status bar
    let chunks = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Min(10),   // settings list
        Constraint::Length(3), // help / status
    ])
    .split(size);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" tdl ", Style::default().bold().cyan()),
        Span::raw(" Settings Editor"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Settings list
    let items: Vec<ListItem> = app
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let value = (field.get)(&app.settings);
            let is_selected = app.list_state.selected() == Some(i);

            let label_style = if is_selected && !app.editing {
                Style::default().bold().yellow()
            } else {
                Style::default()
            };

            let value_style = if is_selected && app.editing {
                Style::default().bold().green()
            } else if is_selected {
                Style::default().bold().white()
            } else {
                Style::default().fg(ratatui::style::Color::DarkGray)
            };

            let label_width = 22;
            let label_padded = format!("{:<width$}", field.label, width = label_width);

            let line = Line::from(vec![
                Span::styled(label_padded, label_style),
                Span::styled(": ", Style::default()),
                Span::styled(value, value_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title_style(Style::default().bold()),
        )
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Draw inline editor overlay when editing
    if app.editing {
        draw_editor(app, frame, chunks[1]);
    }

    // Description + help bar
    let help_text = if app.editing {
        let desc = match app.editing {
            true if !app.enum_options.is_empty() => "↑↓ select  Enter confirm  Esc cancel",
            true => "Type value  Enter confirm  Esc cancel",
            false => "",
        };
        desc.to_string()
    } else {
        let desc = app
            .current_field()
            .map(|f| f.description)
            .unwrap_or("");
        format!(
            "↑↓ navigate  Enter edit  s save  q quit  │ {}",
            desc
        )
    };

    let status_line = if app.status_message.is_empty() {
        help_text
    } else {
        format!("{}  │ {}", app.status_message, help_text)
    };

    let status = Paragraph::new(Line::from(Span::styled(
        status_line,
        Style::default().dark_gray(),
    )))
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(status, chunks[2]);
}

fn draw_editor(app: &App, frame: &mut Frame, area: Rect) {
    let selected = app.list_state.selected().unwrap_or(0);

    // Calculate the popup position (centered)
    let popup_width = 50.min(area.width - 4);
    let popup_height = if !app.enum_options.is_empty() {
        (app.enum_options.len() as u16 + 4).min(area.height - 2)
    } else {
        5
    };
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + 2 + selected as u16;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    if !app.enum_options.is_empty() {
        // Enum selector
        let items: Vec<ListItem> = app
            .enum_options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let style = if i == app.enum_index {
                    Style::default().bold().green()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(opt.clone(), style)))
            })
            .collect();

        let label = app.current_field().map(|f| f.label).unwrap_or("Select");
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", label))
                .style(Style::default().green()),
        );

        let mut state = ListState::default();
        state.select(Some(app.enum_index));
        frame.render_stateful_widget(list, popup_area, &mut state);
    } else {
        // Text/number input
        let label = app.current_field().map(|f| f.label).unwrap_or("Edit");
        let input = Paragraph::new(Line::from(vec![
            Span::styled(&app.edit_buffer, Style::default().yellow()),
            Span::styled("█", Style::default().white()),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", label))
                .style(Style::default().green()),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(input, popup_area);
    }
}

// ---------------------------------------------------------------------------
// Event handling
// ---------------------------------------------------------------------------

fn handle_event(app: &mut App, event: Event) -> bool {
    if let Event::Key(key) = event {
        if app.editing {
            match key.code {
                KeyCode::Enter => app.confirm_edit(),
                KeyCode::Esc => app.cancel_edit(),
                KeyCode::Up => app.move_up(),
                KeyCode::Down => app.move_down(),
                KeyCode::Backspace => {
                    app.edit_buffer.pop();
                }
                KeyCode::Char(c) => {
                    app.edit_buffer.push(c);
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Enter | KeyCode::Char(' ') => app.start_edit(),
                KeyCode::Char('s') => {
                    if let Err(e) = app.settings.save() {
                        app.status_message = format!("Error: {e}");
                    } else {
                        app.status_message = "Settings saved.".to_string();
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => return false,
                _ => {}
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run_settings_editor(settings: &Settings, is_pkce: bool) -> Result<Settings> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(settings.clone(), is_pkce);

    loop {
        terminal.draw(|f| draw(&mut app, f))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            if !handle_event(&mut app, ev) {
                break;
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(app.settings)
}
