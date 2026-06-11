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

fn get_quarantine_dir() -> PathBuf {
    let base_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Local"))
                .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
        });
    let dir = base_dir.join("PurgeKit").join("Quarantine");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn copy_dir_all(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn quarantine_file_or_directory(path_str: &str) -> Result<PathBuf, String> {
    let src_path = std::path::Path::new(path_str);
    if !src_path.exists() {
        return Ok(PathBuf::new());
    }

    let q_dir = get_quarantine_dir();
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let file_uuid = uuid::Uuid::new_v4().to_string();
    let dest_name = format!("{}_{}_{}", timestamp, file_uuid, src_path.file_name().and_then(|n| n.to_str()).unwrap_or("remnant"));
    let dest_path = q_dir.join(dest_name);

    // Try renaming first (fast)
    if fs::rename(src_path, &dest_path).is_ok() {
        return Ok(dest_path);
    }

    // Fallback: Copy recursively then remove
    if src_path.is_dir() {
        copy_dir_all(src_path, &dest_path)
            .map_err(|e| format!("Failed to copy directory to quarantine: {}", e))?;
        let _ = fs::remove_dir_all(src_path);
    } else {
        fs::copy(src_path, &dest_path)
            .map_err(|e| format!("Failed to copy file to quarantine: {}", e))?;
        let _ = fs::remove_file(src_path);
    }

    Ok(dest_path)
}
