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

fn is_in_startup_dir(path_str: &str) -> bool {
    let path = Path::new(path_str);
    let path_norm = crate::winutil::canonicalize_path_safety(&path.to_string_lossy());
    let path_norm_str = path_norm.to_string_lossy().to_lowercase();

    let mut startup_dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        startup_dirs.push(appdata.join(r"Microsoft\Windows\Start Menu\Programs\Startup"));
    }
    if let Some(programdata) = std::env::var_os("ProgramData").map(PathBuf::from) {
        startup_dirs.push(programdata.join(r"Microsoft\Windows\Start Menu\Programs\Startup"));
    }

    for dir in startup_dirs {
        let dir_norm = crate::winutil::canonicalize_path_safety(&dir.to_string_lossy());
        let dir_norm_str = dir_norm.to_string_lossy().to_lowercase();

        if path_norm_str.starts_with(&dir_norm_str) {
            let remain = &path_norm_str[dir_norm_str.len()..];
            if remain.is_empty() || remain.starts_with('\\') || remain.starts_with('/') {
                return true;
            }
        }
    }
    false
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle_lower = needle.to_lowercase();
    let needle_chars_len = needle.chars().count();
    
    for (char_idx, (byte_idx, _)) in haystack.char_indices().enumerate() {
        let mut chars = haystack[byte_idx..].chars();
        let mut substring = String::new();
        for _ in 0..needle_chars_len {
            if let Some(c) = chars.next() {
                substring.push(c);
            } else {
                break;
            }
        }
        if substring.to_lowercase() == needle_lower {
            return Some(byte_idx);
        }
    }
    None
}

fn extract_xml_tag(xml: &str, tag_name: &str) -> Option<String> {
    let open_tag = format!("<{}>", tag_name);
    let close_tag = format!("</{}>", tag_name);
    
    let start_idx = find_case_insensitive(xml, &open_tag)?;
    let content_start = start_idx + open_tag.len();
    
    let remaining = &xml[content_start..];
    let end_offset = find_case_insensitive(remaining, &close_tag)?;
    let content_end = content_start + end_offset;
    
    Some(xml[content_start..content_end].trim().to_string())
}

fn is_in_tasks_dir(path_str: &str) -> bool {
    let path = Path::new(path_str);
    let path_norm = crate::winutil::canonicalize_path_safety(&path.to_string_lossy());
    let path_norm_str = path_norm.to_string_lossy().to_lowercase();

    let windir = std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    let dir_norm = crate::winutil::canonicalize_path_safety(&tasks_dir.to_string_lossy());
    let dir_norm_str = dir_norm.to_string_lossy().to_lowercase();

    if path_norm_str.starts_with(&dir_norm_str) {
        let remain = &path_norm_str[dir_norm_str.len()..];
        if remain.is_empty() || remain.starts_with('\\') || remain.starts_with('/') {
            return true;
        }
    }
    false
}

fn scan_tasks_directory(dir: &Path, items: &mut Vec<StartupItem>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            scan_tasks_directory(&path, items);
        } else if path.is_file() {
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if file_name.starts_with('.') {
                continue;
            }
            
            let xml_content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            
            let is_disabled = file_name.ends_with(".disabled");
            let mut clean_name = file_name.to_string();
            if is_disabled {
                clean_name = clean_name[..clean_name.len() - 9].to_string();
            }
            
            let has_logon = find_case_insensitive(&xml_content, "<logontrigger>").is_some();
            let has_boot = find_case_insensitive(&xml_content, "<boottrigger>").is_some();
            
            if has_logon || has_boot {
                let trigger_type = if has_logon && has_boot {
                    "Scheduled Task (Logon/Boot)"
                } else if has_logon {
                    "Scheduled Task (Logon)"
                } else {
                    "Scheduled Task (Boot)"
                };
                
                let command = extract_xml_tag(&xml_content, "command").unwrap_or_default();
                let arguments = extract_xml_tag(&xml_content, "arguments").unwrap_or_default();
                
                let mut full_command = if command.is_empty() {
                    String::new()
                } else if arguments.is_empty() {
                    command
                } else {
                    format!("{} {}", command, arguments)
                };
                
                if full_command.is_empty() {
                    if let Some(class_id) = extract_xml_tag(&xml_content, "classid") {
                        full_command = format!("COM Handler: {}", class_id);
                    } else {
                        full_command = "Scheduled Action".to_string();
                    }
                }
                
                let id = format!("task|{}", path.to_string_lossy());
                items.push(StartupItem {
                    id,
                    name: clean_name,
                    command: full_command,
                    location: trigger_type.to_string(),
                    enabled: !is_disabled,
                    is_registry: false,
                    registry_path: None,
                    file_path: Some(path.to_string_lossy().to_string()),
                });
            }
        }
    }
}

pub fn list_startup_items() -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();

    // 1. Scan Registry Run keys
    let run_locations = vec![
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKCU", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"),
        ("HKCU", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunServices"),
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunServicesOnce"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"),
        ("HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunServices"),
        ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunServicesOnce"),
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

    // 3. Scan Scheduled Tasks XML files
    let windir = std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    if tasks_dir.exists() {
        scan_tasks_directory(&tasks_dir, &mut items);
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
        if !is_in_startup_dir(file_path_str) {
            return Err("Access Denied: Startup file must reside inside a Windows Startup folder.".to_string());
        }
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
    } else if item_type == "task" {
        let file_path_str = parts[1];
        if !is_in_tasks_dir(file_path_str) {
            return Err("Access Denied: Scheduled task file must reside inside the Windows Tasks folder.".to_string());
        }
        if !crate::settings::check_is_admin() {
            return Err("Modifying scheduled tasks requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let path = Path::new(file_path_str);
        if !path.exists() {
            return Err("Scheduled task file does not exist".to_string());
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let parent = path.parent().ok_or_else(|| "Invalid parent directory".to_string())?;

        if enabled {
            // Enable: Remove .disabled extension if present
            if file_name.to_lowercase().ends_with(".disabled") {
                let new_name = &file_name[..file_name.len() - 9];
                let new_path = parent.join(new_name);
                fs::rename(path, new_path)
                    .map_err(|e| format!("Failed to enable scheduled task: {}", e))?;
            }
        } else {
            // Disable: Append .disabled extension
            if !file_name.to_lowercase().ends_with(".disabled") {
                let new_name = format!("{}.disabled", file_name);
                let new_path = parent.join(new_name);
                fs::rename(path, new_path)
                    .map_err(|e| format!("Failed to disable scheduled task: {}", e))?;
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

        let full_reg_path = format!(r"{}\{}", subpath, value_name);
        if let Err(e) = crate::winutil::is_safe_registry_key(hive_name, &full_reg_path) {
            return Err(e);
        }

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
        if !is_in_startup_dir(file_path_str) {
            return Err("Access Denied: Startup file must reside inside a Windows Startup folder.".to_string());
        }
        if let Err(e) = crate::winutil::is_safe_to_delete(file_path_str) {
            return Err(e);
        }

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
    } else if item_type == "task" {
        let file_path_str = parts[1];
        if !is_in_tasks_dir(file_path_str) {
            return Err("Access Denied: Scheduled task file must reside inside the Windows Tasks folder.".to_string());
        }
        if !crate::settings::check_is_admin() {
            return Err("Deleting scheduled tasks requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }
        if let Err(e) = crate::winutil::is_safe_to_delete(file_path_str) {
            return Err(e);
        }

        let path = Path::new(file_path_str);
        if !path.exists() {
            return Ok(()); // Already deleted
        }

        if path.is_file() {
            fs::remove_file(path)
                .map_err(|e| format!("Failed to delete scheduled task: {}", e))?;
        }
    } else {
        return Err("Unsupported startup type".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_in_startup_dir() {
        let appdata = std::env::var("APPDATA").ok().map(PathBuf::from);
        if let Some(dir) = appdata {
            let startup = dir.join(r"Microsoft\Windows\Start Menu\Programs\Startup");
            let file = startup.join("test.exe");
            assert!(is_in_startup_dir(&file.to_string_lossy()));
            
            let sub_file = startup.join("nested").join("test.exe");
            assert!(is_in_startup_dir(&sub_file.to_string_lossy()));
        }
        
        assert!(!is_in_startup_dir(r"C:\Windows\System32\cmd.exe"));
        assert!(!is_in_startup_dir(r"C:\Users\Public\Documents\some_file.txt"));
    }

    #[test]
    fn test_startup_item_outside_startup_denied() {
        let result_set = set_startup_item_status("file|C:\\Windows\\System32\\cmd.exe".to_string(), false);
        assert!(result_set.is_err());
        assert!(result_set.unwrap_err().contains("Access Denied"));

        let result_del = delete_startup_item("file|C:\\Windows\\System32\\cmd.exe".to_string());
        assert!(result_del.is_err());
        assert!(result_del.unwrap_err().contains("Access Denied"));
    }

    #[test]
    fn test_is_in_tasks_dir() {
        let windir = std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string());
        let tasks = Path::new(&windir).join("System32").join("Tasks");
        let file = tasks.join("GoogleUpdateTaskMachineUA");
        assert!(is_in_tasks_dir(&file.to_string_lossy()));
        
        let sub_file = tasks.join("Microsoft").join("Windows").join("WindowsUpdate");
        assert!(is_in_tasks_dir(&sub_file.to_string_lossy()));

        assert!(!is_in_tasks_dir(r"C:\Windows\System32\cmd.exe"));
    }

    #[test]
    fn test_scheduled_task_outside_tasks_denied() {
        let result_set = set_startup_item_status("task|C:\\Windows\\System32\\cmd.exe".to_string(), false);
        assert!(result_set.is_err());
        assert!(result_set.unwrap_err().contains("Access Denied"));

        let result_del = delete_startup_item("task|C:\\Windows\\System32\\cmd.exe".to_string());
        assert!(result_del.is_err());
        assert!(result_del.unwrap_err().contains("Access Denied"));
    }
}
