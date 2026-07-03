use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::fs;
use serde::{Serialize, Deserialize};

pub mod downloader;
pub mod installer;

// Embedded Windows Guest Helper binary
const GUEST_HELPER_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grapevine-helper.exe"));

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Runner {
    pub name: String,
    pub path: PathBuf, // Path to wine / proton executable
    pub is_proton: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GameConfig {
    pub id: String,
    pub title: String,
    pub source: String,
    pub exe_path: String,
    pub launch_args: Vec<String>,
    pub runner_path: Option<PathBuf>,
    pub env_vars: HashMap<String, String>,
    pub dxvk_enabled: bool,
    pub mangohud_enabled: bool,
    pub gamemode_enabled: bool,
    pub play_time_seconds: u64,
    pub last_played: Option<String>,
    
    #[serde(default)]
    pub download_progress: f32,
    #[serde(default)]
    pub download_speed: String,
    #[serde(default)]
    pub download_eta: String,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "installed".to_string()
}

#[derive(Deserialize, Debug)]
struct StatusReport {
    status: String,
    active_processes: u32,
    elapsed_seconds: u64,
}

pub struct Orchestrator {
    pub base_dir: PathBuf,
    pub runners: Vec<Runner>,
}

impl Orchestrator {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base_dir = home.join(".local").join("share").join("grapevine");
        let _ = fs::create_dir_all(&base_dir);

        let mut orchestrator = Self {
            base_dir,
            runners: Vec::new(),
        };
        orchestrator.detect_runners();
        orchestrator
    }

    pub fn get_prefix_dir(&self, game_id: &str) -> PathBuf {
        self.base_dir.join("prefixes").join(game_id)
    }

    pub fn get_helper_dest_path(&self, game_id: &str) -> PathBuf {
        self.get_prefix_dir(game_id).join("drive_c").join("grapevine-helper.exe")
    }

    pub fn get_status_file_path(&self, game_id: &str) -> PathBuf {
        self.get_prefix_dir(game_id).join("drive_c").join("grapevine-status.json")
    }

    pub fn save_game_config_to_db(&self, config: &GameConfig) -> Result<(), String> {
        let db_path = self.base_dir.join("library.json");
        if let Ok(content) = fs::read_to_string(&db_path) {
            if let Ok(mut current_games) = serde_json::from_str::<Vec<GameConfig>>(&content) {
                for g in &mut current_games {
                    if g.id == config.id {
                        *g = config.clone();
                    }
                }
                let json = serde_json::to_string_pretty(&current_games)
                    .map_err(|e| e.to_string())?;
                fs::write(db_path, json).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    // Automatically scans standard paths for Steam Proton, GE-Proton, Bottles, or system Wine
    pub fn detect_runners(&mut self) {
        self.runners.clear();

        // 1. Check system wine
        if let Ok(output) = Command::new("which").arg("wine").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                self.runners.push(Runner {
                    name: "System Wine".to_string(),
                    path: PathBuf::from(path_str),
                    is_proton: false,
                });
            }
        }

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Paths where runners can be found
        let runner_dirs = vec![
            home.join(".local").join("share").join("Steam").join("compatibilitytools.d"),
            home.join(".steam").join("root").join("compatibilitytools.d"),
            home.join(".local").join("share").join("bottles").join("runners"),
            PathBuf::from("/usr/share/steam/compatibilitytools.d"),
        ];

        for dir in runner_dirs {
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let bin_path_wine = path.join("files").join("bin").join("wine");
                            let bin_path_proton = path.join("files").join("bin").join("proton");
                            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();

                            if bin_path_wine.exists() {
                                self.runners.push(Runner {
                                    name: format!("GE-Proton/Wine-GE ({})", name),
                                    path: bin_path_wine,
                                    is_proton: false,
                                });
                            } else if bin_path_proton.exists() {
                                self.runners.push(Runner {
                                    name: format!("Steam Proton ({})", name),
                                    path: bin_path_proton,
                                    is_proton: true,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Prepares the Wine prefix and copies grapevine-helper.exe
    pub fn prepare_prefix(&self, game_id: &str) -> Result<(), String> {
        let prefix_dir = self.get_prefix_dir(game_id);
        let drive_c = prefix_dir.join("drive_c");
        
        fs::create_dir_all(&drive_c)
            .map_err(|e| format!("Failed to create drive_c directory: {}", e))?;

        // Extract grapevine-helper.exe
        let helper_path = self.get_helper_dest_path(game_id);
        fs::write(&helper_path, GUEST_HELPER_BYTES)
            .map_err(|e| format!("Failed to extract grapevine-helper.exe: {}", e))?;

        // Remove old status file if exists
        let status_path = self.get_status_file_path(game_id);
        if status_path.exists() {
            let _ = fs::remove_file(status_path);
        }

        Ok(())
    }

    // Installs DXVK library overrides into the prefix
    pub fn setup_dxvk(&self, _game_id: &str) -> Result<(), String> {
        Ok(())
    }

    // Launches the game in the prefix using grapevine-helper.exe
    // Updates the game's play_time_seconds dynamically.
    pub fn launch_game(&self, config: &mut GameConfig) -> Result<(), String> {
        let game_id = &config.id;

        // 1. Download and silent install loop if exe_path is a magnet link
        if config.exe_path.starts_with("magnet:") {
            config.status = "downloading".to_string();
            config.download_progress = 0.0;
            config.download_speed = "0.0 B/s".to_string();
            config.download_eta = "Unknown".to_string();
            let _ = self.save_game_config_to_db(config);

            let magnet = config.exe_path.clone();
            let prefix_dir = self.get_prefix_dir(game_id);
            let download_dir = prefix_dir.join("drive_c").join("download");

            let orchestrator_base = self.base_dir.clone();
            let game_id_clone = game_id.clone();

            let download_res = downloader::download_torrent(&magnet, &download_dir, move |progress| {
                let db_path = orchestrator_base.join("library.json");
                if let Ok(content) = fs::read_to_string(&db_path) {
                    if let Ok(mut current_games) = serde_json::from_str::<Vec<GameConfig>>(&content) {
                        for g in &mut current_games {
                            if g.id == game_id_clone {
                                g.status = "downloading".to_string();
                                g.download_progress = progress.percentage;
                                g.download_speed = progress.speed.clone();
                                g.download_eta = progress.eta.clone();
                            }
                        }
                        let _ = fs::write(&db_path, serde_json::to_string_pretty(&current_games).unwrap());
                    }
                }
            });

            let downloaded_folder = match download_res {
                Ok(path) => path,
                Err(e) => {
                    config.status = "installed".to_string();
                    let _ = self.save_game_config_to_db(config);
                    return Err(format!("Download failed: {}", e));
                }
            };

            // 2. Install Phase
            config.status = "installing".to_string();
            let _ = self.save_game_config_to_db(config);

            let drive_c = prefix_dir.join("drive_c");
            let mut installed_successfully = false;

            // Handle direct archives or setup files
            if downloaded_folder.is_file() {
                let ext = downloaded_folder.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "zip" || ext == "gz" || ext == "tgz" || ext == "xz" {
                    if let Err(e) = installer::extract_archive(&downloaded_folder, &drive_c.join("game")) {
                        config.status = "installed".to_string();
                        let _ = self.save_game_config_to_db(config);
                        return Err(format!("Archive extraction failed: {}", e));
                    }
                    installed_successfully = true;
                }
            } else if downloaded_folder.is_dir() {
                let mut setup_exe = None;
                let mut archive_file = None;

                if let Ok(entries) = fs::read_dir(&downloaded_folder) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let filename = path.file_name().unwrap().to_string_lossy().to_lowercase();
                            if filename.contains("setup") || filename.contains("install") {
                                setup_exe = Some(path);
                                break;
                            }
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                            if ext == "zip" || ext == "gz" || ext == "tgz" || ext == "xz" {
                                archive_file = Some(path);
                            }
                        }
                    }
                }

                if let Some(exe) = setup_exe {
                    let runner = self.runners.first().map(|r| r.path.clone()).ok_or("No Wine runner available for installation")?;
                    if let Err(e) = installer::run_silent_installer(&exe, &prefix_dir, &runner) {
                        config.status = "installed".to_string();
                        let _ = self.save_game_config_to_db(config);
                        return Err(format!("Silent setup installer failed: {}", e));
                    }
                    installed_successfully = true;
                } else if let Some(archive) = archive_file {
                    if let Err(e) = installer::extract_archive(&archive, &drive_c.join("game")) {
                        config.status = "installed".to_string();
                        let _ = self.save_game_config_to_db(config);
                        return Err(format!("Archive extraction failed: {}", e));
                    }
                    installed_successfully = true;
                }
            }

            if !installed_successfully {
                // Fallback: Copy downloaded folder structure directly as target game
                let dest_game_dir = drive_c.join("game");
                let _ = fs::remove_dir_all(&dest_game_dir);
                let _ = fs::create_dir_all(&dest_game_dir);
                
                // Copy directories manually or run system move
                let _ = Command::new("cp")
                    .arg("-r")
                    .arg(format!("{}/.", downloaded_folder.to_str().unwrap()))
                    .arg(&dest_game_dir)
                    .status();
            }

            // Cleanup temp download dir
            let _ = fs::remove_dir_all(download_dir);

            // 3. Smart scan for executable
            if let Some(detected_exe) = installer::smart_scan_executables(&prefix_dir) {
                config.exe_path = detected_exe;
                config.status = "installed".to_string();
                let _ = self.save_game_config_to_db(config);
            } else {
                config.status = "installed".to_string();
                let _ = self.save_game_config_to_db(config);
                return Err("Failed to detect executable inside Wine prefix".to_string());
            }
        }

        self.prepare_prefix(game_id)?;

        // Choose runner
        let runner_path = config.runner_path.clone().or_else(|| {
            self.runners.first().map(|r| r.path.clone())
        }).ok_or("No Wine/Proton runner selected or available on system")?;

        let prefix_dir = self.get_prefix_dir(game_id);
        let _helper_path = self.get_helper_dest_path(game_id);
        
        // Formulate target exe arguments for guest helper
        let mut command = Command::new(&runner_path);

        // Env setup
        command.env("WINEPREFIX", prefix_dir.to_str().unwrap());
        command.env("WINEESYNC", "1");
        command.env("WINEFSYNC", "1");

        // Apply custom variables
        for (k, v) in &config.env_vars {
            command.env(k, v);
        }

        // DXVK DLL Overrides
        if config.dxvk_enabled {
            command.env("WINEDLLOVERRIDES", "d3d11,dxgi=n,b");
        }

        // Build args: wine C:\grapevine-helper.exe C:\game.exe [args]
        command.arg("C:\\grapevine-helper.exe");
        command.arg(&config.exe_path);
        
        for arg in &config.launch_args {
            command.arg(arg);
        }

        // Start process
        let mut child = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Wine process: {}", e))?;

        // Monitor status file
        let status_path = self.get_status_file_path(game_id);
        let mut last_elapsed = 0u64;

        loop {
            if let Ok(Some(_status)) = child.try_wait() {
                break;
            }

            if status_path.exists() {
                if let Ok(content) = fs::read_to_string(&status_path) {
                    if let Ok(report) = serde_json::from_str::<StatusReport>(&content) {
                        let diff = report.elapsed_seconds.saturating_sub(last_elapsed);
                        config.play_time_seconds += diff;
                        last_elapsed = report.elapsed_seconds;

                        if report.status == "finished" {
                            break;
                        }
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Final cleanup & capture remaining play time
        if status_path.exists() {
            if let Ok(content) = fs::read_to_string(&status_path) {
                if let Ok(report) = serde_json::from_str::<StatusReport>(&content) {
                    let diff = report.elapsed_seconds.saturating_sub(last_elapsed);
                    config.play_time_seconds += diff;
                }
            }
            let _ = fs::remove_file(status_path);
        }

        // Set last played timestamp
        config.last_played = Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());

        Ok(())
    }
}
