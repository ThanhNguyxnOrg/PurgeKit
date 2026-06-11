use crate::scanner::{self, InstalledApp, DevToolInfo, RemnantItem, PathEntry, GlobalCliPackage};
use crate::snapshot_engine::{self, SnapshotRecord, SnapshotDiff};
use crate::settings::{self, AppSettings};
use serde::Serialize;
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::fs;
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Clone)]
pub struct ProgressPayload {
    pub phase: String,
    pub current: u32,
    pub total: u32,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PurgeResult {
    pub success: u32,
    pub fail: u32,
}

#[tauri::command]
pub async fn get_installed_apps(app: AppHandle) -> Result<Vec<InstalledApp>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("scan-progress", ProgressPayload {
            phase: "registry".into(),
            current: 10,
            total: 100,
            message: "Scanning registry hives...".into(),
        });
        
        let mut apps = scanner::scan_registry();
        
        let _ = app.emit("scan-progress", ProgressPayload {
            phase: "uwp".into(),
            current: 60,
            total: 100,
            message: "Scanning Windows Store apps...".into(),
        });
        
        let uwp_apps = scanner::scan_uwp_apps();
        apps.extend(uwp_apps);
        
        let _ = app.emit("scan-progress", ProgressPayload {
            phase: "sorting".into(),
            current: 90,
            total: 100,
            message: "Sorting applications...".into(),
        });
        
        apps.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        
        let _ = app.emit("scan-progress", ProgressPayload {
            phase: "completed".into(),
            current: 100,
            total: 100,
            message: "Scan completed".into(),
        });
        
        Ok(apps)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_dev_tools(app: AppHandle) -> Result<Vec<DevToolInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("dev-scan-progress", ProgressPayload {
            phase: "scanning".into(),
            current: 20,
            total: 100,
            message: "Analyzing dev tool paths...".into(),
        });
        
        let tools = scanner::scan_dev_tools();
        
        let _ = app.emit("dev-scan-progress", ProgressPayload {
            phase: "completed".into(),
            current: 100,
            total: 100,
            message: "Dev tools scan completed".into(),
        });
        
        Ok(tools)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clean_dev_tool_cache(app: AppHandle, name: String) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("clean-progress", ProgressPayload {
            phase: "scanning".into(),
            current: 10,
            total: 100,
            message: format!("Checking dev tool '{}' cache...", name).into(),
        });
        
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

        let _ = app.emit("clean-progress", ProgressPayload {
            phase: "cleaning".into(),
            current: 40,
            total: 100,
            message: format!("Purging cache for '{}'...", name).into(),
        });

        // Specially handle cargo
        if name == "cargo" {
            if let Some(ref path_str) = tool.cache_path {
                let cargo_dir = Path::new(path_str);
                let cache_dir = cargo_dir.join("registry").join("cache");
                let git_db_dir = cargo_dir.join("git").join("db");
                
                let mut freed = 0;
                if cache_dir.exists() {
                    freed += scanner::cli_dev::calculate_dir_size(&cache_dir);
                    let _ = fs::remove_dir_all(&cache_dir);
                    let _ = fs::create_dir_all(&cache_dir);
                }
                if git_db_dir.exists() {
                    freed += scanner::cli_dev::calculate_dir_size(&git_db_dir);
                    let _ = fs::remove_dir_all(&git_db_dir);
                    let _ = fs::create_dir_all(&git_db_dir);
                }
                return Ok(freed);
            }
        }

        // Specially handle gradle
        if name == "gradle" {
            if let Some(ref path_str) = tool.cache_path {
                let cache_dir = Path::new(path_str);
                let mut freed = 0;
                if cache_dir.exists() {
                    freed += scanner::cli_dev::calculate_dir_size(&cache_dir);
                    let _ = fs::remove_dir_all(&cache_dir);
                    let _ = fs::create_dir_all(&cache_dir);
                }
                return Ok(freed);
            }
        }

        // Specially handle maven
        if name == "maven" {
            if let Some(ref path_str) = tool.cache_path {
                let cache_dir = Path::new(path_str);
                let mut freed = 0;
                if cache_dir.exists() {
                    freed += scanner::cli_dev::calculate_dir_size(&cache_dir);
                    let _ = fs::remove_dir_all(&cache_dir);
                    let _ = fs::create_dir_all(&cache_dir);
                }
                return Ok(freed);
            }
        }


        // Standard CLI tools cache cleaning
        let clean_cmd = tool.clean_command.as_ref()
            .ok_or_else(|| format!("No clean command defined for '{}'", name))?;

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
            initial_size
        };

        let _ = app.emit("clean-progress", ProgressPayload {
            phase: "completed".into(),
            current: 100,
            total: 100,
            message: format!("Successfully cleaned '{}' cache!", name).into(),
        });

        Ok(freed)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_app_remnants(
    app: AppHandle,
    app_name: String,
    publisher: Option<String>,
    install_location: Option<String>,
) -> Result<Vec<RemnantItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("remnant-scan-progress", ProgressPayload {
            phase: "scanning".into(),
            current: 20,
            total: 100,
            message: format!("Searching residual paths for '{}'...", app_name).into(),
        });
        
        let remnants = scanner::scan_app_remnants(&app_name, publisher.as_deref(), install_location.as_deref());
        
        let _ = app.emit("remnant-scan-progress", ProgressPayload {
            phase: "completed".into(),
            current: 100,
            total: 100,
            message: "Leftovers scan completed".into(),
        });
        
        Ok(remnants)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn purge_remnants(app: AppHandle, items: Vec<RemnantItem>) -> Result<PurgeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let total = items.len() as u32;
        let mut success = 0;
        let mut fail = 0;

        let settings = settings::load_settings();

        // 1. Optional backup phase
        if settings.backup_before_delete {
            let _ = app.emit("purge-progress", ProgressPayload {
                phase: "backup".into(),
                current: 0,
                total,
                message: "Backing up registry keys before purge...".into(),
            });
            for item in &items {
                if item.item_type == "RegistryKey" {
                    // Try to backup. Ignore error so we still try to delete it
                    let _ = crate::backup::backup_registry_key(&item.path);
                }
            }
        }

        // 2. Deletion phase
        for (i, item) in items.iter().enumerate() {
            let _ = app.emit("purge-progress", ProgressPayload {
                phase: "deleting".into(),
                current: i as u32,
                total,
                message: format!("Purging: {}", item.path),
            });

            match scanner::remnants::purge_remnant_item(item) {
                Ok(_) => success += 1,
                Err(_) => fail += 1,
            }
        }

        // 3. Broadcast changes
        let _ = app.emit("purge-progress", ProgressPayload {
            phase: "system_notify".into(),
            current: total,
            total,
            message: "Notifying Windows shell and environment changes...".into(),
        });
        
        crate::broadcast::broadcast_all_system_changes();

        let _ = app.emit("purge-progress", ProgressPayload {
            phase: "completed".into(),
            current: total,
            total,
            message: "Purging completed successfully".into(),
        });

        Ok(PurgeResult { success, fail })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn take_snapshot(app: AppHandle, name: String) -> Result<SnapshotRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("snapshot-progress", ProgressPayload {
            phase: "scanning".into(),
            current: 20,
            total: 100,
            message: "Creating system snapshot (scanning registry & files)...".into(),
        });
        
        let record = snapshot_engine::create_snapshot(&name)?;
        
        let _ = app.emit("snapshot-progress", ProgressPayload {
            phase: "completed".into(),
            current: 100,
            total: 100,
            message: "Snapshot created successfully".into(),
        });
        
        Ok(record)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_snapshots() -> Result<Vec<SnapshotRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        snapshot_engine::list_snapshots()
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_snapshot(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        snapshot_engine::delete_snapshot_by_id(&id)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn compare_snapshots(before_id: String, after_id: String) -> Result<SnapshotDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        snapshot_engine::compare_snapshots_by_id(&before_id, &after_id)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_path_entries() -> Result<Vec<PathEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::get_path_entries()
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_path_entries(remaining_values: Vec<String>, scope: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::set_path_entries(remaining_values, &scope)?;
        crate::broadcast::broadcast_environment_change();
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn run_uninstall_command(uninstall_string: String) -> Result<(), String> {
    if uninstall_string.trim().is_empty() {
        return Err("Uninstall command is empty".to_string());
    }
    
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = Command::new("cmd")
            .args(&["/C", &uninstall_string])
            .spawn()
            .map_err(|e| format!("Failed to spawn uninstaller: {}", e))?;
            
        let _ = child.wait().map_err(|e| format!("Uninstaller finished with error: {}", e))?;
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_global_npm_packages() -> Result<Vec<GlobalCliPackage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::scan_global_npm_packages())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_global_npm_package(name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::uninstall_global_npm_package(&name)
    }).await.map_err(|e| e.to_string())?
}

// Settings Commands
#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    Ok(settings::load_settings())
}

#[tauri::command]
pub async fn save_settings(settings: AppSettings) -> Result<(), String> {
    settings::save_settings(&settings)
}

#[tauri::command]
pub async fn check_is_admin() -> Result<bool, String> {
    Ok(settings::check_is_admin())
}

#[tauri::command]
pub async fn start_install_tracking(
    state: tauri::State<'_, crate::tracker::ActiveTracking>,
    name: String,
) -> Result<(), String> {
    let state_clone = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let current_usn = unsafe { crate::tracker::query_current_usn('C') }?;
        let start_time = std::time::SystemTime::now();
        
        // Take registry baseline
        let mut reg_baseline = Vec::new();
        let _ = crate::snapshot_engine::scan_registry_keys_recursive(
            winreg::enums::HKEY_CURRENT_USER, "HKCU", "SOFTWARE", 0, 5, &mut reg_baseline
        );
        let _ = crate::snapshot_engine::scan_registry_keys_recursive(
            winreg::enums::HKEY_LOCAL_MACHINE, "HKLM", "SOFTWARE", 0, 5, &mut reg_baseline
        );

        let mut active = state_clone.lock().map_err(|e| e.to_string())?;
        *active = Some(crate::tracker::TrackingSession {
            start_usn: current_usn,
            name,
            start_time,
            reg_baseline,
        });
        
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn stop_install_tracking(
    state: tauri::State<'_, crate::tracker::ActiveTracking>,
    _app: AppHandle,
) -> Result<SnapshotRecord, String> {
    let state_clone = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = {
            let mut active = state_clone.lock().map_err(|e| e.to_string())?;
            active.take().ok_or_else(|| "No active installation tracking session is running".to_string())?
        };

        let _current_usn = unsafe { crate::tracker::query_current_usn('C') }?;
        let usn_changes = unsafe { crate::tracker::read_usn_changes('C', session.start_usn) }?;
        
        // Scan registry after installation
        let mut current_reg = Vec::new();
        let _ = crate::snapshot_engine::scan_registry_keys_recursive(
            winreg::enums::HKEY_CURRENT_USER, "HKCU", "SOFTWARE", 0, 5, &mut current_reg
        );
        let _ = crate::snapshot_engine::scan_registry_keys_recursive(
            winreg::enums::HKEY_LOCAL_MACHINE, "HKLM", "SOFTWARE", 0, 5, &mut current_reg
        );

        // Diff registry keys
        use std::collections::HashSet;
        let baseline_reg_set: HashSet<&String> = session.reg_baseline.iter().collect();
        let mut new_registry_keys = Vec::new();
        for key in &current_reg {
            if !baseline_reg_set.contains(key) {
                new_registry_keys.push(key.clone());
            }
        }

        // Deduplicate and filter USN changes to get new file/folder paths.
        let mut new_files = Vec::new();
        let dirs = vec![
            std::env::var_os("APPDATA").map(std::path::PathBuf::from),
            std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from),
            std::env::var_os("ProgramData").map(std::path::PathBuf::from),
        ];

        // Convert usn_changes to a HashSet for O(1) filename matching
        let usn_set: HashSet<String> = usn_changes.into_iter().map(|s| s.to_lowercase()).collect();
        
        for dir in dirs.into_iter().flatten() {
            if !dir.exists() { continue; }
            for entry in walkdir::WalkDir::new(&dir)
                .max_depth(4)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if let Ok(metadata) = path.metadata() {
                    let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    
                    // Filter: modified after tracking start time OR filename is in USN journal changes
                    let is_newer = modified >= session.start_time;
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    
                    if is_newer || usn_set.contains(&filename) {
                        new_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // De-duplicate lists
        new_registry_keys.sort();
        new_registry_keys.dedup();
        new_files.sort();
        new_files.dedup();

        // Create SnapshotRecord and SnapshotData files
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        let snap_data = crate::snapshot_engine::SnapshotData {
            registry_keys: new_registry_keys.clone(),
            files: new_files.clone(),
        };

        let snap_dir = crate::db::get_snapshots_dir();
        let file_name = format!("snap_{}.json", id);
        let data_file_path = snap_dir.join(file_name);
        
        let json_data = serde_json::to_string(&snap_data).map_err(|e| e.to_string())?;
        std::fs::write(&data_file_path, json_data).map_err(|e| e.to_string())?;

        let db_path = crate::db::get_db_path();
        let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        
        let display_name = format!("[Tracked] {}", session.name);
        
        conn.execute(
            "INSERT INTO snapshots (id, name, created_at, data_file_path) VALUES (?1, ?2, ?3, ?4)",
            [&id, &display_name, &created_at, &data_file_path.to_string_lossy().to_string()],
        ).map_err(|e| e.to_string())?;

        Ok(SnapshotRecord {
            id,
            name: display_name,
            created_at,
            data_file_path: data_file_path.to_string_lossy().to_string(),
            reg_count: new_registry_keys.len(),
            file_count: new_files.len(),
        })
    }).await.map_err(|e| e.to_string())?
}
