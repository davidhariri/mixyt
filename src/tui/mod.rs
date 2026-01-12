use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Gauge},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

use crate::config::Config;
use crate::db::Database;
use crate::ipc::DaemonClient;
use crate::models::{PlaybackState, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Library,
    Queue,
    Playlists,
}

pub struct Tui {
    config: Config,
    db: Database,
    client: DaemonClient,
    tracks: Vec<Track>,
    playlists: Vec<crate::models::Playlist>,
    selected_panel: Panel,
    library_state: ListState,
    queue_state: ListState,
    playlist_state: ListState,
    playback_state: PlaybackState,
    search_query: String,
    search_mode: bool,
}

impl Tui {
    pub fn new(config: Config, db: Database) -> Result<Self> {
        let client = DaemonClient::new(config.socket_path());
        let tracks = db.get_all_tracks()?;
        let playlists = db.get_all_playlists()?;

        let playback_state = if client.is_daemon_running() {
            client.get_status().unwrap_or_default()
        } else {
            PlaybackState::default()
        };

        let mut library_state = ListState::default();
        if !tracks.is_empty() {
            library_state.select(Some(0));
        }

        Ok(Self {
            config,
            db,
            client,
            tracks,
            playlists,
            selected_panel: Panel::Library,
            library_state,
            queue_state: ListState::default(),
            playlist_state: ListState::default(),
            playback_state,
            search_query: String::new(),
            search_mode: false,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.main_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            // Refresh playback state
            if self.client.is_daemon_running() {
                if let Ok(state) = self.client.get_status() {
                    self.playback_state = state;
                }
            }

            terminal.draw(|f| self.ui(f))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.search_mode {
                            match key.code {
                                KeyCode::Esc => {
                                    self.search_mode = false;
                                    self.search_query.clear();
                                }
                                KeyCode::Enter => {
                                    self.search_mode = false;
                                    self.apply_search();
                                }
                                KeyCode::Backspace => {
                                    self.search_query.pop();
                                }
                                KeyCode::Char(c) => {
                                    self.search_query.push(c);
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('/') => {
                                    self.search_mode = true;
                                }
                                KeyCode::Tab => self.next_panel(),
                                KeyCode::BackTab => self.prev_panel(),
                                KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
                                KeyCode::Down | KeyCode::Char('j') => self.select_next(),
                                KeyCode::Enter => self.play_selected(),
                                KeyCode::Char(' ') => self.toggle_playback(),
                                KeyCode::Char('n') => self.next_track(),
                                KeyCode::Char('p') => self.prev_track(),
                                KeyCode::Char('s') => self.toggle_shuffle(),
                                KeyCode::Char('r') => self.cycle_repeat(),
                                KeyCode::Char('+') | KeyCode::Char('=') => self.volume_up(),
                                KeyCode::Char('-') => self.volume_down(),
                                KeyCode::Char('a') => self.add_to_queue(),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Now playing
                Constraint::Min(10),    // Main content
                Constraint::Length(3),  // Controls
                Constraint::Length(1),  // Help
            ])
            .split(f.area());

        self.render_now_playing(f, chunks[0]);
        self.render_main_content(f, chunks[1]);
        self.render_controls(f, chunks[2]);
        self.render_help(f, chunks[3]);
    }

    fn render_now_playing(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Now Playing ")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        f.render_widget(block, area);

        if let Some(track) = &self.playback_state.current_track {
            let status = if self.playback_state.is_playing { "▶" } else { "⏸" };
            let text = format!("{} {} - {}", status, track.display_name(), track.format_duration());

            // Progress bar
            let progress = if track.duration > 0 {
                (self.playback_state.position as f64 / track.duration as f64).min(1.0)
            } else {
                0.0
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1)])
                .split(inner);

            let title = Paragraph::new(text);
            f.render_widget(title, chunks[0]);

            let gauge = Gauge::default()
                .ratio(progress)
                .gauge_style(Style::default().fg(Color::Cyan));
            f.render_widget(gauge, chunks[1]);
        } else {
            let text = Paragraph::new("Nothing playing");
            f.render_widget(text, inner);
        }
    }

    fn render_main_content(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(area);

        // Library
        let library_block = Block::default()
            .title(format!(" Library ({}) ", self.tracks.len()))
            .borders(Borders::ALL)
            .border_style(if self.selected_panel == Panel::Library {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let items: Vec<ListItem> = self
            .tracks
            .iter()
            .map(|t| {
                let style = if !t.available {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{} - {}", t.display_name(), t.format_duration()))
                    .style(style)
            })
            .collect();

        let list = List::new(items)
            .block(library_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, chunks[0], &mut self.library_state.clone());

        // Right panel (Queue or Playlists)
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);

        // Queue
        let queue_block = Block::default()
            .title(format!(" Queue ({}) ", self.playback_state.queue.len()))
            .borders(Borders::ALL)
            .border_style(if self.selected_panel == Panel::Queue {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let queue_items: Vec<ListItem> = self
            .playback_state
            .queue
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let marker = if i == self.playback_state.queue_index { "▶ " } else { "  " };
                ListItem::new(format!("{}{}", marker, t.display_name()))
            })
            .collect();

        let queue_list = List::new(queue_items)
            .block(queue_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        f.render_stateful_widget(queue_list, right_chunks[0], &mut self.queue_state.clone());

        // Playlists
        let playlist_block = Block::default()
            .title(format!(" Playlists ({}) ", self.playlists.len()))
            .borders(Borders::ALL)
            .border_style(if self.selected_panel == Panel::Playlists {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let playlist_items: Vec<ListItem> = self
            .playlists
            .iter()
            .map(|p| ListItem::new(p.name.clone()))
            .collect();

        let playlist_list = List::new(playlist_items)
            .block(playlist_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        f.render_stateful_widget(playlist_list, right_chunks[1], &mut self.playlist_state.clone());
    }

    fn render_controls(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Controls ")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        f.render_widget(block, area);

        let shuffle = if self.playback_state.shuffle { "🔀" } else { "  " };
        let repeat = match self.playback_state.repeat {
            crate::models::RepeatMode::Off => "  ",
            crate::models::RepeatMode::One => "🔂",
            crate::models::RepeatMode::All => "🔁",
        };

        let text = format!(
            "Volume: {}%  {} Shuffle  {} Repeat: {}",
            self.playback_state.volume,
            shuffle,
            repeat,
            self.playback_state.repeat
        );

        let para = Paragraph::new(text);
        f.render_widget(para, inner);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help_text = if self.search_mode {
            format!("Search: {}█  (Enter to search, Esc to cancel)", self.search_query)
        } else {
            "q:Quit  /:Search  Tab:Switch  ↑↓:Navigate  Enter:Play  Space:Pause  n/p:Next/Prev  s:Shuffle  r:Repeat  +/-:Volume  a:Add to queue".to_string()
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(help, area);
    }

    fn next_panel(&mut self) {
        self.selected_panel = match self.selected_panel {
            Panel::Library => Panel::Queue,
            Panel::Queue => Panel::Playlists,
            Panel::Playlists => Panel::Library,
        };
    }

    fn prev_panel(&mut self) {
        self.selected_panel = match self.selected_panel {
            Panel::Library => Panel::Playlists,
            Panel::Queue => Panel::Library,
            Panel::Playlists => Panel::Queue,
        };
    }

    fn current_list_state(&mut self) -> &mut ListState {
        match self.selected_panel {
            Panel::Library => &mut self.library_state,
            Panel::Queue => &mut self.queue_state,
            Panel::Playlists => &mut self.playlist_state,
        }
    }

    fn current_list_len(&self) -> usize {
        match self.selected_panel {
            Panel::Library => self.tracks.len(),
            Panel::Queue => self.playback_state.queue.len(),
            Panel::Playlists => self.playlists.len(),
        }
    }

    fn select_next(&mut self) {
        let len = self.current_list_len();
        if len == 0 {
            return;
        }

        let state = self.current_list_state();
        let i = match state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        state.select(Some(i));
    }

    fn select_prev(&mut self) {
        let len = self.current_list_len();
        if len == 0 {
            return;
        }

        let state = self.current_list_state();
        let i = match state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        state.select(Some(i));
    }

    fn play_selected(&mut self) {
        if self.selected_panel != Panel::Library {
            return;
        }

        if let Some(i) = self.library_state.selected() {
            if let Some(track) = self.tracks.get(i) {
                if track.available {
                    let _ = self.client.play(track.clone());
                }
            }
        }
    }

    fn toggle_playback(&mut self) {
        if self.playback_state.is_playing {
            let _ = self.client.pause();
        } else {
            let _ = self.client.resume();
        }
    }

    fn next_track(&mut self) {
        let _ = self.client.next();
    }

    fn prev_track(&mut self) {
        let _ = self.client.previous();
    }

    fn toggle_shuffle(&mut self) {
        let _ = self.client.set_shuffle(!self.playback_state.shuffle);
    }

    fn cycle_repeat(&mut self) {
        let new_mode = match self.playback_state.repeat {
            crate::models::RepeatMode::Off => crate::models::RepeatMode::All,
            crate::models::RepeatMode::All => crate::models::RepeatMode::One,
            crate::models::RepeatMode::One => crate::models::RepeatMode::Off,
        };
        let _ = self.client.set_repeat(new_mode);
    }

    fn volume_up(&mut self) {
        let vol = (self.playback_state.volume + 5).min(100);
        let _ = self.client.set_volume(vol);
    }

    fn volume_down(&mut self) {
        let vol = self.playback_state.volume.saturating_sub(5);
        let _ = self.client.set_volume(vol);
    }

    fn add_to_queue(&mut self) {
        if self.selected_panel != Panel::Library {
            return;
        }

        if let Some(i) = self.library_state.selected() {
            if let Some(track) = self.tracks.get(i) {
                let _ = self.client.queue_add(track.clone());
            }
        }
    }

    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        // Filter tracks based on search
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let matcher = SkimMatcherV2::default();
        let query = &self.search_query;

        let mut matches: Vec<_> = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(i, track)| {
                let title_score = matcher.fuzzy_match(&track.title, query).unwrap_or(0);
                let alias_score = track
                    .alias
                    .as_ref()
                    .and_then(|a| matcher.fuzzy_match(a, query))
                    .unwrap_or(0);
                let score = title_score.max(alias_score);
                if score > 0 {
                    Some((i, score))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|a, b| b.1.cmp(&a.1));

        if let Some((index, _)) = matches.first() {
            self.library_state.select(Some(*index));
        }

        self.search_query.clear();
    }
}

pub fn run(config: Config, db: Database) -> Result<()> {
    let mut tui = Tui::new(config, db)?;
    tui.run()
}
