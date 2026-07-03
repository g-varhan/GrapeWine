mod parser;
mod plugins;
mod orchestrator;
mod tui;

use std::fs;
use std::path::PathBuf;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use orchestrator::{Orchestrator, GameConfig};
use plugins::PluginManager;
use tui::TuiApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orchestrator = Orchestrator::new();
    let base_dir = orchestrator.base_dir.clone();
    
    let plugins_dir = base_dir.join("plugins");
    let cache_file = base_dir.join("cache.json");
    let library_file = base_dir.join("library.json");
    
    fs::create_dir_all(&plugins_dir)?;

    // Copy out-of-the-box plugins from build repository to runtime directory if they are missing
    let workspace_plugins = PathBuf::from("plugins");
    if workspace_plugins.exists() {
        if let Ok(entries) = fs::read_dir(workspace_plugins) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "lua") {
                    let dest = plugins_dir.join(path.file_name().unwrap());
                    if !dest.exists() {
                        let _ = fs::copy(&path, &dest);
                    }
                }
            }
        }
    }

    // Load plugins
    let mut plugin_manager = PluginManager::new(&plugins_dir, &cache_file);
    if let Err(e) = plugin_manager.load_all_plugins() {
        eprintln!("Warning loading plugins: {}", e);
    }

    // Load existing library games database
    let mut games = Vec::new();
    if library_file.exists() {
        if let Ok(content) = fs::read_to_string(&library_file) {
            if let Ok(parsed) = serde_json::from_str::<Vec<GameConfig>>(&content) {
                games = parsed;
            }
        }
    } else {
        // If library is empty, insert a demo launcher configuration (e.g. minesweeper or notepad)
        // to give the user something to see and test!
        let demo_game = GameConfig {
            id: "minesweeper_demo".to_string(),
            title: "Windows Minesweeper (Demo)".to_string(),
            source: "GrapeVine".to_string(),
            exe_path: "C:\\windows\\system32\\winmine.exe".to_string(),
            launch_args: Vec::new(),
            runner_path: None,
            env_vars: std::collections::HashMap::new(),
            dxvk_enabled: false,
            mangohud_enabled: false,
            gamemode_enabled: false,
            play_time_seconds: 0,
            last_played: None,
            download_progress: 0.0,
            download_speed: String::new(),
            download_eta: String::new(),
            status: "installed".to_string(),
        };
        games.push(demo_game);
        let _ = fs::write(&library_file, serde_json::to_string_pretty(&games).unwrap());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup panic hook to restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut out = std::io::stdout();
        let _ = execute!(out, LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Run TUI app
    let mut app = TuiApp::new(orchestrator, plugin_manager, games);
    let res = app.run(&mut terminal);

    // Save plugin cache
    app.save_cache();

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error running application: {:?}", err);
    }

    Ok(())
}
