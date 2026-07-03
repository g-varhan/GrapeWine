use std::io;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use crate::orchestrator::{Orchestrator, GameConfig, Runner};
use crate::plugins::{PluginManager, GameInfo};

// Visual theme colors (Mocha styled)
const COLOR_BG: Color = Color::Rgb(30, 30, 46);
const COLOR_PANEL: Color = Color::Rgb(17, 17, 27);
const COLOR_TEXT: Color = Color::Rgb(205, 214, 244);
const COLOR_PRIMARY: Color = Color::Rgb(180, 190, 254); // Lavender
const COLOR_SECONDARY: Color = Color::Rgb(137, 180, 250); // Sapphire
const COLOR_SUCCESS: Color = Color::Rgb(166, 227, 161); // Green
const COLOR_HIGHLIGHT: Color = Color::Rgb(249, 226, 175); // Peach
const COLOR_MUTED: Color = Color::Rgb(108, 112, 134); // Subtext

#[derive(PartialEq)]
enum ActiveTab {
    Home,
    Search,
    Runners,
    Plugins,
}

pub struct TuiApp {
    tab: ActiveTab,
    orchestrator: Orchestrator,
    plugin_manager: PluginManager,
    
    // Game Library State (Home)
    games: Vec<GameConfig>,
    game_list_state: ListState,
    
    // Search State
    search_query: String,
    search_results: Vec<GameInfo>,
    search_list_state: ListState,
    
    // Runner State
    runner_list_state: ListState,
    
    // Plugin State
    plugin_list_state: ListState,

    // Global Command Palette State
    command_palette_active: bool,
    palette_query: String,
    palette_results: Vec<GameConfig>,
    palette_list_state: ListState,
    
    // Launching state
    currently_running_game: Arc<Mutex<Option<String>>>, // Contains title of playing game
}

impl TuiApp {
    pub fn new(orchestrator: Orchestrator, plugin_manager: PluginManager, initial_games: Vec<GameConfig>) -> Self {
        let mut app = Self {
            tab: ActiveTab::Home,
            orchestrator,
            plugin_manager,
            games: initial_games,
            game_list_state: ListState::default(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_list_state: ListState::default(),
            runner_list_state: ListState::default(),
            plugin_list_state: ListState::default(),
            
            command_palette_active: false,
            palette_query: String::new(),
            palette_results: Vec::new(),
            palette_list_state: ListState::default(),
            
            currently_running_game: Arc::new(Mutex::new(None)),
        };
        
        if !app.games.is_empty() {
            app.game_list_state.select(Some(0));
        }
        
        app.update_palette_results();
        app
    }

    pub fn save_cache(&self) {
        self.plugin_manager.save_cache();
    }

    fn update_palette_results(&mut self) {
        if self.palette_query.is_empty() {
            self.palette_results = self.games.clone();
        } else {
            let q = self.palette_query.to_lowercase();
            self.palette_results = self.games
                .iter()
                .filter(|g| g.title.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        
        if self.palette_results.is_empty() {
            self.palette_list_state.select(None);
        } else {
            self.palette_list_state.select(Some(0));
        }
    }

    fn handle_search(&mut self) {
        self.search_results.clear();
        for plugin_id in self.plugin_manager.plugins.keys() {
            if let Ok(res) = self.plugin_manager.search(plugin_id, &self.search_query) {
                self.search_results.extend(res);
            }
        }
        
        if !self.search_results.is_empty() {
            self.search_list_state.select(Some(0));
        } else {
            self.search_list_state.select(None);
        }
    }

    fn import_selected_search_game(&mut self) {
        if let Some(idx) = self.search_list_state.selected() {
            if let Some(game_info) = self.search_results.get(idx).cloned() {
                // Check if already in library
                if self.games.iter().any(|g| g.id == game_info.id) {
                    return;
                }
                
                let new_game = GameConfig {
                    id: game_info.id,
                    title: game_info.title,
                    source: game_info.source,
                    exe_path: game_info.exec_path,
                    launch_args: game_info.launch_args,
                    runner_path: None,
                    env_vars: HashMap::new(),
                    dxvk_enabled: true,
                    mangohud_enabled: false,
                    gamemode_enabled: false,
                    play_time_seconds: 0,
                    last_played: None,
                    download_progress: 0.0,
                    download_speed: String::new(),
                    download_eta: String::new(),
                    status: "installed".to_string(),
                };
                
                self.games.push(new_game);
                
                // Save database
                if let Ok(content) = serde_json::to_string_pretty(&self.games) {
                    let db_path = self.orchestrator.base_dir.join("library.json");
                    let _ = std::fs::write(db_path, content);
                }
            }
        }
    }

    fn launch_selected_game(&mut self, game: GameConfig) {
        // Run on background thread
        let orchestrator_clone = Orchestrator::new();
        let running_ref = Arc::clone(&self.currently_running_game);
        let game_title = game.title.clone();
        
        let show_popup = !game.exe_path.starts_with("magnet:") && !game.exe_path.starts_with("http://") && !game.exe_path.starts_with("https://");
        if show_popup {
            *running_ref.lock().unwrap() = Some(game_title);
        }

        let mut config_clone = game.clone();
        let base_dir = self.orchestrator.base_dir.clone();
        
        std::thread::spawn(move || {
            if let Err(e) = orchestrator_clone.launch_game(&mut config_clone) {
                println!("Launch/Install failed: {}", e);
            }
            
            // Clear running status
            *running_ref.lock().unwrap() = None;

            // Load and update the database with new game config (play times, paths, status)
            let db_path = base_dir.join("library.json");
            if db_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&db_path) {
                    if let Ok(mut current_games) = serde_json::from_str::<Vec<GameConfig>>(&content) {
                        for g in &mut current_games {
                            if g.id == config_clone.id {
                                g.play_time_seconds = config_clone.play_time_seconds;
                                g.last_played = config_clone.last_played.clone();
                                g.exe_path = config_clone.exe_path.clone();
                                g.status = config_clone.status.clone();
                            }
                        }
                        let _ = std::fs::write(&db_path, serde_json::to_string_pretty(&current_games).unwrap());
                    }
                }
            }
        });
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        let mut last_db_update = Instant::now();

        loop {
            // Periodically reload database play times when no game is running
            if last_db_update.elapsed() > Duration::from_secs(2) {
                let db_path = self.orchestrator.base_dir.join("library.json");
                if db_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(db_path) {
                        if let Ok(parsed) = serde_json::from_str::<Vec<GameConfig>>(&content) {
                            self.games = parsed;
                        }
                    }
                }
                last_db_update = Instant::now();
            }

            terminal.draw(|f| self.draw(f))?;

            // Event polling
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        // Global Ctrl+P Command Palette toggle
                        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            self.command_palette_active = !self.command_palette_active;
                            if self.command_palette_active {
                                self.palette_query.clear();
                                self.update_palette_results();
                            }
                            continue;
                        }

                        // Escape to close command palette
                        if self.command_palette_active {
                            match key.code {
                                KeyCode::Esc => {
                                    self.command_palette_active = false;
                                }
                                KeyCode::Char(c) => {
                                    self.palette_query.push(c);
                                    self.update_palette_results();
                                }
                                KeyCode::Backspace => {
                                    self.palette_query.pop();
                                    self.update_palette_results();
                                }
                                KeyCode::Up => {
                                    if let Some(idx) = self.palette_list_state.selected() {
                                        if idx > 0 {
                                            self.palette_list_state.select(Some(idx - 1));
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(idx) = self.palette_list_state.selected() {
                                        if idx + 1 < self.palette_results.len() {
                                            self.palette_list_state.select(Some(idx + 1));
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(idx) = self.palette_list_state.selected() {
                                        if let Some(game) = self.palette_results.get(idx).cloned() {
                                            self.command_palette_active = false;
                                            self.launch_selected_game(game);
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // Tab specific controls
                        match self.tab {
                            ActiveTab::Home => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('1') => self.tab = ActiveTab::Home,
                                KeyCode::Char('2') => self.tab = ActiveTab::Search,
                                KeyCode::Char('3') => self.tab = ActiveTab::Runners,
                                KeyCode::Char('4') => self.tab = ActiveTab::Plugins,
                                KeyCode::Tab => self.tab = ActiveTab::Search,
                                KeyCode::Up => {
                                    if let Some(idx) = self.game_list_state.selected() {
                                        if idx > 0 {
                                            self.game_list_state.select(Some(idx - 1));
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(idx) = self.game_list_state.selected() {
                                        if idx + 1 < self.games.len() {
                                            self.game_list_state.select(Some(idx + 1));
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(idx) = self.game_list_state.selected() {
                                        if let Some(game) = self.games.get(idx).cloned() {
                                            self.launch_selected_game(game);
                                        }
                                    }
                                }
                                _ => {}
                            },
                            ActiveTab::Search => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('1') => self.tab = ActiveTab::Home,
                                KeyCode::Char('2') => self.tab = ActiveTab::Search,
                                KeyCode::Char('3') => self.tab = ActiveTab::Runners,
                                KeyCode::Char('4') => self.tab = ActiveTab::Plugins,
                                KeyCode::Tab => self.tab = ActiveTab::Runners,
                                KeyCode::Char(c) => {
                                    self.search_query.push(c);
                                    self.handle_search();
                                }
                                KeyCode::Backspace => {
                                    self.search_query.pop();
                                    self.handle_search();
                                }
                                KeyCode::Up => {
                                    if let Some(idx) = self.search_list_state.selected() {
                                        if idx > 0 {
                                            self.search_list_state.select(Some(idx - 1));
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(idx) = self.search_list_state.selected() {
                                        if idx + 1 < self.search_results.len() {
                                            self.search_list_state.select(Some(idx + 1));
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    self.import_selected_search_game();
                                    self.tab = ActiveTab::Home;
                                }
                                _ => {}
                            },
                            ActiveTab::Runners => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('1') => self.tab = ActiveTab::Home,
                                KeyCode::Char('2') => self.tab = ActiveTab::Search,
                                KeyCode::Char('3') => self.tab = ActiveTab::Runners,
                                KeyCode::Char('4') => self.tab = ActiveTab::Plugins,
                                KeyCode::Tab => self.tab = ActiveTab::Plugins,
                                _ => {}
                            },
                            ActiveTab::Plugins => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('1') => self.tab = ActiveTab::Home,
                                KeyCode::Char('2') => self.tab = ActiveTab::Search,
                                KeyCode::Char('3') => self.tab = ActiveTab::Runners,
                                KeyCode::Char('4') => self.tab = ActiveTab::Plugins,
                                KeyCode::Tab => self.tab = ActiveTab::Home,
                                _ => {}
                            },
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, f: &mut ratatui::Frame) {
        // Main screen split (top stats, middle tab area, bottom status bar)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top Navigation Tabs
                Constraint::Min(5),    // Content Area
                Constraint::Length(3), // Status Bar
            ])
            .split(f.area());

        // Draw Navigation Tabs
        let tabs_titles = vec!["[1] Home Library", "[2] Search Sources", "[3] Runners Manager", "[4] Plugins"];
        let mut line_spans = Vec::new();
        
        for (i, title) in tabs_titles.iter().enumerate() {
            let active = match self.tab {
                ActiveTab::Home => i == 0,
                ActiveTab::Search => i == 1,
                ActiveTab::Runners => i == 2,
                ActiveTab::Plugins => i == 3,
            };
            
            if active {
                line_spans.push(Span::raw("   "));
                line_spans.push(Span::styled(*title, Style::default().fg(COLOR_PRIMARY).bold()));
                line_spans.push(Span::raw("   "));
            } else {
                line_spans.push(Span::raw("   "));
                line_spans.push(Span::styled(*title, Style::default().fg(COLOR_MUTED)));
                line_spans.push(Span::raw("   "));
            }
        }

        let nav_block = Block::default()
            .title(" GrapeVine Launcher ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_SECONDARY))
            .bg(COLOR_PANEL);

        let nav_paragraph = Paragraph::new(Line::from(line_spans))
            .block(nav_block);
        
        f.render_widget(nav_paragraph, main_chunks[0]);

        // Draw Tab Content
        match self.tab {
            ActiveTab::Home => {
                let home_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(main_chunks[1]);

                // Left: Game list
                let game_items: Vec<ListItem> = self.games
                    .iter()
                    .map(|g| {
                        let source = format!(" ({})", g.source);
                        let source_len = source.len();
                        
                        if g.status == "downloading" {
                            let prog_str = format!("Downloading {:.1}%", g.download_progress);
                            ListItem::new(Line::from(vec![
                                Span::styled(format!("  {} ", g.title), Style::default().fg(COLOR_TEXT).bold()),
                                Span::styled(source, Style::default().fg(COLOR_MUTED)),
                                Span::raw(" ".repeat(home_split[0].width.saturating_sub(g.title.len() as u16 + source_len as u16 + prog_str.len() as u16 + 12) as usize)),
                                Span::styled(prog_str, Style::default().fg(COLOR_PRIMARY)),
                            ]))
                        } else if g.status == "installing" {
                            let inst_str = "Installing...";
                            ListItem::new(Line::from(vec![
                                Span::styled(format!("  {} ", g.title), Style::default().fg(COLOR_TEXT).bold()),
                                Span::styled(source, Style::default().fg(COLOR_MUTED)),
                                Span::raw(" ".repeat(home_split[0].width.saturating_sub(g.title.len() as u16 + source_len as u16 + inst_str.len() as u16 + 12) as usize)),
                                Span::styled(inst_str, Style::default().fg(COLOR_SUCCESS)),
                            ]))
                        } else {
                            let play_time = format!("{}m", g.play_time_seconds / 60);
                            ListItem::new(Line::from(vec![
                                Span::styled(format!("  {} ", g.title), Style::default().fg(COLOR_TEXT).bold()),
                                Span::styled(source, Style::default().fg(COLOR_MUTED)),
                                Span::raw(" ".repeat(home_split[0].width.saturating_sub(g.title.len() as u16 + source_len as u16 + play_time.len() as u16 + 12) as usize)),
                                Span::styled(play_time, Style::default().fg(COLOR_HIGHLIGHT)),
                            ]))
                        }
                    })
                    .collect();

                let game_list = List::new(game_items)
                    .block(Block::default()
                        .title(" Installed Games ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_PRIMARY))
                        .bg(COLOR_PANEL))
                    .highlight_style(Style::default().bg(COLOR_MUTED).fg(COLOR_HIGHLIGHT).add_modifier(Modifier::BOLD));

                f.render_stateful_widget(game_list, home_split[0], &mut self.game_list_state);

                // Right: Game Details pane
                let details_block = Block::default()
                    .title(" Game Configuration ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(COLOR_PRIMARY))
                    .bg(COLOR_PANEL);

                if let Some(idx) = self.game_list_state.selected() {
                    if let Some(g) = self.games.get(idx) {
                        let text = if g.status == "downloading" {
                            let progress_width: usize = 30;
                            let filled = ((g.download_progress / 100.0) * progress_width as f32) as usize;
                            let empty = progress_width.saturating_sub(filled);
                            let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));
                            
                            vec![
                                Line::from(vec![Span::styled(format!("Title:        {}", g.title), Style::default().fg(COLOR_HIGHLIGHT).bold())]),
                                Line::from(vec![Span::styled(format!("Source:       {}", g.source), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("Status:       Downloading ({:.1}%)", g.download_progress), Style::default().fg(COLOR_PRIMARY).bold())]),
                                Line::from(vec![Span::styled(format!("Speed:        {}", g.download_speed), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("ETA:          {}", g.download_eta), Style::default().fg(COLOR_TEXT))]),
                                Line::from(""),
                                Line::from(vec![Span::styled(bar, Style::default().fg(COLOR_PRIMARY))]),
                                Line::from(""),
                                Line::from(vec![Span::styled("Downloading... Please wait.", Style::default().fg(COLOR_MUTED))]),
                            ]
                        } else if g.status == "installing" {
                            vec![
                                Line::from(vec![Span::styled(format!("Title:        {}", g.title), Style::default().fg(COLOR_HIGHLIGHT).bold())]),
                                Line::from(vec![Span::styled(format!("Source:       {}", g.source), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled("Status:       Installing...", Style::default().fg(COLOR_SUCCESS).bold())]),
                                Line::from(""),
                                Line::from(vec![Span::styled("[⚙️ Running silent setup installer in Wine Prefix...]", Style::default().fg(COLOR_SUCCESS))]),
                                Line::from(""),
                                Line::from(vec![Span::styled("Please wait for the installation to finish.", Style::default().fg(COLOR_MUTED))]),
                            ]
                        } else {
                            vec![
                                Line::from(vec![Span::styled(format!("Title:        {}", g.title), Style::default().fg(COLOR_HIGHLIGHT).bold())]),
                                Line::from(vec![Span::styled(format!("Source:       {}", g.source), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("Wine Path:    {}", g.runner_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Default (First Available)".to_string())), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("DXVK Overlay: {}", if g.dxvk_enabled { "Enabled" } else { "Disabled" }), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("MangoHud:     {}", if g.mangohud_enabled { "Enabled" } else { "Disabled" }), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("GameMode:     {}", if g.gamemode_enabled { "Enabled" } else { "Disabled" }), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("Last Played:  {}", g.last_played.as_deref().unwrap_or("Never")), Style::default().fg(COLOR_TEXT))]),
                                Line::from(vec![Span::styled(format!("Play Time:    {} hours {} minutes", g.play_time_seconds / 3600, (g.play_time_seconds % 3600) / 60), Style::default().fg(COLOR_SUCCESS))]),
                                Line::from(""),
                                Line::from(vec![Span::styled("Press [ENTER] to Launch Game", Style::default().fg(COLOR_SUCCESS).bold())]),
                            ]
                        };
                        let details_p = Paragraph::new(text).block(details_block).wrap(Wrap { trim: true });
                        f.render_widget(details_p, home_split[1]);
                    }
                } else {
                    let details_p = Paragraph::new("No game selected.").block(details_block);
                    f.render_widget(details_p, home_split[1]);
                }
            }
            ActiveTab::Search => {
                let search_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(5)])
                    .split(main_chunks[1]);

                // Query box
                let query_p = Paragraph::new(Line::from(vec![
                    Span::styled(" Query: ", Style::default().fg(COLOR_HIGHLIGHT)),
                    Span::styled(&self.search_query, Style::default().fg(COLOR_TEXT).bold()),
                    Span::styled("_", Style::default().fg(COLOR_PRIMARY).add_modifier(Modifier::RAPID_BLINK)),
                ]))
                .block(Block::default()
                    .title(" Search Sources ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(COLOR_PRIMARY))
                    .bg(COLOR_PANEL));
                
                f.render_widget(query_p, search_split[0]);

                // Results list
                let results_items: Vec<ListItem> = self.search_results
                    .iter()
                    .map(|g| {
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {} ", g.title), Style::default().fg(COLOR_TEXT).bold()),
                            Span::styled(format!("  ({})", g.source), Style::default().fg(COLOR_MUTED)),
                            Span::raw(" ".repeat(search_split[1].width.saturating_sub(g.title.len() as u16 + g.source.len() as u16 + 25) as usize)),
                            Span::styled("Press [ENTER] to Install", Style::default().fg(COLOR_SUCCESS)),
                        ]))
                    })
                    .collect();

                let results_list = List::new(results_items)
                    .block(Block::default()
                        .title(" Search Results ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_PRIMARY))
                        .bg(COLOR_PANEL))
                    .highlight_style(Style::default().bg(COLOR_MUTED).fg(COLOR_HIGHLIGHT).add_modifier(Modifier::BOLD));

                f.render_stateful_widget(results_list, search_split[1], &mut self.search_list_state);
            }
            ActiveTab::Runners => {
                let runner_items: Vec<ListItem> = self.orchestrator.runners
                    .iter()
                    .map(|r| {
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {} ", r.name), Style::default().fg(COLOR_TEXT).bold()),
                            Span::raw(" ".repeat(main_chunks[1].width.saturating_sub(r.name.len() as u16 + r.path.to_string_lossy().len() as u16 + 10) as usize)),
                            Span::styled(r.path.to_string_lossy().to_string(), Style::default().fg(COLOR_MUTED)),
                        ]))
                    })
                    .collect();

                let runners_list = List::new(runner_items)
                    .block(Block::default()
                        .title(" Discovered Compatibility Runners ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_PRIMARY))
                        .bg(COLOR_PANEL))
                    .highlight_style(Style::default().bg(COLOR_MUTED));

                f.render_stateful_widget(runners_list, main_chunks[1], &mut self.runner_list_state);
            }
            ActiveTab::Plugins => {
                let plugin_items: Vec<ListItem> = self.plugin_manager.plugins
                    .values()
                    .map(|p| {
                        let perms = p.permissions.join(", ");
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {} ", p.name), Style::default().fg(COLOR_TEXT).bold()),
                            Span::raw(" ".repeat(main_chunks[1].width.saturating_sub(p.name.len() as u16 + perms.len() as u16 + 22) as usize)),
                            Span::styled(format!("Permissions: [{}]", perms), Style::default().fg(COLOR_HIGHLIGHT)),
                        ]))
                    })
                    .collect();

                let plugins_list = List::new(plugin_items)
                    .block(Block::default()
                        .title(" Loaded Lua Plugins ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_PRIMARY))
                        .bg(COLOR_PANEL))
                    .highlight_style(Style::default().bg(COLOR_MUTED));

                f.render_stateful_widget(plugins_list, main_chunks[1], &mut self.plugin_list_state);
            }
        }

        // Draw Status Bar
        let status_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_SECONDARY))
            .bg(COLOR_PANEL);

        let status_spans = vec![
            Span::styled(" [TAB] Switch View ", Style::default().fg(COLOR_TEXT)),
            Span::raw(" | "),
            Span::styled(" [Ctrl+P] Command Palette ", Style::default().fg(COLOR_PRIMARY).bold()),
            Span::raw(" | "),
            Span::styled(" [Q] Quit ", Style::default().fg(COLOR_TEXT)),
        ];

        let status_paragraph = Paragraph::new(Line::from(status_spans))
            .block(status_block);
        
        f.render_widget(status_paragraph, main_chunks[2]);

        // Overlay: Now Playing Screen
        let running_game_opt = self.currently_running_game.lock().unwrap().clone();
        if let Some(game_title) = running_game_opt {
            let area = centered_rect(60, 25, f.area());
            f.render_widget(Clear, area); // clear the background of the popup

            let playing_block = Block::default()
                .title(" Playing ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD))
                .bg(COLOR_PANEL);

            let playing_text = vec![
                Line::from(""),
                Line::from(vec![Span::styled(format!("  Now Orchestrating: {}", game_title), Style::default().fg(COLOR_HIGHLIGHT).bold())]),
                Line::from(""),
                Line::from(vec![Span::styled("  GrapeVine is managing the Wine prefix container...", Style::default().fg(COLOR_TEXT))]),
                Line::from(vec![Span::styled("  Close the game process to return to the launcher.", Style::default().fg(COLOR_MUTED))]),
                Line::from(""),
            ];

            let playing_p = Paragraph::new(playing_text)
                .block(playing_block)
                .alignment(ratatui::layout::Alignment::Center);

            f.render_widget(playing_p, area);
        }

        // Overlay: Global Command Palette (Ctrl+P)
        if self.command_palette_active {
            let area = centered_rect(80, 50, f.area());
            f.render_widget(Clear, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(2)])
                .split(area);

            let input_block = Block::default()
                .title(" Command Palette (Fuzzy Search Games) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_PRIMARY))
                .bg(COLOR_PANEL);

            let input_p = Paragraph::new(Line::from(vec![
                Span::styled(" > ", Style::default().fg(COLOR_HIGHLIGHT)),
                Span::styled(&self.palette_query, Style::default().fg(COLOR_TEXT).bold()),
                Span::styled("_", Style::default().fg(COLOR_PRIMARY).add_modifier(Modifier::RAPID_BLINK)),
            ]))
            .block(input_block);
            
            f.render_widget(input_p, chunks[0]);

            let results_items: Vec<ListItem> = self.palette_results
                .iter()
                .map(|g| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("  {} ", g.title), Style::default().fg(COLOR_TEXT).bold()),
                        Span::styled(format!("  ({})", g.source), Style::default().fg(COLOR_MUTED)),
                    ]))
                })
                .collect();

            let results_list = List::new(results_items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(COLOR_PRIMARY))
                    .bg(COLOR_PANEL))
                .highlight_style(Style::default().bg(COLOR_MUTED).fg(COLOR_HIGHLIGHT).add_modifier(Modifier::BOLD));

            f.render_stateful_widget(results_list, chunks[1], &mut self.palette_list_state);
        }
    }
}

// Helper function to create a centered Rect for overlays
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
