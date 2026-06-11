use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathEntry {
    pub value: String,
    pub expanded: String,
    pub is_valid: bool,
    pub is_duplicate: bool,
    pub is_overlap: bool,
    pub scope: String, // "User" or "System"
    pub issue_reason: String,
}

pub fn get_path_entries() -> Result<Vec<PathEntry>, String> {
    let mut raw_entries = Vec::new();

    // 1. Read User PATH from HKCU\Environment
    if let Ok(user_path) = read_path_from_registry(HKEY_CURRENT_USER, "Environment") {
        for p in user_path.split(';') {
            let p_trim = p.trim().to_string();
            if !p_trim.is_empty() {
                raw_entries.push((p_trim, "User".to_string()));
            }
        }
    }

    // 2. Read System PATH
    let system_subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    if let Ok(sys_path) = read_path_from_registry(HKEY_LOCAL_MACHINE, system_subpath) {
        for p in sys_path.split(';') {
            let p_trim = p.trim().to_string();
            if !p_trim.is_empty() {
                raw_entries.push((p_trim, "System".to_string()));
            }
        }
    }

    // Validate entries
    let mut results = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // Prepare overlap detection
    let mut sorted: Vec<(usize, String)> = raw_entries.iter().enumerate()
        .map(|(i, (p, _))| {
            let expanded = crate::winutil::expand_env_strings(p);
            (i, expanded.to_lowercase().trim_end_matches('\\').to_string())
        })
        .collect();
    // Sort by path length to easily find prefix relationships
    sorted.sort_by(|a, b| a.1.len().cmp(&b.1.len()));

    let mut overlaps = std::collections::HashSet::new();
    for i in 0..sorted.len() {
        for j in i+1..sorted.len() {
            let parent = &sorted[i].1;
            let child = &sorted[j].1;
            if !parent.is_empty() && !child.is_empty() && (child.starts_with(&format!("{}\\", parent)) || child == parent) {
                if sorted[j].0 != sorted[i].0 {
                    overlaps.insert(sorted[j].0);
                }
            }
        }
    }

    for (idx, (raw, scope)) in raw_entries.into_iter().enumerate() {
        let expanded = crate::winutil::expand_env_strings(&raw);
        let normalized = expanded.to_lowercase().trim_end_matches('\\').to_string();
        let exists = Path::new(&expanded).is_dir();

        let dead = !exists;
        
        let (is_dup, dup_of) = if let Some(&first) = seen.get(&normalized) {
            (true, Some(first))
        } else {
            seen.insert(normalized.clone(), idx);
            (false, None)
        };

        let is_overlap = overlaps.contains(&idx);

        let issue = if dead {
            "Thư mục không tồn tại".to_string()
        } else if is_dup {
            format!("Trùng lặp với dòng thứ #{}", dup_of.unwrap() + 1)
        } else if is_overlap {
            "Thư mục con bị lặp thừa của một mục khác trong PATH".to_string()
        } else {
            "Hợp lệ".to_string()
        };

        results.push(PathEntry {
            value: raw,
            expanded,
            is_valid: exists,
            is_duplicate: is_dup,
            is_overlap,
            scope,
            issue_reason: issue,
        });
    }

    Ok(results)
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

    Ok(())
}

fn read_path_from_registry(hkey: winreg::HKEY, subpath: &str) -> Result<String, String> {
    let root = RegKey::predef(hkey);
    let key = root.open_subkey_with_flags(subpath, KEY_READ)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    
    let val: String = key.get_value("PATH")
        .or_else(|_| key.get_value("Path"))
        .map_err(|e| format!("Failed to read PATH value: {}", e))?;
    
    Ok(val)
}

fn write_path_to_registry(hkey: winreg::HKEY, subpath: &str, value: &str) -> Result<(), String> {
    let root = RegKey::predef(hkey);
    let key = root.open_subkey_with_flags(subpath, KEY_WRITE)
        .map_err(|e| format!("Access Denied (Requires Admin elevation?): {}", e))?;

    // PATH must stay REG_EXPAND_SZ: set_value writes REG_SZ, which silently
    // breaks entries containing environment variables like %SystemRoot%.
    let mut bytes: Vec<u8> = Vec::with_capacity((value.len() + 1) * 2);
    for unit in value.encode_utf16().chain(std::iter::once(0u16)) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let reg_value = winreg::RegValue { bytes: bytes.into(), vtype: REG_EXPAND_SZ };

    key.set_raw_value("Path", &reg_value)
        .map_err(|e| format!("Failed to write Registry key: {}", e))?;

    Ok(())
}

