use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub enabled: bool,
    pub is_registry: bool,
    pub registry_path: Option<String>,
    pub file_path: Option<String>,
}

fn get_hive_from_name(hive_name: &str) -> Option<winreg::HKEY> {
    match hive_name {
        "HKCU" => Some(HKEY_CURRENT_USER),
        "HKLM" => Some(HKEY_LOCAL_MACHINE),
        _ => None,
    }
}

pub fn list_startup_items() -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();

    // 1. Scan Registry Run keys
    let run_locations = vec![
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"),
        ("HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
    ];

    for (hive_name, subpath) in run_locations {
        let hive = match get_hive_from_name(hive_name) {
            Some(h) => h,
            None => continue,
        };
        let root = RegKey::predef(hive);
        
        // Scan active ones
        if let Ok(key) = root.open_subkey_with_flags(subpath, KEY_READ) {
            for (name, val) in key.enum_values().filter_map(|x| x.ok()) {
                let command_str = match key.get_value::<String, _>(&name) {
                    Ok(s) => s.replace('\0', "").trim().to_string(),
                    Err(_) => String::from_utf8_lossy(&val.bytes).replace('\0', "").trim().to_string(),
                };
                let id = format!("reg|{}|{}|{}", hive_name, subpath, name);
                items.push(StartupItem {
                    id,
                    name: name.clone(),
                    command: command_str,
                    location: format!("{} - {}", hive_name, subpath.split('\\').next_back().unwrap_or("Run")),
                    enabled: true,
                    is_registry: true,
                    registry_path: Some(format!(r"{}\{}", subpath, name)),
                    file_path: None,
                });
            }
        }

        // Scan disabled ones (in PurgeKit_Disabled subkey)
        let disabled_subpath = format!(r"{}\PurgeKit_Disabled", subpath);
        if let Ok(key) = root.open_subkey_with_flags(&disabled_subpath, KEY_READ) {
            for (name, val) in key.enum_values().filter_map(|x| x.ok()) {
                let command_str = match key.get_value::<String, _>(&name) {
                    Ok(s) => s.replace('\0', "").trim().to_string(),
                    Err(_) => String::from_utf8_lossy(&val.bytes).replace('\0', "").trim().to_string(),
                };
                let id = format!("reg|{}|{}|{}", hive_name, subpath, name);
                items.push(StartupItem {
                    id,
                    name: name.clone(),
                    command: command_str,
                    location: format!("{} - {} (Disabled)", hive_name, subpath.split('\\').next_back().unwrap_or("Run")),
                    enabled: false,
                    is_registry: true,
                    registry_path: Some(format!(r"{}\{}", subpath, name)),
                    file_path: None,
                });
            }
        }
    }

    // 2. Scan Startup Folder keys
    let mut startup_dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        startup_dirs.push((appdata.join(r"Microsoft\Windows\Start Menu\Programs\Startup"), "Startup Folder (User)"));
    }
    if let Some(programdata) = std::env::var_os("ProgramData").map(PathBuf::from) {
        startup_dirs.push((programdata.join(r"Microsoft\Windows\Start Menu\Programs\Startup"), "Startup Folder (Common)"));
    }

    for (dir, loc_name) in startup_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    if file_name.is_empty() {
                        continue;
                    }
                    
                    let has_lnk = file_name.to_lowercase().ends_with(".lnk") || file_name.to_lowercase().contains(".lnk.disabled");
                    let has_disabled = file_name.to_lowercase().ends_with(".disabled");
                    
                    if has_lnk || has_disabled || file_name.to_lowercase().ends_with(".exe") || file_name.to_lowercase().contains(".exe.disabled") {
                        let id = format!("file|{}", path.to_string_lossy());
                        let enabled = !has_disabled;
                        
                        let mut display_name = file_name.clone();
                        if display_name.to_lowercase().ends_with(".disabled") {
                            display_name = display_name[..display_name.len() - 9].to_string();
                        }
                        if display_name.to_lowercase().ends_with(".lnk") {
                            display_name = display_name[..display_name.len() - 4].to_string();
                        }
                        if display_name.to_lowercase().ends_with(".exe") {
                            display_name = display_name[..display_name.len() - 4].to_string();
                        }
                        
                        items.push(StartupItem {
                            id,
                            name: display_name,
                            command: path.to_string_lossy().to_string(),
                            location: loc_name.to_string(),
                            enabled,
                            is_registry: false,
                            registry_path: None,
                            file_path: Some(path.to_string_lossy().to_string()),
                        });
                    }
                }
            }
        }
    }

    Ok(items)
}

pub fn set_startup_item_status(id: String, enabled: bool) -> Result<(), String> {
    let parts: Vec<&str> = id.split('|').collect();
    if parts.len() < 2 {
        return Err("Invalid startup ID".to_string());
    }

    let item_type = parts[0];
    if item_type == "reg" {
        if parts.len() < 4 {
            return Err("Invalid registry startup ID".to_string());
        }
        let hive_name = parts[1];
        let subpath = parts[2];
        let value_name = parts[3];

        if hive_name == "HKLM" && !crate::settings::check_is_admin() {
            return Err("Modifying system startup items (HKLM) requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let hive = get_hive_from_name(hive_name)
            .ok_or_else(|| "Invalid registry hive".to_string())?;
        let root = RegKey::predef(hive);

        if enabled {
            // Enable: Move from PurgeKit_Disabled subkey to parent key
            let disabled_subpath = format!(r"{}\PurgeKit_Disabled", subpath);
            let disabled_key = root.open_subkey_with_flags(&disabled_subpath, KEY_READ | KEY_WRITE)
                .map_err(|e| format!("Failed to open disabled registry key: {}", e))?;
            
            let raw_val = disabled_key.get_raw_value(value_name)
                .map_err(|e| format!("Failed to read disabled registry value: {}", e))?;
            
            let parent_key = root.open_subkey_with_flags(subpath, KEY_WRITE)
                .map_err(|e| format!("Failed to open run registry key: {}", e))?;
            
            parent_key.set_raw_value(value_name, &raw_val)
                .map_err(|e| format!("Failed to write registry value: {}", e))?;
            
            let _ = disabled_key.delete_value(value_name);
        } else {
            // Disable: Move from parent key to PurgeKit_Disabled subkey
            let parent_key = root.open_subkey_with_flags(subpath, KEY_READ | KEY_WRITE)
                .map_err(|e| format!("Failed to open run registry key: {}", e))?;
            
            let raw_val = parent_key.get_raw_value(value_name)
                .map_err(|e| format!("Failed to read registry value: {}", e))?;
            
            let disabled_subpath = format!(r"{}\PurgeKit_Disabled", subpath);
            let (disabled_key, _) = root.create_subkey_with_flags(&disabled_subpath, KEY_WRITE)
                .map_err(|e| format!("Failed to create/open disabled registry key: {}", e))?;
            
            disabled_key.set_raw_value(value_name, &raw_val)
                .map_err(|e| format!("Failed to save disabled registry value: {}", e))?;
            
            let _ = parent_key.delete_value(value_name);
        }
    } else if item_type == "file" {
        let file_path_str = parts[1];
        let path = Path::new(file_path_str);
        if !path.exists() {
            return Err("Startup file does not exist".to_string());
        }

        // For system folder (Common Startup), check admin rights
        let is_system = file_path_str.to_uppercase().contains("PROGRAMDATA");
        if is_system && !crate::settings::check_is_admin() {
            return Err("Modifying system startup items requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let parent = path.parent().ok_or_else(|| "Invalid parent directory".to_string())?;

        if enabled {
            // Enable: Remove .disabled extension if present
            if file_name.to_lowercase().ends_with(".disabled") {
                let new_name = &file_name[..file_name.len() - 9];
                let new_path = parent.join(new_name);
                fs::rename(path, new_path)
                    .map_err(|e| format!("Failed to enable startup file: {}", e))?;
            }
        } else {
            // Disable: Append .disabled extension
            if !file_name.to_lowercase().ends_with(".disabled") {
                let new_name = format!("{}.disabled", file_name);
                let new_path = parent.join(new_name);
                fs::rename(path, new_path)
                    .map_err(|e| format!("Failed to disable startup file: {}", e))?;
            }
        }
    } else {
        return Err("Unsupported startup type".to_string());
    }

    Ok(())
}

pub fn delete_startup_item(id: String) -> Result<(), String> {
    let parts: Vec<&str> = id.split('|').collect();
    if parts.len() < 2 {
        return Err("Invalid startup ID".to_string());
    }

    let item_type = parts[0];
    if item_type == "reg" {
        if parts.len() < 4 {
            return Err("Invalid registry startup ID".to_string());
        }
        let hive_name = parts[1];
        let subpath = parts[2];
        let value_name = parts[3];

        if hive_name == "HKLM" && !crate::settings::check_is_admin() {
            return Err("Deleting system startup items (HKLM) requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let hive = get_hive_from_name(hive_name)
            .ok_or_else(|| "Invalid registry hive".to_string())?;
        let root = RegKey::predef(hive);

        // Try to delete from active Run key
        if let Ok(key) = root.open_subkey_with_flags(subpath, KEY_WRITE) {
            let _ = key.delete_value(value_name);
        }
        // Try to delete from PurgeKit_Disabled subkey
        let disabled_subpath = format!(r"{}\PurgeKit_Disabled", subpath);
        if let Ok(key) = root.open_subkey_with_flags(&disabled_subpath, KEY_WRITE) {
            let _ = key.delete_value(value_name);
        }
    } else if item_type == "file" {
        let file_path_str = parts[1];
        let path = Path::new(file_path_str);
        if !path.exists() {
            return Ok(()); // Already deleted
        }

        let is_system = file_path_str.to_uppercase().contains("PROGRAMDATA");
        if is_system && !crate::settings::check_is_admin() {
            return Err("Deleting system startup items requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        if path.is_file() {
            fs::remove_file(path)
                .map_err(|e| format!("Failed to delete startup file: {}", e))?;
        }
    } else {
        return Err("Unsupported startup type".to_string());
    }

    Ok(())
}
