use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::fs;
use chrono::Local;

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn get_backups_dir() -> PathBuf {
    let base_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Local"))
                .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
        });
    let dir = base_dir.join("PurgeKit").join("Backups");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn backup_registry_key(key_path: &str) -> Result<PathBuf, String> {
    // Normal registry key path e.g. "HKCU\SOFTWARE\AppName"
    // Normalize or clean path for filename
    let cleaned_name = key_path
        .replace('\\', "_")
        .replace(':', "_")
        .replace('/', "_")
        .replace(" ", "_");
    
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name = format!("{}_{}.reg", timestamp, cleaned_name);
    let backup_file = get_backups_dir().join(file_name);
    
    let output = Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["export", key_path, &backup_file.to_string_lossy(), "/y"])
        .output()
        .map_err(|e| format!("Failed to run reg export command: {}", e))?;
        
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Reg export failed: {}", err_msg.trim()));
    }
    
    Ok(backup_file)
}
