use serde::{Deserialize, Serialize};
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

    // 1. Read System PATH first (canonical machine-wide order)
    let system_subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    if let Ok(sys_path) = read_path_from_registry(HKEY_LOCAL_MACHINE, system_subpath) {
        for p in sys_path.split(';') {
            let p_trim = p.trim().to_string();
            if !p_trim.is_empty() {
                raw_entries.push((p_trim, "System".to_string()));
            }
        }
    }

    // 2. Read User PATH second
    if let Ok(user_path) = read_path_from_registry(HKEY_CURRENT_USER, "Environment") {
        for p in user_path.split(';') {
            let p_trim = p.trim().to_string();
            if !p_trim.is_empty() {
                raw_entries.push((p_trim, "User".to_string()));
            }
        }
    }

    // Validate entries
    let results = validate_path_entries(raw_entries);
    Ok(results)
}

pub fn set_path_entries(remaining_values: Vec<String>, scope: &str) -> Result<(), String> {
    if scope == "System" && remaining_values.is_empty() {
        return Err("System PATH cannot be empty. Deleting all entries would cause Windows system instability.".to_string());
    }

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

fn validate_path_entries(raw_entries: Vec<(String, String)>) -> Vec<PathEntry> {
    let mut results = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (idx, (raw, scope)) in raw_entries.into_iter().enumerate() {
        let trimmed_raw = raw.trim();
        let unquoted = if trimmed_raw.starts_with('"') && trimmed_raw.ends_with('"') && trimmed_raw.len() >= 2 {
            &trimmed_raw[1..trimmed_raw.len() - 1]
        } else {
            trimmed_raw
        };

        let expanded = crate::winutil::expand_env_strings(unquoted);
        let clean_path = expanded.trim().trim_matches('"');
        let normalized = clean_path.to_lowercase().trim_end_matches('\\').to_string();
        let exists = Path::new(clean_path).is_dir();

        let dead = !exists;
        
        let (is_dup, dup_of) = if let Some(&first) = seen.get(&normalized) {
            (true, Some(first))
        } else {
            seen.insert(normalized.clone(), idx);
            (false, None)
        };

        let is_overlap = false;

        let issue = if dead {
            "Directory does not exist".to_string()
        } else if is_dup {
            format!("Duplicate of line #{}", dup_of.unwrap() + 1)
        } else {
            "Valid".to_string()
        };

        results.push(PathEntry {
            value: raw,
            expanded: clean_path.to_string(),
            is_valid: exists,
            is_duplicate: is_dup,
            is_overlap,
            scope,
            issue_reason: issue,
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_entries() {
        let temp_dir = std::env::temp_dir().join("purgekit_test_path_cleaner");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let sub_dir = temp_dir.join("sub");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let quoted_sub = format!("\"{}\"", sub_dir.to_string_lossy());

        let raw_entries = vec![
            (sub_dir.to_string_lossy().to_string(), "System".to_string()),
            (quoted_sub, "User".to_string()), // duplicate with quotes
            (temp_dir.join("non_existent").to_string_lossy().to_string(), "User".to_string()), // dead
        ];

        let results = validate_path_entries(raw_entries);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert_eq!(results.len(), 3);

        // First is valid (System)
        assert!(results[0].is_valid);
        assert!(!results[0].is_duplicate);
        assert_eq!(results[0].issue_reason, "Valid");
        assert_eq!(results[0].scope, "System");

        // Second is valid folder despite quotes, but duplicate of line 1
        assert!(results[1].is_valid);
        assert!(results[1].is_duplicate);
        assert!(results[1].issue_reason.contains("Duplicate of line #1"));
        assert_eq!(results[1].scope, "User");

        // Third is dead
        assert!(!results[2].is_valid);
        assert!(!results[2].is_duplicate);
        assert_eq!(results[2].issue_reason, "Directory does not exist");
    }
}

