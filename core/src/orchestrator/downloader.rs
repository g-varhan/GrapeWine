use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

pub struct DownloadProgress {
    pub percentage: f32,
    pub speed: String,
    pub eta: String,
}

pub fn parse_aria2_line(line: &str) -> Option<DownloadProgress> {
    // Typical line: [#3b1a8d 12.3MiB/45.6MiB(27%) CN:3 DL:2.4MiB ETA:3m45s]
    let open_pct = line.find('(')?;
    let close_pct = line[open_pct..].find("%)")?;
    let pct_str = &line[open_pct + 1..open_pct + close_pct];
    let percentage = pct_str.parse::<f32>().ok()?;

    let dl_idx = line.find("DL:")?;
    let speed_part = &line[dl_idx + 3..];
    let speed_end = speed_part.find(' ').unwrap_or(speed_part.len());
    let speed = speed_part[..speed_end].to_string();

    let eta_idx = line.find("ETA:")?;
    let eta_part = &line[eta_idx + 4..];
    let eta_end = eta_part.find(|c| c == ' ' || c == ']' || c == '\r' || c == '\n').unwrap_or(eta_part.len());
    let eta = eta_part[..eta_end].to_string();

    Some(DownloadProgress {
        percentage,
        speed,
        eta,
    })
}

// Spawns aria2c to download the torrent magnet link and streams updates to progress_cb
pub fn download_torrent<F>(magnet_link: &str, dest_dir: &Path, progress_cb: F) -> Result<PathBuf, String>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
{
    // Ensure destination exists
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;

    // Try to find aria2c on system, fall back to ~/.local/bin/aria2c
    let bin_path = if Command::new("aria2c").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
        "aria2c".to_string()
    } else {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let local_bin = home.join(".local").join("bin").join("aria2c");
        if local_bin.exists() {
            local_bin.to_string_lossy().to_string()
        } else {
            "aria2c".to_string() // fallback
        }
    };

    let mut child = Command::new(bin_path)
        .arg("--enable-rpc=false")
        .arg("--seed-time=0")
        .arg("--bt-stop-timeout=120") // Timeout after 2 minutes of no seed/peer connection
        .arg("--dir")
        .arg(dest_dir)
        .arg(magnet_link)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn aria2c: {} (Is aria2 installed?)", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open child stdout")?;
    let reader = BufReader::new(stdout);

    for line_res in reader.lines() {
        if let Ok(line) = line_res {
            if let Some(progress) = parse_aria2_line(&line) {
                progress_cb(progress);
            }
        }
    }

    let status = child.wait().map_err(|e| format!("aria2c process failed: {}", e))?;
    if !status.success() {
        return Err(format!("aria2c exited with error code: {:?}", status.code()));
    }

    // Attempt to locate downloaded files inside the directory
    // Normally we'd return the first folder or file created
    let mut downloaded_path = dest_dir.to_path_buf();
    if let Ok(mut entries) = std::fs::read_dir(dest_dir) {
        if let Some(Ok(entry)) = entries.next() {
            downloaded_path = entry.path();
        }
    }

    Ok(downloaded_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aria2_line_parsing() {
        let line = "[#3b1a8d 12.3MiB/45.6MiB(27%) CN:3 DL:2.4MiB ETA:3m45s]";
        let progress = parse_aria2_line(line).unwrap();
        assert_eq!(progress.percentage, 27.0);
        assert_eq!(progress.speed, "2.4MiB");
        assert_eq!(progress.eta, "3m45s");
    }
}
