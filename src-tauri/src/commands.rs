use crate::scanner::{self, InstalledApp, DevToolInfo, RemnantItem, PathEntry};
use crate::snapshot_engine::{self, SnapshotRecord, SnapshotDiff};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::fs;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[tauri::command]
pub fn get_installed_apps() -> Vec<InstalledApp> {
    scanner::scan_all_apps()
}

#[tauri::command]
pub fn get_dev_tools() -> Vec<DevToolInfo> {
    scanner::scan_dev_tools()
}

#[tauri::command]
pub fn clean_dev_tool_cache(name: String) -> Result<u64, String> {
    let tools = scanner::scan_dev_tools();
    let tool = tools.iter().find(|t| t.name == name)
        .ok_or_else(|| format!("Dev tool '{}' not found or not detected", name))?;

    if !tool.detected {
        return Err(format!("Dev tool '{}' is not installed on this system", name));
    }

    let initial_size = tool.cache_size.unwrap_or(0);
    if initial_size == 0 {
        return Ok(0); // Cache is already empty
    }

    // Specially handle cargo since "cargo clean" only cleans current workspace,
    // but the global cache is at %UserProfile%/.cargo/registry/cache and %UserProfile%/.cargo/git/db
    if name == "cargo" {
        if let Some(ref path_str) = tool.cache_path {
            let cargo_dir = Path::new(path_str);
            let cache_dir = cargo_dir.join("registry").join("cache");
            let git_db_dir = cargo_dir.join("git").join("db");
            
            // Delete cache files
            let mut freed = 0;
            if cache_dir.exists() {
                freed += scanner::cli_dev::calculate_dir_size(&cache_dir);
                let _ = fs::remove_dir_all(&cache_dir);
                let _ = fs::create_dir_all(&cache_dir); // Recreate empty
            }
            if git_db_dir.exists() {
                freed += scanner::cli_dev::calculate_dir_size(&git_db_dir);
                let _ = fs::remove_dir_all(&git_db_dir);
                let _ = fs::create_dir_all(&git_db_dir); // Recreate empty
            }
            return Ok(freed);
        }
    }

    // Standard CLI tools cache cleaning
    let clean_cmd = tool.clean_command.as_ref()
        .ok_or_else(|| format!("No clean command defined for '{}'", name))?;

    // On Windows, if we run npm, it's npm.cmd, yarn is yarn.cmd, etc.
    // If we execute cmd /c "command", it is safer and resolves PATH issues.
    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["/C", clean_cmd])
        .output()
        .map_err(|e| format!("Failed to execute clean command: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Clean command failed: {}", err_msg.trim()));
    }

    // Re-scan to compute the new size
    let updated_tools = scanner::scan_dev_tools();
    let updated_tool = updated_tools.iter().find(|t| t.name == name);
    let final_size = updated_tool.and_then(|t| t.cache_size).unwrap_or(0);

    let freed = if initial_size > final_size {
        initial_size - final_size
    } else {
        initial_size // If final_size calculation failed or is 0, assume we freed everything
    };

    Ok(freed)
}

#[tauri::command]
pub fn get_app_remnants(
    app_name: String,
    publisher: Option<String>,
    install_location: Option<String>,
) -> Vec<RemnantItem> {
    scanner::scan_app_remnants(&app_name, publisher.as_deref(), install_location.as_deref())
}

#[tauri::command]
pub fn purge_remnants(items: Vec<RemnantItem>) -> (u32, u32) {
    scanner::purge_all_remnants(&items)
}

#[tauri::command]
pub fn take_snapshot(name: String) -> Result<SnapshotRecord, String> {
    snapshot_engine::create_snapshot(&name)
}

#[tauri::command]
pub fn list_snapshots() -> Result<Vec<SnapshotRecord>, String> {
    snapshot_engine::list_snapshots()
}

#[tauri::command]
pub fn compare_snapshots(before_id: String, after_id: String) -> Result<SnapshotDiff, String> {
    snapshot_engine::compare_snapshots_by_id(&before_id, &after_id)
}

#[tauri::command]
pub fn get_path_entries() -> Result<Vec<PathEntry>, String> {
    scanner::get_path_entries()
}

#[tauri::command]
pub fn save_path_entries(remaining_values: Vec<String>, scope: String) -> Result<(), String> {
    scanner::set_path_entries(remaining_values, &scope)
}

#[tauri::command]
pub fn run_uninstall_command(uninstall_string: String) -> Result<(), String> {
    if uninstall_string.trim().is_empty() {
        return Err("Uninstall command is empty".to_string());
    }
    // We spawn it without CREATE_NO_WINDOW so the user can interact with the uninstaller GUI
    Command::new("cmd")
        .args(&["/C", &uninstall_string])
        .spawn()
        .map_err(|e| format!("Failed to spawn uninstaller: {}", e))?;
    Ok(())
}
