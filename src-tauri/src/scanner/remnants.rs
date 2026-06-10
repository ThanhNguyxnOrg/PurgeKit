use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;
use rayon::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemnantItem {
    pub path: String,
    pub item_type: String, // "File", "Directory", "RegistryKey"
    pub size: u64,
}

pub fn scan_app_remnants(
    app_name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();
    let app_name_lower = app_name.to_lowercase();
    
    // Clean app name for matching (e.g. remove "version 1.0", "uninstaller", etc.)
    let match_token = clean_match_token(&app_name_lower);
    if match_token.len() < 3 {
        return remnants; // Avoid matching too short strings like "a", "py", which could delete system files
    }

    // 1. Scan File System Remnants
    let mut dirs_to_scan = Vec::new();
    if let Some(appdata) = env::var_os("APPDATA").map(PathBuf::from) {
        dirs_to_scan.push((appdata, "Directory"));
    }
    if let Some(localappdata) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs_to_scan.push((localappdata, "Directory"));
    }
    if let Some(programdata) = env::var_os("ProgramData").map(PathBuf::from) {
        dirs_to_scan.push((programdata, "Directory"));
    }
    
    // Program Files directories
    if let Some(pf) = env::var_os("ProgramFiles").map(PathBuf::from) {
        dirs_to_scan.push((pf, "Directory"));
    }
    if let Some(pf86) = env::var_os("ProgramFiles(x86)").map(PathBuf::from) {
        dirs_to_scan.push((pf86, "Directory"));
    }
    
    // User Documents
    if let Some(userprofile) = env::var_os("USERPROFILE").map(PathBuf::from) {
        dirs_to_scan.push((userprofile.join("Documents"), "Directory"));
    }

    // If install location is known and exists, we should scan it
    if let Some(loc) = install_location {
        let loc_path = Path::new(loc);
        if loc_path.exists() {
            // Include files/directories directly inside the install location
            scan_directory_remnants(loc_path, &match_token, &mut remnants);
        }
    }

    // Scan general AppData and Program Files folders for subdirectories matching app name in parallel
    let mut fs_remnants: Vec<RemnantItem> = dirs_to_scan.into_par_iter()
        .filter(|(dir, _)| dir.exists())
        .flat_map(|(dir, _)| {
            let mut local_remnants = Vec::new();
            scan_directory_remnants(&dir, &match_token, &mut local_remnants);
            local_remnants
        })
        .collect();
    remnants.append(&mut fs_remnants);

    // 2. Scan Registry Remnants
    // Search in HKEY_CURRENT_USER\Software and HKEY_LOCAL_MACHINE\Software
    scan_registry_remnants_in_hive(HKEY_CURRENT_USER, "HKCU", &match_token, publisher, &mut remnants);
    scan_registry_remnants_in_hive(HKEY_LOCAL_MACHINE, "HKLM", &match_token, publisher, &mut remnants);
    scan_registry_remnants_in_hive(HKEY_LOCAL_MACHINE, "HKLM\\Wow6432Node", &match_token, publisher, &mut remnants);

    remnants
}

fn clean_match_token(app_name: &str) -> String {
    let mut token = app_name.to_string();
    // Remove common words
    let remove_words = vec!["uninstaller", "installer", "setup", "software", "client", "x64", "x86", "32bit", "64bit", "free", "pro", "ultimate"];
    for word in remove_words {
        token = token.replace(word, "");
    }
    
    // Keep only alphanumeric and spaces
    token = token.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_').collect();
    token.trim().to_lowercase()
}

fn scan_directory_remnants(parent_dir: &Path, token: &str, remnants: &mut Vec<RemnantItem>) {
    let entries = match fs::read_dir(parent_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_lowercase(),
            None => continue,
        };

        // If the folder name contains our clean app token, it's highly likely to be a remnant
        if name.contains(token) {
            let item_type = if path.is_dir() { "Directory" } else { "File" };
            let size = if path.is_dir() {
                super::cli_dev::calculate_dir_size(&path)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };

            remnants.push(RemnantItem {
                path: path.to_string_lossy().to_string(),
                item_type: item_type.to_string(),
                size,
            });
        }
    }
}

fn scan_registry_remnants_in_hive(
    hkey: winreg::HKEY,
    hive_name: &str,
    token: &str,
    publisher: Option<&str>,
    remnants: &mut Vec<RemnantItem>,
) {
    let root = RegKey::predef(hkey);
    let software_path = if hive_name.contains("Wow6432Node") {
        r"SOFTWARE\Wow6432Node"
    } else {
        r"SOFTWARE"
    };

    let software_key = match root.open_subkey_with_flags(software_path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    // 1. Search directly in SOFTWARE\<AppName>
    for name in software_key.enum_keys().filter_map(|x| x.ok()) {
        let name_lower = name.to_lowercase();
        if name_lower.contains(token) {
            remnants.push(RemnantItem {
                path: format!(r"{}\{}\{}", hive_name, software_path, name),
                item_type: "RegistryKey".to_string(),
                size: 0,
            });
        }
    }

    // 2. If publisher is provided, check SOFTWARE\<Publisher>\<AppName>
    if let Some(pub_name) = publisher {
        let pub_clean = clean_match_token(&pub_name.to_lowercase());
        if pub_clean.len() >= 3 {
            for name in software_key.enum_keys().filter_map(|x| x.ok()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains(&pub_clean) {
                    // Open publisher subkey and search apps inside
                    let pub_path = format!(r"{}\{}", software_path, name);
                    if let Ok(pub_key) = root.open_subkey_with_flags(&pub_path, KEY_READ) {
                        for app_name in pub_key.enum_keys().filter_map(|x| x.ok()) {
                            let app_lower = app_name.to_lowercase();
                            if app_lower.contains(token) {
                                remnants.push(RemnantItem {
                                    path: format!(r"{}\{}\{}", hive_name, pub_path, app_name),
                                    item_type: "RegistryKey".to_string(),
                                    size: 0,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn purge_remnant_item(item: &RemnantItem) -> Result<(), String> {
    match item.item_type.as_str() {
        "File" => {
            let path = Path::new(&item.path);
            if path.exists() {
                fs::remove_file(path).map_err(|e| format!("Failed to remove file: {}", e))?;
            }
        }
        "Directory" => {
            let path = Path::new(&item.path);
            if path.exists() {
                fs::remove_dir_all(path).map_err(|e| format!("Failed to remove directory: {}", e))?;
            }
        }
        "RegistryKey" => {
            // Path structure: "HKCU\SOFTWARE\AppName" or "HKLM\SOFTWARE\Wow6432Node\SOFTWARE\AppName"
            // Let's normalize HKLM\Wow6432Node representation
            let (hive_name, path_str) = {
                let path_str_clone = item.path.clone();
                if path_str_clone.starts_with("HKLM\\Wow6432Node") {
                    ("HKLM_WOW6432".to_string(), path_str_clone.replacen("HKLM\\Wow6432Node\\", "", 1))
                } else {
                    let parts: Vec<&str> = path_str_clone.splitn(2, '\\').collect();
                    if parts.len() < 2 {
                        return Err("Invalid registry path".to_string());
                    }
                    (parts[0].to_string(), parts[1].to_string())
                }
            };

            let root = match hive_name.as_str() {
                "HKCU" => RegKey::predef(HKEY_CURRENT_USER),
                "HKLM" => RegKey::predef(HKEY_LOCAL_MACHINE),
                "HKLM_WOW6432" => RegKey::predef(HKEY_LOCAL_MACHINE), // We already removed the Wow6432Node prefix to normalize path_str
                _ => return Err(format!("Unsupported hive: {}", hive_name)),
            };

            let normalized_path = if hive_name == "HKLM_WOW6432" {
                format!(r"SOFTWARE\Wow6432Node\{}", path_str)
            } else {
                path_str
            };

            let last_backslash = normalized_path.rfind('\\')
                .ok_or_else(|| format!("Invalid registry path: {}", normalized_path))?;
            let parent_path = &normalized_path[..last_backslash];
            let key_to_delete = &normalized_path[last_backslash + 1..];

            let parent_key = root.open_subkey_with_flags(parent_path, KEY_WRITE)
                .map_err(|e| format!("Failed to open registry parent key: {}", e))?;

            parent_key.delete_subkey(key_to_delete)
                .map_err(|e| format!("Failed to delete registry key: {}", e))?;
        }
        _ => return Err(format!("Unknown item type: {}", item.item_type)),
    }
    Ok(())
}

pub fn purge_all_remnants(items: &[RemnantItem]) -> (u32, u32) {
    let mut success_count = 0;
    let mut fail_count = 0;
    for item in items {
        match purge_remnant_item(item) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }
    }
    (success_count, fail_count)
}
