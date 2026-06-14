use crate::scanner::{self, InstalledApp, DevToolInfo, RemnantItem, PathEntry, GlobalCliPackage, ProjectFolder, WslDistroInfo, ToolchainVersion};
use crate::snapshot_engine::{self, SnapshotRecord, SnapshotDiff};
use crate::settings::{self, AppSettings};
use serde::Serialize;
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::fs;
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x08000000;

const SAFE_DEV_TOOL_CLEAN_COMMANDS: &[&str] = &[
    "npm cache clean --force",
    "pnpm store prune",
    "yarn cache clean",
    "pip cache purge",
    "cargo clean",
    "go clean -cache -modcache",
    "bun pm clean",
    "deno clean",
    "gradle clean",
    "mvn clean",
    "dotnet nuget locals all --clear",
    "docker system prune -f",
    "conda clean -a -y",
    "gem cleanup",
    "flutter pub cache clean --force",
];

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

        // Load rules to find resolved cache paths
        let rules = scanner::cli_dev::load_dev_tools_rules();
        let rule = rules.iter().find(|r| r.name == name)
            .ok_or_else(|| format!("Rule configuration not found for '{}'", name))?;

        let mut resolved_paths = Vec::new();
        if let Some(ref dyn_cmd) = rule.dynamic_cache_cmd {
            if let Some(dyn_path) = scanner::cli_dev::get_single_dynamic_cache_path(&name, dyn_cmd) {
                resolved_paths.push(dyn_path);
            }
        }
        if resolved_paths.is_empty() {
            if let Some(ref templates) = rule.cache_path_templates {
                for temp in templates {
                    if let Some(p) = scanner::cli_dev::resolve_template_path(temp) {
                        if p.exists() {
                            resolved_paths.push(p);
                        }
                    }
                }
            }
        }

        let mut cleaned_via_cmd = false;

        if let Some(ref clean_cmd) = rule.clean_command {
            if SAFE_DEV_TOOL_CLEAN_COMMANDS.contains(&clean_cmd.as_str()) {
                let secure_path = crate::winutil::get_secure_system_path();
                let output = Command::new("cmd")
                    .creation_flags(CREATE_NO_WINDOW)
                    .env("PATH", &secure_path)
                    .args(&["/C", clean_cmd])
                    .output()
                    .map_err(|e| format!("Failed to execute clean command: {}", e))?;

                if !output.status.success() {
                    let err_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("Clean command failed: {}", err_msg.trim()));
                }
                cleaned_via_cmd = true;
            }
        }

        // If not cleaned via whitelisted command, execute safe directory purging
        if !cleaned_via_cmd {
            for p in &resolved_paths {
                if p.exists() {
                    crate::winutil::is_safe_to_delete(&p.to_string_lossy()).map_err(|e| e.to_string())?;
                    let _ = fs::remove_dir_all(p);
                    let _ = fs::create_dir_all(p);
                }
            }
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

        // Admin elevation pre-check for protected items
        if !settings::check_is_admin() {
            let needs_admin = items.iter().any(|item| remnant_requires_admin(item));
            if needs_admin {
                return Err("Some selected remnant items (such as HKLM registry keys or system files) require Administrator privileges to delete. Please run the application as Administrator.".to_string());
            }
        }

        // 0. Optional System Restore Point phase
        if settings.create_restore_point {
            create_system_restore_point(&app);
        }

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
        if scope == "System" && !settings::check_is_admin() {
            return Err("Modifying System PATH requires Administrator privileges. Please run the application as Administrator.".to_string());
        }
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
    
    // Check if it is a UWP uninstall command
    let is_uwp = uninstall_string.starts_with("powershell -NoProfile -Command \"Remove-AppxPackage -Package ") 
        && uninstall_string.ends_with('"');
        
    let secure_path = crate::winutil::get_secure_system_path();

    if is_uwp {
        let prefix = "powershell -NoProfile -Command \"Remove-AppxPackage -Package ";
        let package_full = uninstall_string
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(|| "Malformed UWP uninstall string".to_string())?
            .to_string();
        if !package_full.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return Err("Invalid UWP package name or contains unsafe characters".to_string());
        }
        
        let path_clone = secure_path.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let mut child = Command::new("powershell")
                .creation_flags(CREATE_NO_WINDOW)
                .env("PATH", &path_clone)
                .args(&["-NoProfile", "-Command", &format!("Remove-AppxPackage -Package {}", package_full)])
                .spawn()
                .map_err(|e| format!("Failed to spawn powershell: {}", e))?;
            let _ = child.wait().map_err(|e| format!("PowerShell finished with error: {}", e))?;
            Ok(())
        }).await.map_err(|e| e.to_string())?;
    }
    
    if !is_safe_uninstall_string(&uninstall_string) {
        return Err("Uninstall command contains invalid or unsafe characters (Command Injection Blocked)".to_string());
    }
    
    if let Err(e) = validate_uninstall_command_safety(&uninstall_string) {
        return Err(e);
    }
    
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = Command::new("cmd")
            .env("PATH", &secure_path)
            .args(&["/C", &uninstall_string])
            .spawn()
            .map_err(|e| format!("Failed to spawn uninstaller: {}", e))?;
            
        let _ = child.wait().map_err(|e| format!("Uninstaller finished with error: {}", e))?;
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_global_cli_packages() -> Result<Vec<GlobalCliPackage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::scan_global_cli_packages())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_global_cli_package(name: String, manager: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::uninstall_global_cli_package(&name, &manager)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_cli_package_bin_names(package_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::get_cli_package_bin_names(&package_path))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_cli_package_remnants(
    name: String,
    manager: String,
    package_path: String,
    bin_names: Vec<String>,
) -> Result<Vec<RemnantItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::get_cli_package_remnants(&name, &manager, &package_path, bin_names))
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

fn is_safe_uninstall_string(s: &str) -> bool {
    let dangerous_chars = ['&', '|', '>', '<', ';', '`', '$', '\n', '\r'];
    !s.chars().any(|c| dangerous_chars.contains(&c))
}

fn validate_uninstall_command_safety(cmd_str: &str) -> Result<(), String> {
    let exe_path = crate::winutil::extract_executable_path(cmd_str);
    let exe_lower = exe_path.to_lowercase();
    
    let is_shell = exe_lower.ends_with("cmd.exe") || exe_lower == "cmd" 
        || exe_lower.ends_with("powershell.exe") || exe_lower == "powershell"
        || exe_lower.ends_with("pwsh.exe") || exe_lower == "pwsh";

    if is_shell {
        let cmd_lower = cmd_str.to_lowercase();
        if cmd_lower.contains("users\\") || cmd_lower.contains("appdata") || cmd_lower.contains("temp\\") {
            return Err("Execution blocked: Uninstaller command uses a shell wrapper targeting user-writable folders. This could lead to privilege escalation.".to_string());
        }
    }

    let user_profile = std::env::var("USERPROFILE").unwrap_or_default().to_lowercase();
    if !user_profile.is_empty() && exe_lower.starts_with(&user_profile) {
        let path = std::path::Path::new(&exe_path);
        if !path.exists() {
            return Err(format!("Uninstaller executable does not exist: {}", exe_path));
        }
        
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "exe" {
            return Err(format!("Execution blocked: Uninstaller uses a non-executable script/file ({}) from a user-writable folder.", ext));
        }

        if is_elevated::is_elevated() && !crate::winutil::verify_file_signature(&exe_path) {
            return Err(format!(
                "Security Block: The uninstaller executable '{}' resides in a user-writable folder and is UNSIGNED. Running this elevated poses a Local Privilege Escalation risk.",
                exe_path
            ));
        }
    }

    Ok(())
}

fn remnant_requires_admin(item: &RemnantItem) -> bool {
    if item.item_type == "RegistryKey" || item.item_type == "RegistryValue" {
        return item.path.to_uppercase().starts_with("HKLM");
    }
    if item.item_type == "File" || item.item_type == "Directory" {
        let path_upper = item.path.to_uppercase();
        return path_upper.contains("PROGRAM FILES") 
            || path_upper.contains("WINDOWS") 
            || path_upper.starts_with("C:\\PROGRAMDATA");
    }
    false
}

fn create_system_restore_point(app: &AppHandle) {
    let _ = app.emit("purge-progress", ProgressPayload {
        phase: "restore_point".into(),
        current: 0,
        total: 100,
        message: "Creating System Restore Point...".into(),
    });

    if !settings::check_is_admin() {
        let _ = app.emit("purge-progress", ProgressPayload {
            phase: "restore_point_warning".into(),
            current: 0,
            total: 100,
            message: "Skipping System Restore Point creation (Administrator privileges required)".into(),
        });
        return;
    }

    let secure_path = crate::winutil::get_secure_system_path();
    let status = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&[
            "-NoProfile",
            "-Command",
            "Checkpoint-Computer -Description \"PurgeKit Restore Point\" -RestorePointType \"APPLICATION_UNINSTALL\""
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            let _ = app.emit("purge-progress", ProgressPayload {
                phase: "restore_point".into(),
                current: 100,
                total: 100,
                message: "System Restore Point created successfully!".into(),
            });
        }
        _ => {
            let _ = app.emit("purge-progress", ProgressPayload {
                phase: "restore_point_warning".into(),
                current: 0,
                total: 100,
                message: "Failed to create Restore Point (System Protection might be disabled, or a checkpoint was created recently)".into(),
            });
        }
    }
}

#[tauri::command]
pub async fn get_startup_items() -> Result<Vec<crate::startup_manager::StartupItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::startup_manager::list_startup_items()
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_startup_item_status(id: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::startup_manager::set_startup_item_status(id, enabled)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_startup_item(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::startup_manager::delete_startup_item(id)
    }).await.map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
pub struct QuarantineItem {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub quarantine_path: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_quarantine_items() -> Result<Vec<QuarantineItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db_path = crate::db::get_db_path();
        let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, name, original_path, quarantine_path, created_at FROM quarantine ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map([], |row| {
            Ok(QuarantineItem {
                id: row.get(0)?,
                name: row.get(1)?,
                original_path: row.get(2)?,
                quarantine_path: row.get(3)?,
                created_at: row.get(4)?,
            })
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for item in rows {
            if let Ok(i) = item {
                items.push(i);
            }
        }
        Ok(items)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn restore_quarantine_item(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db_path = crate::db::get_db_path();
        let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        
        // Find quarantine item
        let item: QuarantineItem = conn.query_row(
            "SELECT id, name, original_path, quarantine_path, created_at FROM quarantine WHERE id = ?1",
            [&id],
            |row| {
                Ok(QuarantineItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    original_path: row.get(2)?,
                    quarantine_path: row.get(3)?,
                    created_at: row.get(4)?,
                })
            }
        ).map_err(|e| format!("Quarantine item not found: {}", e))?;

        let src_path = std::path::Path::new(&item.quarantine_path);
        if !src_path.exists() {
            return Err("Quarantined file does not exist on disk".to_string());
        }

        let dest_path = std::path::Path::new(&item.original_path);
        
        if let Err(e) = crate::winutil::is_safe_to_delete(&item.original_path) {
            return Err(format!("Restoration blocked: {}", e));
        }
        
        // Admin pre-check if restoring to protected system folders
        let dest_upper = item.original_path.to_uppercase();
        let is_system = dest_upper.contains("PROGRAM FILES") 
            || dest_upper.contains("WINDOWS") 
            || dest_upper.starts_with("C:\\PROGRAMDATA");
            
        if is_system && !crate::settings::check_is_admin() {
            return Err("Restoring system files requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        // Create parent directories if they don't exist
        if let Some(parent) = dest_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Try renaming first
        if std::fs::rename(src_path, dest_path).is_err() {
            // Fallback: copy and delete
            if src_path.is_dir() {
                let secure_path = crate::winutil::get_secure_system_path();
                let status = std::process::Command::new("cmd")
                    .creation_flags(0x08000000)
                    .env("PATH", &secure_path)
                    .args(&["/C", &format!("xcopy /E /I /Y \"{}\" \"{}\"", item.quarantine_path, item.original_path)])
                    .status();
                if status.map_or(false, |s| s.success()) {
                    let _ = std::fs::remove_dir_all(src_path);
                } else {
                    return Err("Failed to restore directory".to_string());
                }
            } else {
                std::fs::copy(src_path, dest_path).map_err(|e| format!("Failed to copy file for restoration: {}", e))?;
                let _ = std::fs::remove_file(src_path);
            }
        }

        // Remove row from DB
        conn.execute("DELETE FROM quarantine WHERE id = ?1", [&id]).map_err(|e| e.to_string())?;

        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_quarantine_item(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db_path = crate::db::get_db_path();
        let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        
        let item: QuarantineItem = conn.query_row(
            "SELECT id, name, original_path, quarantine_path, created_at FROM quarantine WHERE id = ?1",
            [&id],
            |row| {
                Ok(QuarantineItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    original_path: row.get(2)?,
                    quarantine_path: row.get(3)?,
                    created_at: row.get(4)?,
                })
            }
        ).map_err(|e| format!("Quarantine item not found: {}", e))?;

        let q_path = std::path::Path::new(&item.quarantine_path);
        if q_path.exists() {
            if q_path.is_dir() {
                std::fs::remove_dir_all(q_path).map_err(|e| format!("Failed to delete directory: {}", e))?;
            } else {
                std::fs::remove_file(q_path).map_err(|e| format!("Failed to delete file: {}", e))?;
            }
        }

        conn.execute("DELETE FROM quarantine WHERE id = ?1", [&id]).map_err(|e| e.to_string())?;

        Ok(())
    }).await.map_err(|e| e.to_string())?
}

pub fn get_silent_uninstall_command(app: &InstalledApp) -> Option<String> {
    if app.is_uwp {
        return app.uninstall_string.clone();
    }

    if let Some(ref quiet) = app.quiet_uninstall_string {
        if !quiet.trim().is_empty() {
            return Some(quiet.clone());
        }
    }

    let uninst = app.uninstall_string.as_ref()?;
    let uninst_lower = uninst.to_lowercase();

    // 1. MSI Installer
    if uninst_lower.contains("msiexec.exe") || (app.id.starts_with('{') && app.id.ends_with('}')) {
        if let Some(guid_start) = uninst_lower.find("{") {
            let guid = &uninst[guid_start..];
            return Some(format!("MsiExec.exe /X{} /qn /norestart", guid));
        }
    }

    // 2. Inno Setup (unins000.exe)
    if uninst_lower.contains("unins000.exe") {
        return Some(format!("{} /VERYSILENT /SUPPRESSMSGBOXES /NORESTART", uninst));
    }

    // 3. Nullsoft Installer (NSIS) (uninstall.exe / uninst.exe)
    if uninst_lower.contains("uninstall.exe") || uninst_lower.contains("uninst.exe") {
        return Some(format!("{} /S", uninst));
    }

    // 4. Advanced Installer
    if uninst_lower.contains("setup.exe") {
        return Some(format!("{} /qn", uninst));
    }

    Some(uninst.clone())
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
pub struct BulkUninstallProgress {
    pub app_id: String,
    pub app_name: String,
    pub phase: String, // "uninstalling", "scanning_leftovers", "completed", "error"
    pub current: u32,
    pub total: u32,
    pub message: String,
}

#[tauri::command]
pub async fn run_bulk_silent_uninstall(
    app: AppHandle,
    app_ids: Vec<String>,
    auto_purge: bool,
) -> Result<Vec<RemnantItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut all_apps = scanner::scan_registry();
        all_apps.extend(scanner::scan_uwp_apps());

        let total = app_ids.len() as u32;
        let mut all_remaining_remnants = Vec::new();

        for (index, id) in app_ids.iter().enumerate() {
            let current = (index + 1) as u32;
            let target_app = match all_apps.iter().find(|a| &a.id == id) {
                Some(a) => a,
                None => {
                    let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                        app_id: id.clone(),
                        app_name: id.clone(),
                        phase: "error".into(),
                        current,
                        total,
                        message: format!("App with ID '{}' not found in registry.", id),
                    });
                    continue;
                }
            };

            let app_name = target_app.display_name.clone();

            let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                app_id: id.clone(),
                app_name: app_name.clone(),
                phase: "uninstalling".into(),
                current,
                total,
                message: format!("Uninstalling {} silently...", app_name),
            });

            let secure_path = crate::winutil::get_secure_system_path();
            let uninstall_success = if target_app.is_uwp {
                if let Some(ref uninst_str) = target_app.uninstall_string {
                    let prefix = "powershell -NoProfile -Command \"Remove-AppxPackage -Package ";
                    if let Some(package_full) = uninst_str.strip_prefix(prefix).and_then(|s| s.strip_suffix('"')) {
                        if package_full.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-') {
                            let status = Command::new("powershell")
                                .creation_flags(CREATE_NO_WINDOW)
                                .env("PATH", &secure_path)
                                .args(&["-NoProfile", "-Command", &format!("Remove-AppxPackage -Package {}", package_full)])
                                .status();
                            status.map(|s| s.success()).unwrap_or(false)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else if let Some(silent_cmd) = get_silent_uninstall_command(target_app) {
                if is_safe_uninstall_string(&silent_cmd) && validate_uninstall_command_safety(&silent_cmd).is_ok() {
                    let status = Command::new("cmd")
                        .creation_flags(CREATE_NO_WINDOW)
                        .env("PATH", &secure_path)
                        .args(&["/C", &silent_cmd])
                        .status();
                    status.map(|s| s.success()).unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };

            if !uninstall_success {
                let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                    app_id: id.clone(),
                    app_name: app_name.clone(),
                    phase: "error".into(),
                    current,
                    total,
                    message: format!("Failed to run silent uninstaller for {}.", app_name),
                });
            }

            let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                app_id: id.clone(),
                app_name: app_name.clone(),
                phase: "scanning_leftovers".into(),
                current,
                total,
                message: format!("Scanning remnants for {}...", app_name),
            });

            let remnants = scanner::scan_app_remnants(
                &target_app.display_name,
                target_app.publisher.as_deref(),
                target_app.install_location.as_deref(),
            );

            if auto_purge {
                let mut success = 0;
                let mut fail = 0;
                let mut remaining = Vec::new();

                for item in remnants {
                    if item.score >= 60 {
                        match scanner::remnants::purge_remnant_item(&item) {
                            Ok(_) => success += 1,
                            Err(_) => {
                                fail += 1;
                                remaining.push(item);
                            }
                        }
                    } else {
                        remaining.push(item);
                    }
                }

                all_remaining_remnants.extend(remaining);

                let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                    app_id: id.clone(),
                    app_name: app_name.clone(),
                    phase: "completed".into(),
                    current,
                    total,
                    message: format!(
                        "Auto-purged {} items for {} (Failed: {}).",
                        success, app_name, fail
                    ),
                });
            } else {
                all_remaining_remnants.extend(remnants);
                let _ = app.emit("bulk-uninstall-progress", BulkUninstallProgress {
                    app_id: id.clone(),
                    app_name: app_name.clone(),
                    phase: "completed".into(),
                    current,
                    total,
                    message: format!("Uninstalled and collected remnants for {}.", app_name),
                });
            }
        }

        crate::broadcast::broadcast_all_system_changes();

        Ok(all_remaining_remnants)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn scan_project_directories(
    app: AppHandle,
    roots: Vec<String>,
    folder_types: Vec<String>,
) -> Result<Vec<ProjectFolder>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::scan_project_folders(&app, &roots, &folder_types)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_project_directories(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut results = std::collections::HashMap::new();
        let total = paths.len() as u32;

        for (index, path_str) in paths.iter().enumerate() {
            let current = (index + 1) as u32;
            let _ = app.emit("project-delete-progress", ProgressPayload {
                phase: "deleting".into(),
                current,
                total,
                message: format!("Deleting project directory: {}", path_str),
            });

            match crate::locker::delete_file_with_escalation(path_str) {
                crate::locker::DeleteResult::Deleted | crate::locker::DeleteResult::DeletedAfterUnlock | crate::locker::DeleteResult::ForceDeleted => {
                    results.insert(path_str.clone(), "Deleted".to_string());
                }
                crate::locker::DeleteResult::ScheduledForReboot => {
                    results.insert(path_str.clone(), "ScheduledForReboot".to_string());
                }
                crate::locker::DeleteResult::Failed(err) => {
                    results.insert(path_str.clone(), format!("Failed: {}", err));
                }
            }
        }

        let _ = app.emit("project-delete-progress", ProgressPayload {
            phase: "completed".into(),
            current: total,
            total,
            message: "Project directories cleanup finished.".into(),
        });

        Ok(results)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_wsl_distros() -> Result<Vec<WslDistroInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::scan_wsl_distributions())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn compact_wsl_distro(
    app: AppHandle,
    _name: String,
    vhdx_path: String,
) -> Result<String, String> {
    if !settings::check_is_admin() {
        return Err("WSL disk compaction using diskpart requires Administrator privileges. Please run PurgeKit as Administrator.".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        scanner::compact_vhdx_diskpart(&app, &vhdx_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_wsl_distro_sparse_mode(
    app: AppHandle,
    name: String,
    sparse: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::set_wsl_distro_sparse(&app, &name, sparse)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_toolchain_versions() -> Result<Vec<ToolchainVersion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(scanner::scan_toolchain_versions())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_toolchain_version(
    manager: String,
    version: String,
    path: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        scanner::uninstall_toolchain_version(&manager, &version, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_directory_exists(path: String) -> Result<bool, String> {
    let p = std::path::Path::new(&path);
    Ok(p.exists() && p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_app(id: &str, uninst: Option<&str>, quiet: Option<&str>, is_uwp: bool) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            display_name: "Test App".to_string(),
            display_version: None,
            publisher: None,
            uninstall_string: uninst.map(|s| s.to_string()),
            quiet_uninstall_string: quiet.map(|s| s.to_string()),
            install_location: None,
            install_date: None,
            display_icon: None,
            icon_base64: None,
            estimated_size: None,
            registry_path: "".to_string(),
            hive: "".to_string(),
            is_uwp,
            is_verified: None,
        }
    }

    #[test]
    fn test_get_silent_uninstall_command() {
        // 1. App registers quiet_uninstall_string
        let app = create_mock_app("test", Some("uninstall.exe"), Some("uninstall.exe /quiet"), false);
        assert_eq!(get_silent_uninstall_command(&app), Some("uninstall.exe /quiet".to_string()));

        // 2. UWP App
        let app = create_mock_app("test_uwp", Some("powershell Remove-AppxPackage"), None, true);
        assert_eq!(get_silent_uninstall_command(&app), Some("powershell Remove-AppxPackage".to_string()));

        // 3. MSI App with GUID in ID
        let app = create_mock_app("{1234-5678}", Some("msiexec.exe /I{1234-5678}"), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("MsiExec.exe /X{1234-5678} /qn /norestart".to_string()));

        // 4. MSI App with msiexec.exe in uninst string but GUID is elsewhere
        let app = create_mock_app("some-msi", Some("C:\\Windows\\System32\\msiexec.exe /I {ABC-DEF}"), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("MsiExec.exe /X{ABC-DEF} /qn /norestart".to_string()));

        // 5. Inno Setup
        let app = create_mock_app("inno", Some("\"C:\\Program Files\\App\\unins000.exe\""), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("\"C:\\Program Files\\App\\unins000.exe\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART".to_string()));

        // 6. NSIS
        let app = create_mock_app("nsis", Some("C:\\Program Files\\App\\uninstall.exe"), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("C:\\Program Files\\App\\uninstall.exe /S".to_string()));

        // 7. Advanced Installer
        let app = create_mock_app("adv", Some("C:\\Program Files\\App\\setup.exe"), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("C:\\Program Files\\App\\setup.exe /qn".to_string()));

        // 8. Fallback
        let app = create_mock_app("custom", Some("C:\\Program Files\\App\\custom_cleaner.exe"), None, false);
        assert_eq!(get_silent_uninstall_command(&app), Some("C:\\Program Files\\App\\custom_cleaner.exe".to_string()));
    }
}

