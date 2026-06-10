use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathEntry {
    pub value: String,
    pub is_valid: bool,
    pub scope: String, // "User" or "System"
}

pub fn get_path_entries() -> Result<Vec<PathEntry>, String> {
    let mut entries = Vec::new();

    // 1. Read User PATH from HKCU\Environment
    if let Ok(user_path) = read_path_from_registry(HKEY_CURRENT_USER, "Environment") {
        for p in user_path.split(';') {
            let p_trim = p.trim();
            if !p_trim.is_empty() {
                // Expand environment variables like %USERPROFILE% for validation
                let expanded = expand_env_vars(p_trim);
                let is_valid = Path::new(&expanded).exists();
                entries.push(PathEntry {
                    value: p_trim.to_string(),
                    is_valid,
                    scope: "User".to_string(),
                });
            }
        }
    }

    // 2. Read System PATH from HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
    let system_subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    if let Ok(sys_path) = read_path_from_registry(HKEY_LOCAL_MACHINE, system_subpath) {
        for p in sys_path.split(';') {
            let p_trim = p.trim();
            if !p_trim.is_empty() {
                let expanded = expand_env_vars(p_trim);
                let is_valid = Path::new(&expanded).exists();
                entries.push(PathEntry {
                    value: p_trim.to_string(),
                    is_valid,
                    scope: "System".to_string(),
                });
            }
        }
    }

    Ok(entries)
}

pub fn set_path_entries(remaining_values: Vec<String>, scope: &str) -> Result<(), String> {
    let new_path_val = remaining_values.join(";");

    if scope == "User" {
        write_path_to_registry(HKEY_CURRENT_USER, "Environment", &new_path_val)?;
    } else if scope == "System" {
        let system_subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
        write_path_to_registry(HKEY_LOCAL_MACHINE, system_subpath, &new_path_val)?;
    } else {
        return Err(format!("Unknown PATH scope: {}", scope));
    }

    // Broadcast change to OS so terminal sessions can update (WM_SETTINGCHANGE)
    // For now we just return success, broadcast can be implemented using Win32 API
    Ok(())
}

fn read_path_from_registry(hkey: winreg::HKEY, subpath: &str) -> Result<String, String> {
    let root = RegKey::predef(hkey);
    let key = root.open_subkey_with_flags(subpath, KEY_READ)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    
    // PATH is usually REG_EXPAND_SZ
    let val: String = key.get_value("PATH")
        .or_else(|_| key.get_value("Path"))
        .map_err(|e| format!("Failed to read PATH value: {}", e))?;
    
    Ok(val)
}

fn write_path_to_registry(hkey: winreg::HKEY, subpath: &str, value: &str) -> Result<(), String> {
    let root = RegKey::predef(hkey);
    // Requires KEY_WRITE, HKLM requires elevation
    let key = root.open_subkey_with_flags(subpath, KEY_WRITE)
        .map_err(|e| format!("Access Denied (Requires Admin elevation?): {}", e))?;

    // Set value as REG_EXPAND_SZ so env variables like %SystemRoot% remain expand-ready
    key.set_value("Path", &value)
        .map_err(|e| format!("Failed to write Registry key: {}", e))?;

    Ok(())
}

fn expand_env_vars(raw_path: &str) -> String {
    // Basic expander for windows environment variables like %USERPROFILE% or %SystemRoot%
    let mut expanded = raw_path.to_string();
    let mut start = 0;

    while let Some(pos_start) = expanded[start..].find('%') {
        let actual_start = start + pos_start;
        if let Some(pos_end) = expanded[actual_start + 1..].find('%') {
            let actual_end = actual_start + 1 + pos_end;
            let var_name = &expanded[actual_start + 1..actual_end];
            if let Ok(var_val) = env::var(var_name) {
                expanded.replace_range(actual_start..=actual_end, &var_val);
                start = actual_start + var_val.len();
            } else {
                start = actual_end + 1;
            }
        } else {
            break;
        }
    }
    expanded
}
