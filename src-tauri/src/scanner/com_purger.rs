use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;
use crate::scanner::RemnantItem;

pub fn scan_com_orphans(app_token: &str, install_dir: Option<&str>) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();
    let app_token_lower = app_token.to_lowercase();
    
    // Hives and paths to scan CLSID
    let clsid_locations = vec![
        (HKEY_CURRENT_USER, "HKCU", r"Software\Classes\CLSID"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Classes\CLSID"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Classes\Wow6432Node\CLSID"),
    ];

    for (hive, hive_name, subpath) in clsid_locations {
        let root = RegKey::predef(hive);
        let clsid_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for guid in clsid_key.enum_keys().filter_map(|x| x.ok()) {
            let guid_path = format!(r"{}\{}", subpath, guid);
            let guid_key = match clsid_key.open_subkey_with_flags(&guid, KEY_READ) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // Check InprocServer32 (DLL) and LocalServer32 (EXE)
            for server_type in &["InprocServer32", "LocalServer32"] {
                if let Ok(server_key) = guid_key.open_subkey_with_flags(server_type, KEY_READ) {
                    let raw_path: String = match server_key.get_value("") {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    let cleaned_path = raw_path.trim().trim_matches('"').to_string();
                    
                    // Skip Darwin descriptor strings (used by MSI installer, starts with a bracket/special char)
                    if cleaned_path.starts_with('>') || cleaned_path.starts_with('<') || cleaned_path.starts_with('[') {
                        continue;
                    }

                    let expanded = crate::winutil::expand_env_strings(&cleaned_path);
                    if expanded.is_empty() {
                        continue;
                    }

                    let path_lower = expanded.to_lowercase();
                    let file_exists = Path::new(&expanded).exists();

                    let mut is_match = false;
                    let mut score = 50;

                    // Match Strategy A: path is inside the uninstalled app install location
                    if let Some(install_loc) = install_dir {
                        let install_lower = install_loc.to_lowercase();
                        if !install_lower.is_empty() && path_lower.starts_with(&install_lower) {
                            is_match = true;
                            score = 95; // VeryHigh COM Orphan
                        }
                    }

                    // Match Strategy B: binary file is missing AND path contains app_token
                    if !is_match && !file_exists && path_lower.contains(&app_token_lower) {
                        is_match = true;
                        score = 75; // High COM Orphan
                    }

                    if is_match {
                        remnants.push(RemnantItem {
                            path: format!(r"{}\{}", hive_name, guid_path),
                            item_type: "RegistryKey".to_string(),
                            size: 0,
                            confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                            score,
                        });
                        break; // Found matching server type, no need to check other server types for this GUID
                    }
                }
            }
        }
    }

    remnants
}

pub fn scan_typelib_orphans(app_token: &str) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();
    let app_token_lower = app_token.to_lowercase();

    let typelib_locations = vec![
        (HKEY_CURRENT_USER, "HKCU", r"Software\Classes\TypeLib"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Classes\TypeLib"),
    ];

    for (hive, hive_name, subpath) in typelib_locations {
        let root = RegKey::predef(hive);
        let typelib_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for guid in typelib_key.enum_keys().filter_map(|x| x.ok()) {
            let guid_path = format!(r"{}\{}", subpath, guid);
            let guid_key = match typelib_key.open_subkey_with_flags(&guid, KEY_READ) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // TypeLib structure: TypeLib\{GUID}\{Version}\{LCID}\win32 or win64
            for version in guid_key.enum_keys().filter_map(|x| x.ok()) {
                if let Ok(ver_key) = guid_key.open_subkey_with_flags(&version, KEY_READ) {
                    for lcid in ver_key.enum_keys().filter_map(|x| x.ok()) {
                        if let Ok(lcid_key) = ver_key.open_subkey_with_flags(&lcid, KEY_READ) {
                            for arch in &["win32", "win64"] {
                                if let Ok(arch_key) = lcid_key.open_subkey_with_flags(arch, KEY_READ) {
                                    let raw_path: String = match arch_key.get_value("") {
                                        Ok(p) => p,
                                        Err(_) => continue,
                                    };

                                    let cleaned_path = raw_path.trim().trim_matches('"');
                                    let expanded = crate::winutil::expand_env_strings(cleaned_path);
                                    if expanded.is_empty() {
                                        continue;
                                    }

                                    let file_exists = Path::new(&expanded).exists();
                                    if !file_exists && expanded.to_lowercase().contains(&app_token_lower) {
                                        remnants.push(RemnantItem {
                                            path: format!(r"{}\{}", hive_name, guid_path),
                                            item_type: "RegistryKey".to_string(),
                                            size: 0,
                                            confidence: "High".to_string(),
                                            score: 70,
                                        });
                                        break; // break architecture check for this LCID
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    remnants
}

