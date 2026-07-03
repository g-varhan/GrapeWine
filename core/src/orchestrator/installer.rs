use std::path::{Path, PathBuf};
use std::process::Command;

// Extractor helper: extracts zip/tar.gz files
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let extension = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let status = match extension {
        "zip" => Command::new("unzip")
            .arg("-q")
            .arg(archive_path)
            .arg("-d")
            .arg(dest_dir)
            .status(),
        "gz" | "tgz" => Command::new("tar")
            .arg("-xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(dest_dir)
            .status(),
        "xz" => Command::new("tar")
            .arg("-xJf")
            .arg(archive_path)
            .arg("-C")
            .arg(dest_dir)
            .status(),
        _ => return Err(format!("Unsupported archive extension: .{}", extension)),
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("Extraction command exited with error: {:?}", s.code())),
        Err(e) => Err(format!("Failed to run extraction command: {}", e)),
    }
}

// Spawns a silent Wine installer
pub fn run_silent_installer(installer_path: &Path, prefix_dir: &Path, runner_path: &Path) -> Result<(), String> {
    let mut command = Command::new(runner_path);
    command.env("WINEPREFIX", prefix_dir.to_str().unwrap());
    
    // Pass standard silent installation flags:
    // /S = NSIS/GOG installers
    // /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- = Inno Setup
    command.arg(installer_path);
    command.arg("/VERYSILENT");
    command.arg("/SUPPRESSMSGBOXES");
    command.arg("/NORESTART");
    command.arg("/SP-");
    command.arg("/S");

    let status = command.status()
        .map_err(|e| format!("Failed to spawn Wine installer: {}", e))?;

    if !status.success() {
        return Err(format!("Wine installer exited with error: {:?}", status.code()));
    }

    Ok(())
}

// Scans prefix drive_c for game executables, returning the main game executable
pub fn smart_scan_executables(prefix_dir: &Path) -> Option<String> {
    let drive_c = prefix_dir.join("drive_c");
    if !drive_c.exists() {
        return None;
    }

    let mut candidates = Vec::new();
    find_exes(&drive_c, &mut candidates);

    // Filter out common uninstaller and redistribution patterns
    let exes: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| {
            let filename = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_lowercase();
            !filename.contains("unins") &&
            !filename.contains("uninstall") &&
            !filename.contains("uninst") &&
            !filename.contains("redist") &&
            !filename.contains("dxsetup") &&
            !filename.contains("crash") &&
            !filename.contains("cleanup") &&
            !filename.contains("touchup") &&
            !filename.contains("setup") &&
            !filename.contains("helper")
        })
        .collect();

    // Sort by file size descending
    let mut sized_exes = Vec::new();
    for path in exes {
        if let Ok(meta) = std::fs::metadata(&path) {
            sized_exes.push((path, meta.len()));
        }
    }
    sized_exes.sort_by(|a, b| b.1.cmp(&a.1));

    let main_exe_path = &sized_exes.first()?.0;

    // Convert Unix path to Wine internal format (e.g. C:\Program Files\Game\game.exe)
    if let Ok(relative) = main_exe_path.strip_prefix(&drive_c) {
        let windows_path = relative.to_string_lossy().replace("/", "\\");
        return Some(format!("C:\\{}", windows_path));
    }

    None
}

fn find_exes(dir: &Path, candidates: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_exes(&path, candidates);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("exe") {
                        candidates.push(path);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_smart_exe_scanner() {
        let temp_dir = std::env::temp_dir().join("grapevine_test_prefix");
        let drive_c = temp_dir.join("drive_c");
        let prog_files = drive_c.join("Program Files").join("MyGame");
        let _ = fs::create_dir_all(&prog_files);

        // Create mock executables
        let game_exe = prog_files.join("game.exe");
        let unins_exe = prog_files.join("unins000.exe");
        let helper_exe = prog_files.join("crash_helper.exe");

        let mut f1 = File::create(&game_exe).unwrap();
        std::io::Write::write_all(&mut f1, &[0; 5000]).unwrap(); // Largest (Game)

        let mut f2 = File::create(&unins_exe).unwrap();
        std::io::Write::write_all(&mut f2, &[0; 1000]).unwrap(); // Small uninstaller

        let mut f3 = File::create(&helper_exe).unwrap();
        std::io::Write::write_all(&mut f3, &[0; 6000]).unwrap(); // Larger but matches filter "helper"

        let detected = smart_scan_executables(&temp_dir).unwrap();
        assert!(detected.contains("game.exe"));
        assert!(!detected.contains("unins"));
        assert!(!detected.contains("helper"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
