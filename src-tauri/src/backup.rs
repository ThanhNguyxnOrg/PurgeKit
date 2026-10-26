use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::fs;
use chrono::Local;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

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
    // Validate key_path to start with HKCU\ or HKLM\
    let key_upper = key_path.to_uppercase();
    if !key_upper.starts_with("HKCU\\") && !key_upper.starts_with("HKLM\\") {
        return Err("Invalid Registry Key path (Must start with HKCU or HKLM)".to_string());
    }

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
    
    let secure_path = crate::winutil::get_secure_system_path();
    let output = Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&["export", key_path, &backup_file.to_string_lossy(), "/y"])
        .output()
        .map_err(|e| format!("Failed to run reg export command: {}", e))?;
        
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Reg export failed: {}", err_msg.trim()));
    }
    
    Ok(backup_file)
}

pub fn get_quarantine_dir() -> PathBuf {
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

pub fn copy_dir_all(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let ty = metadata.file_type();

        // Junctions and other reparse points are not regular directories. Skip
        // them so a copy can never escape the selected source tree.
        if ty.is_symlink() || is_reparse_point(&metadata) {
            continue;
        }

        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn restore_quarantined_item(src_path: &Path, dest_path: &Path) -> Result<(), String> {
    if let Some(parent) = src_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if fs::rename(dest_path, src_path).is_ok() {
        return Ok(());
    }

    if dest_path.is_dir() {
        copy_dir_all(dest_path, src_path).map_err(|e| e.to_string())?;
        fs::remove_dir_all(dest_path).map_err(|e| e.to_string())?;
    } else {
        fs::copy(dest_path, src_path).map_err(|e| e.to_string())?;
        fs::remove_file(dest_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn quarantine_file_or_directory(path_str: &str) -> Result<PathBuf, String> {
    if let Err(e) = crate::winutil::is_safe_to_delete(path_str) {
        return Err(e);
    }

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
    let moved = if fs::rename(src_path, &dest_path).is_ok() {
        true
    } else {
        // Fallback: Copy recursively then remove
        if src_path.is_dir() {
            if copy_dir_all(src_path, &dest_path).is_ok() {
                if fs::remove_dir_all(src_path).is_ok() {
                    true
                } else {
                    let _ = fs::remove_dir_all(&dest_path);
                    false
                }
            } else {
                false
            }
        } else {
            if fs::copy(src_path, &dest_path).is_ok() {
                if fs::remove_file(src_path).is_ok() {
                    true
                } else {
                    let _ = fs::remove_file(&dest_path);
                    false
                }
            } else {
                false
            }
        }
    };

    if !moved {
        return Err("Failed to move item to quarantine".to_string());
    }

    let insert_result = match crate::db::open_connection() {
        Ok(conn) => conn.execute(
            "INSERT INTO quarantine (id, name, original_path, quarantine_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                file_uuid,
                src_path.file_name().and_then(|n| n.to_str()).unwrap_or("remnant"),
                path_str,
                dest_path.to_string_lossy().to_string(),
                timestamp
            ],
        ).map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };

    if let Err(e) = insert_result {
        // Restore the item instead of leaving an untracked moving file behind.
        let restore_result = restore_quarantined_item(src_path, &dest_path);
        return match restore_result {
            Ok(()) => Err(format!(
                "Quarantine record could not be written ({}); the item was restored to its original location.",
                e
            )),
            Err(restore_error) => Err(format!(
                "Quarantine record could not be written ({}), and restoring the item failed ({}). It remains at {}.",
                e,
                restore_error,
                dest_path.to_string_lossy()
            )),
        };
    }

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_quarantine_file() {
        let temp_dir = std::env::temp_dir().join("purgekit_test_quarantine");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let test_file = temp_dir.join("test_quarantine_file.txt");
        fs::write(&test_file, "quarantine contents").unwrap();

        // Initialize the DB first
        let _ = crate::db::init_db();

        let test_file_str = test_file.to_string_lossy().to_string();
        let res = quarantine_file_or_directory(&test_file_str);
        assert!(res.is_ok(), "Quarantine should succeed");

        let q_path = res.unwrap();
        assert!(q_path.exists(), "Quarantined file should exist at destination");
        assert!(!test_file.exists(), "Source file should have been removed");

        // Clean up
        let _ = fs::remove_file(&q_path);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_quarantine_protected_denied() {
        let res = quarantine_file_or_directory(r"C:\Windows\System32\cmd.exe");
        assert!(res.is_err(), "Quarantining protected system file should be blocked");
    }
}
