use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

const ALLOWED_RUN_SUBPATHS: &[&str] = &[
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
    r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run",
    r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce",
];

fn parse_registry_startup_id(id: &str) -> Result<(&str, &str, &str), String> {
    let mut parts = id.splitn(4, '|');
    if parts.next() != Some("reg") {
        return Err("Invalid registry startup ID".to_string());
    }

    let hive_name = parts
        .next()
        .ok_or_else(|| "Invalid registry startup ID".to_string())?;
    let subpath = parts
        .next()
        .ok_or_else(|| "Invalid registry startup ID".to_string())?;
    // splitn(4, ...) keeps any '|' in the value name intact.
    let value_name = parts
        .next()
        .ok_or_else(|| "Invalid registry startup ID".to_string())?;

    if !matches!(hive_name, "HKCU" | "HKLM") {
        return Err("Invalid registry hive".to_string());
    }
    if !ALLOWED_RUN_SUBPATHS.contains(&subpath) {
        return Err("Access Denied: startup registry path is not an allowed Run/RunOnce key.".to_string());
    }
    if value_name.trim().is_empty() {
        return Err("Invalid startup value name".to_string());
    }

    Ok((hive_name, subpath, value_name))
}

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
    pub icon_base64: Option<String>,
    pub target_missing: bool,
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
            if !remain.is_empty() && (remain.starts_with('\\') || remain.starts_with('/')) {
                return true;
            }
        }
    }
    false
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle_lower = needle.to_lowercase();
    let needle_chars_len = needle.chars().count();
    
    for (byte_idx, _) in haystack.char_indices() {
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

    let windir = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    let dir_norm = crate::winutil::canonicalize_path_safety(&tasks_dir.to_string_lossy());
    let dir_norm_str = dir_norm.to_string_lossy().to_lowercase();

    if path_norm_str.starts_with(&dir_norm_str) {
        let remain = &path_norm_str[dir_norm_str.len()..];
        if !remain.is_empty() && (remain.starts_with('\\') || remain.starts_with('/')) {
            return true;
        }
    }
    false
}

/// Windows servicing/update tasks live under \Microsoft\Windows. They are
/// not user startup programs and deleting them can break the OS.
pub fn is_protected_task_name(task_name: &str) -> bool {
    task_name
        .trim()
        .to_lowercase()
        .starts_with(r"\microsoft\windows\")
}

pub(crate) fn is_task_file_path(path_str: &str) -> bool {
    is_in_tasks_dir(path_str) && Path::new(path_str).is_file()
}

pub fn get_task_name_from_path(task_path: &Path) -> Option<String> {
    let windir = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    task_path.strip_prefix(&tasks_dir).ok().map(|rel| {
        let s = rel.to_string_lossy().replace('/', "\\");
        let s_trimmed = if s.ends_with(".disabled") {
            s[..s.len() - 9].to_string()
        } else {
            s
        };
        if !s_trimmed.starts_with('\\') {
            format!("\\{}", s_trimmed)
        } else {
            s_trimmed
        }
    })
}

fn scan_tasks_directory(dir: &Path, items: &mut Vec<StartupItem>, icon_cache: &mut HashMap<String, Option<String>>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            scan_tasks_directory(&path, items, icon_cache);
        } else if path.is_file() {
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if file_name.starts_with('.') {
                continue;
            }
            
            let xml_content = match crate::winutil::read_file_utf8_or_utf16(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            
            let is_disabled = file_name.ends_with(".disabled") || find_case_insensitive(&xml_content, "<enabled>false</enabled>").is_some();
            let mut clean_name = file_name.to_string();
            if clean_name.ends_with(".disabled") {
                clean_name = clean_name[..clean_name.len() - 9].to_string();
            }
            
            let has_logon = find_case_insensitive(&xml_content, "<logontrigger>").is_some();
            let has_boot = find_case_insensitive(&xml_content, "<boottrigger>").is_some();
            
            if has_logon || has_boot {
                if let Some(task_name) = get_task_name_from_path(&path) {
                    if is_protected_task_name(&task_name) {
                        continue;
                    }
                }

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
                
                let task_icon = startup_icon_base64(
                    startup_icon_source(&full_command, None),
                    icon_cache,
                );
                let target_missing = startup_target_missing(&full_command);
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
                    icon_base64: task_icon,
                    target_missing,
                });
            }
        }
    }
}

fn startup_target_missing(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    let executable = crate::winutil::extract_executable_path(command);
    let path = Path::new(executable.trim().trim_matches('"'));
    path.is_absolute() && !path.exists()
}

fn startup_icon_source(command: &str, file_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = file_path {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    if command.trim().is_empty() {
        return None;
    }
    let executable = crate::winutil::extract_executable_path(command);
    let path = Path::new(executable.trim().trim_matches('"'));
    path.is_file().then(|| path.to_path_buf())
}

fn startup_icon_base64(
    source: Option<PathBuf>,
    cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    let source = source?;
    let key = source.to_string_lossy().to_lowercase();
    if let Some(cached) = cache.get(&key) {
        return cached.clone();
    }
    let icon = windows_icons::get_icon_base64_by_path(&source).ok();
    cache.insert(key, icon.clone());
    icon
}

pub fn list_startup_items() -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();
    let mut icon_cache: HashMap<String, Option<String>> = HashMap::new();

    // 1. Scan Registry Run keys
    let run_locations = vec![
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        ("HKCU", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"),
        ("HKCU", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
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
                let icon = startup_icon_base64(
                    startup_icon_source(&command_str, None),
                    &mut icon_cache,
                );
                let target_missing = startup_target_missing(&command_str);
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
                    icon_base64: icon,
                    target_missing,
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
                let icon = startup_icon_base64(
                    startup_icon_source(&command_str, None),
                    &mut icon_cache,
                );
                let target_missing = startup_target_missing(&command_str);
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
                    icon_base64: icon,
                    target_missing,
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
                        
                        let startup_icon = startup_icon_base64(Some(path.clone()), &mut icon_cache);
                        items.push(StartupItem {
                            id,
                            name: display_name,
                            command: path.to_string_lossy().to_string(),
                            location: loc_name.to_string(),
                            enabled,
                            is_registry: false,
                            registry_path: None,
                            file_path: Some(path.to_string_lossy().to_string()),
                            icon_base64: startup_icon,
                            target_missing: false,
                        });
                    }
                }
            }
        }
    }

    // 3. Scan Scheduled Tasks XML files
    let windir = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    if tasks_dir.exists() {
        scan_tasks_directory(&tasks_dir, &mut items, &mut icon_cache);
    }

    Ok(items)
}

pub fn set_startup_item_status(id: String, enabled: bool) -> Result<(), String> {
    let (item_type, rest) = id
        .split_once('|')
        .ok_or_else(|| "Invalid startup ID".to_string())?;

    if item_type == "reg" {
        let (hive_name, subpath, value_name) = parse_registry_startup_id(&id)?;

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
            
            disabled_key
                .delete_value(value_name)
                .map_err(|e| format!("Failed to remove the enabled Run value: {}", e))?;
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
            
            parent_key
                .delete_value(value_name)
                .map_err(|e| format!("Failed to remove the enabled Run value: {}", e))?;
        }
    } else if item_type == "file" {
        let file_path_str = rest;
        if !is_in_startup_dir(file_path_str) {
            return Err("Access Denied: Startup file must reside inside a Windows Startup folder.".to_string());
        }
        let path = Path::new(file_path_str);
        if !path.exists() {
            return Err("Startup file does not exist".to_string());
        }
        if !path.is_file() {
            return Err("Startup path is not a file".to_string());
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
        let file_path_str = rest;
        if !is_task_file_path(file_path_str) {
            return Err("Access Denied: Scheduled task must be a file inside the Windows Tasks folder.".to_string());
        }
        if !crate::settings::check_is_admin() {
            return Err("Modifying scheduled tasks requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let path = Path::new(file_path_str);
        let task_name = get_task_name_from_path(path)
            .ok_or_else(|| "Failed to resolve scheduled task name".to_string())?;
        if is_protected_task_name(&task_name) {
            return Err("Access Denied: Windows servicing tasks under \\Microsoft\\Windows are protected.".to_string());
        }

        let change_arg = if enabled { "/Enable" } else { "/Disable" };
        let secure_path = crate::winutil::get_secure_system_path();
        let output = std::process::Command::new("schtasks")
            .creation_flags(CREATE_NO_WINDOW)
            .env("PATH", &secure_path)
            .args(&["/Change", "/TN", &task_name, change_arg])
            .output()
            .map_err(|e| format!("Failed to execute schtasks: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("schtasks failed: {}", stderr.trim()));
        }
    } else {
        return Err("Unsupported startup type".to_string());
    }

    Ok(())
}

pub fn delete_startup_item(id: String) -> Result<(), String> {
    let (item_type, rest) = id
        .split_once('|')
        .ok_or_else(|| "Invalid startup ID".to_string())?;

    if item_type == "reg" {
        let (hive_name, subpath, value_name) = parse_registry_startup_id(&id)?;

        if hive_name == "HKLM" && !crate::settings::check_is_admin() {
            return Err("Deleting system startup items (HKLM) requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let hive = get_hive_from_name(hive_name)
            .ok_or_else(|| "Invalid registry hive".to_string())?;
        let root = RegKey::predef(hive);

        let disabled_subpath = format!(r"{}\PurgeKit_Disabled", subpath);
        let mut found = false;
        for key_path in [subpath, disabled_subpath.as_str()] {
            if let Ok(key) = root.open_subkey_with_flags(key_path, KEY_WRITE) {
                match key.delete_value(value_name) {
                    Ok(_) => found = true,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(format!("Failed to delete startup registry value: {}", e));
                    }
                }
            }
        }

        if !found {
            return Err("Startup registry value was not found or access was denied.".to_string());
        }
    } else if item_type == "file" {
        let file_path_str = rest;
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
        if !path.is_file() {
            return Err("Startup path is not a file".to_string());
        }

        let is_system = file_path_str.to_uppercase().contains("PROGRAMDATA");
        if is_system && !crate::settings::check_is_admin() {
            return Err("Deleting system startup items requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        fs::remove_file(path)
            .map_err(|e| format!("Failed to delete startup file: {}", e))?;
    } else if item_type == "task" {
        let file_path_str = rest;
        if !is_task_file_path(file_path_str) {
            return Err("Access Denied: Scheduled task must be a file inside the Windows Tasks folder.".to_string());
        }
        if !crate::settings::check_is_admin() {
            return Err("Deleting scheduled tasks requires Administrator privileges. Please restart the application as Administrator.".to_string());
        }

        let path = Path::new(file_path_str);
        let task_name = get_task_name_from_path(path)
            .ok_or_else(|| "Failed to resolve scheduled task name".to_string())?;
        if is_protected_task_name(&task_name) {
            return Err("Access Denied: Windows servicing tasks under \\Microsoft\\Windows are protected.".to_string());
        }

        let secure_path = crate::winutil::get_secure_system_path();
        let output = std::process::Command::new("schtasks")
            .creation_flags(CREATE_NO_WINDOW)
            .env("PATH", &secure_path)
            .args(&["/Delete", "/TN", &task_name, "/F"])
            .output()
            .map_err(|e| format!("Failed to execute schtasks delete: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("schtasks delete failed: {}", stderr.trim()));
        }
    } else {
        return Err("Unsupported startup type".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_registry_startup_id_allowlist() {
        let ok = parse_registry_startup_id(
            r"reg|HKCU|SOFTWARE\Microsoft\Windows\CurrentVersion\Run|MyApp",
        )
        .unwrap();
        assert_eq!(ok.0, "HKCU");
        assert_eq!(ok.2, "MyApp");

        let with_pipe = parse_registry_startup_id(
            r"reg|HKCU|SOFTWARE\Microsoft\Windows\CurrentVersion\Run|My|App",
        )
        .unwrap();
        assert_eq!(with_pipe.2, "My|App");

        assert!(parse_registry_startup_id(r"reg|HKCU|Environment|Path").is_err());
        assert!(parse_registry_startup_id(r"reg|HKLM|SOFTWARE\Microsoft\Windows\CurrentVersion\Run|").is_err());
    }

    #[test]
    fn test_startup_target_missing_only_checks_absolute_paths() {
        assert!(startup_target_missing(
            r"C:\PurgeKitMissingTarget\missing.exe --background"
        ));
        assert!(!startup_target_missing(r"C:\Windows\System32\cmd.exe"));
        assert!(!startup_target_missing("cmd.exe /c echo hello"));
    }

    #[test]
    fn test_windows_servicing_tasks_are_protected() {
        assert!(is_protected_task_name(r"\Microsoft\Windows\Data Integrity Scan"));
        assert!(is_protected_task_name(r"\Microsoft\Windows\AppxDeploymentClient\Pre-staged app cleanup"));
        assert!(!is_protected_task_name(r"\Vendor\My App Updater"));
        assert!(!is_protected_task_name("My App"));
    }

    #[test]
    fn test_startup_id_rejects_environment_path() {
        let result = set_startup_item_status("reg|HKCU|Environment|Path".to_string(), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Access Denied"));
    }

    #[test]
    fn test_is_in_startup_dir() {
        let appdata = std::env::var("APPDATA").ok().map(PathBuf::from);
        if let Some(dir) = appdata {
            let startup = dir.join(r"Microsoft\Windows\Start Menu\Programs\Startup");
            let file = startup.join("test.exe");
            assert!(is_in_startup_dir(&file.to_string_lossy()));
            
            let sub_file = startup.join("nested").join("test.exe");
            assert!(is_in_startup_dir(&sub_file.to_string_lossy()));
            assert!(!is_in_startup_dir(&startup.to_string_lossy()));
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
        let windir = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("windir"))
            .unwrap_or_else(|_| "C:\\Windows".to_string());
        let tasks = Path::new(&windir).join("System32").join("Tasks");
        let file = tasks.join("GoogleUpdateTaskMachineUA");
        assert!(is_in_tasks_dir(&file.to_string_lossy()));
        
        let sub_file = tasks.join("Microsoft").join("Windows").join("WindowsUpdate");
        assert!(is_in_tasks_dir(&sub_file.to_string_lossy()));
        assert!(!is_in_tasks_dir(&tasks.to_string_lossy()));

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
