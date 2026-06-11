use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemnantItem {
    pub path: String,
    pub item_type: String, // "File", "Directory", "RegistryKey"
    pub size: u64,
    pub confidence: String, // "VeryHigh" | "High" | "Medium" | "Low"
    pub score: i32,
}

#[derive(Debug, Clone)]
enum TargetType {
    Directory(PathBuf),
    RegistryKey { hive: winreg::HKEY, hive_name: &'static str, path: String },
}

struct ScanTarget {
    target: TargetType,
    base_score: i32,
}

fn dir_target(path: PathBuf, base_score: i32) -> ScanTarget {
    ScanTarget {
        target: TargetType::Directory(path),
        base_score,
    }
}

fn reg_target(hive: winreg::HKEY, hive_name: &'static str, path: &str, base_score: i32) -> ScanTarget {
    ScanTarget {
        target: TargetType::RegistryKey {
            hive,
            hive_name,
            path: path.to_string(),
        },
        base_score,
    }
}

fn get_scan_locations(level: &str) -> Vec<ScanTarget> {
    let mut targets = Vec::new();
    let unsub = r"Microsoft\Windows\CurrentVersion\Uninstall";

    // ═══ TIER 1: Primary Install Dirs (base_score: 80) ═══
    if let Some(pf) = env::var_os("ProgramFiles").map(PathBuf::from) {
        targets.push(dir_target(pf, 80));
    }
    if let Some(pf86) = env::var_os("ProgramFiles(x86)").map(PathBuf::from) {
        targets.push(dir_target(pf86, 80));
    }

    // ═══ TIER 2: User AppData & ProgramData (base_score: 60) ═══
    if let Some(appdata) = env::var_os("APPDATA").map(PathBuf::from) {
        targets.push(dir_target(appdata, 60));
        targets.push(dir_target(env::var_os("APPDATA").map(PathBuf::from).unwrap().join(r"Microsoft\Windows\Start Menu\Programs"), 65));
    }
    if let Some(localappdata) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        targets.push(dir_target(localappdata.clone(), 60));
        targets.push(dir_target(localappdata.join("Programs"), 60));
    }
    if let Some(programdata) = env::var_os("ProgramData").map(PathBuf::from) {
        targets.push(dir_target(programdata.clone(), 55));
        targets.push(dir_target(programdata.join(r"Microsoft\Windows\Start Menu\Programs"), 65));
    }

    // ═══ TIER 3: User Profile (base_score: 40) ═══
    if let Some(userprofile) = env::var_os("USERPROFILE").map(PathBuf::from) {
        targets.push(dir_target(userprofile.clone(), 40));
        targets.push(dir_target(userprofile.join(".config"), 45));
        targets.push(dir_target(userprofile.join("Documents"), 35));
        targets.push(dir_target(userprofile.join("Saved Games"), 40));
        targets.push(dir_target(userprofile.join("Desktop"), 60));
    }
    if let Some(public_desktop) = env::var_os("PUBLIC").map(PathBuf::from) {
        targets.push(dir_target(public_desktop.join("Desktop"), 55));
    }

    // ═══ TIER 6: Registry SOFTWARE (base_score: 50) ═══
    targets.push(reg_target(HKEY_CURRENT_USER, "HKCU", "SOFTWARE", 50));
    targets.push(reg_target(HKEY_LOCAL_MACHINE, "HKLM", "SOFTWARE", 50));
    targets.push(reg_target(HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node", 50));

    // Uninstall keys (ghost entries)
    // NOTE: use real hive names so purge_remnant_item and `reg export` backups
    // can resolve these paths (HKLM_UNINSTALL/HKCU_UNINSTALL were unsupported
    // hives, so purging ghost uninstall entries always failed).
    targets.push(reg_target(HKEY_LOCAL_MACHINE, "HKLM", &format!(r"SOFTWARE\{}", unsub), 70));
    targets.push(reg_target(HKEY_CURRENT_USER, "HKCU", &format!(r"SOFTWARE\{}", unsub), 70));

    // ═══ TIER 7: VirtualStore remnants (base_score: 65) ═══
    targets.push(reg_target(HKEY_CURRENT_USER, "HKCU", r"Software\Classes\VirtualStore\MACHINE\SOFTWARE", 65));
    targets.push(reg_target(HKEY_CURRENT_USER, "HKCU", r"Software\Classes\VirtualStore\MACHINE\SOFTWARE\Wow6432Node", 65));

    if level != "safe" {
        // TIER 4: Temp & Cache
        if let Some(temp) = env::var_os("TEMP").map(PathBuf::from) {
            targets.push(dir_target(temp, 35));
        }
        targets.push(dir_target(PathBuf::from(r"C:\Windows\Temp"), 30));
        if let Some(localappdata) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            targets.push(dir_target(localappdata.join("CrashDumps"), 40));
        }
    }

    targets
}

pub fn scan_app_remnants(
    app_name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();
    let app_name_lower = app_name.to_lowercase();
    
    let match_token = clean_match_token(&app_name_lower);
    if match_token.len() < 3 {
        return remnants; // Avoid matching too short strings
    }

    let settings = crate::settings::load_settings();
    let scan_targets = get_scan_locations(&settings.scan_level);

    // If install location is known and exists, scan it directly
    if let Some(loc) = install_location {
        let loc_path = Path::new(loc);
        if loc_path.exists() {
            scan_directory_remnants_recursive(loc_path, &match_token, 80, &mut remnants);
        }
    }

    // Process targets in standard iterator (parallel not possible due to raw HKEY pointer)
    let mut fs_remnants: Vec<RemnantItem> = scan_targets.into_iter()
        .flat_map(|target| {
            let mut local_remnants = Vec::new();
            match target.target {
                TargetType::Directory(ref dir) => {
                    if dir.exists() {
                        scan_directory_remnants_recursive(dir, &match_token, target.base_score, &mut local_remnants);
                    }
                }
                TargetType::RegistryKey { hive, hive_name, ref path } => {
                    scan_registry_remnants_in_path(hive, hive_name, path, &match_token, publisher, target.base_score, &mut local_remnants);
                }
            }
            local_remnants
        })
        .collect();


    remnants.append(&mut fs_remnants);

    if settings.scan_level == "aggressive" {
        let mut com_orphans = super::com_purger::scan_com_orphans(&match_token, install_location);
        let mut typelib_orphans = super::com_purger::scan_typelib_orphans(&match_token);
        let mut asep_remnants = super::autoruns::scan_aseps(&match_token, install_location);
        let mut msi_remnants = super::msi::scan_msi_remnants(&match_token, install_location);
        
        remnants.append(&mut com_orphans);
        remnants.append(&mut typelib_orphans);
        remnants.append(&mut asep_remnants);
        remnants.append(&mut msi_remnants);
    }


    // Calculate confidence for each remnant
    for item in &mut remnants {
        let (conf, score) = calculate_confidence(item, app_name, publisher, install_location);
        item.confidence = conf;
        item.score = score;
    }

    // Deduplicate by path
    let mut unique_remnants = Vec::new();
    for item in remnants {
        if !unique_remnants.iter().any(|r: &RemnantItem| r.path == item.path) {
            unique_remnants.push(item);
        }
    }

    // Sort by confidence score (highest first)
    unique_remnants.sort_by(|a, b| b.score.cmp(&a.score));

    unique_remnants
}

fn clean_match_token(app_name: &str) -> String {
    let remove_words = ["uninstaller", "installer", "setup", "software", "client", "x64", "x86", "32bit", "64bit", "free", "pro", "ultimate"];

    // Strip noise words on word boundaries only. A plain substring replace
    // mangled real names (e.g. "GoPro" -> "go", "Professional" -> "fessional"),
    // causing wrong or missed matches.
    let cleaned: String = app_name
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .collect();

    let kept: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|w| {
            let wl = w.to_lowercase();
            !remove_words.contains(&wl.as_str())
        })
        .collect();

    kept.join(" ").trim().to_lowercase()
}

fn scan_directory_remnants_recursive(parent_dir: &Path, token: &str, base_score: i32, remnants: &mut Vec<RemnantItem>) {
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
                confidence: "Medium".to_string(), // Placeholder, updated later
                score: base_score,
            });
        }
    }
}

fn scan_registry_remnants_in_path(
    hkey: winreg::HKEY,
    hive_name: &str,
    software_path: &str,
    token: &str,
    publisher: Option<&str>,
    base_score: i32,
    remnants: &mut Vec<RemnantItem>,
) {
    let root = RegKey::predef(hkey);
    let key_to_open = if software_path == "HKLM_UNINSTALL" {
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
    } else if software_path == "HKCU_UNINSTALL" {
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
    } else {
        software_path
    };

    let key = match root.open_subkey_with_flags(key_to_open, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    // 1. Direct search in subkeys
    for name in key.enum_keys().filter_map(|x| x.ok()) {
        let name_lower = name.to_lowercase();
        if name_lower.contains(token) {
            remnants.push(RemnantItem {
                path: format!(r"{}\{}\{}", hive_name, key_to_open, name),
                item_type: "RegistryKey".to_string(),
                size: 0,
                confidence: "Medium".to_string(),
                score: base_score,
            });
        }
    }

    // 2. Publisher specific subkeys search
    if let Some(pub_name) = publisher {
        let pub_clean = clean_match_token(&pub_name.to_lowercase());
        if pub_clean.len() >= 3 {
            for name in key.enum_keys().filter_map(|x| x.ok()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains(&pub_clean) {
                    let pub_path = format!(r"{}\{}", key_to_open, name);
                    if let Ok(pub_key) = root.open_subkey_with_flags(&pub_path, KEY_READ) {
                        for app_name in pub_key.enum_keys().filter_map(|x| x.ok()) {
                            let app_lower = app_name.to_lowercase();
                            if app_lower.contains(token) {
                                remnants.push(RemnantItem {
                                    path: format!(r"{}\{}\{}", hive_name, pub_path, app_name),
                                    item_type: "RegistryKey".to_string(),
                                    size: 0,
                                    confidence: "Medium".to_string(),
                                    score: base_score,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn calculate_confidence(item: &RemnantItem, app_name: &str, _publisher: Option<&str>, install_location: Option<&str>) -> (String, i32) {
    let mut score = item.score; // starts with base_score (30-80)
    let path_lower = item.path.to_lowercase();
    let _app_name_lower = app_name.to_lowercase();


    // ═══ BOOST MODIFIERS ═══
    // GUID Match
    if path_lower.contains("{") && path_lower.contains("}") {
        score += 20;
    }
    // Empty directory or log files only
    if item.item_type == "Directory" && is_folder_empty_or_logs_only(Path::new(&item.path)) {
        score += 10;
    }
    // Install location exact/prefix match
    if let Some(loc) = install_location {
        let loc_lower = loc.to_lowercase();
        if !loc_lower.is_empty() && path_lower.starts_with(&loc_lower) {
            score += 15;
        }
    }

    // ═══ CRITICAL SAFETY MODIFIERS (Anti-False-Positive) ═══
    // Generic Folder Names Check
    if is_generic_name(&item.path) {
        score -= 35;
    }
    // SharedDLLs reference count check
    if item.item_type == "File" && is_shared_dll(&item.path) {
        score -= 50;
    }
    // Modified in the last 24h
    if has_recent_activity(&item.path, 86400) {
        score -= 15;
    }
    // Common Files
    if path_lower.contains("common files") {
        score -= 20;
    }
    // System paths
    if path_lower.contains("system32") || path_lower.contains("syswow64") || path_lower.starts_with(r"c:\windows") {
        score -= 40;
    }

    score = score.clamp(0, 100);

    let conf = match score {
        80..=100 => "VeryHigh".to_string(),
        55..=79  => "High".to_string(),
        35..=54  => "Medium".to_string(),
        _        => "Low".to_string(),
    };

    (conf, score)
}

fn is_folder_empty_or_logs_only(path: &Path) -> bool {
    if !path.is_dir() { return false; }
    if let Ok(entries) = fs::read_dir(path) {
        let mut count = 0;
        let mut logs_only = true;
        for entry in entries.filter_map(|e| e.ok()) {
            count += 1;
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.ends_with(".log") && !name.ends_with(".txt") {
                logs_only = false;
            }
        }
        return count == 0 || (count > 0 && logs_only);
    }
    false
}

fn is_shared_dll(path: &str) -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs", KEY_READ)
        .and_then(|k| k.get_value::<u32, _>(path))
        .map_or(false, |count| count > 0)
}

fn is_generic_name(path: &str) -> bool {
    let path_buf = Path::new(path);
    if let Some(name) = path_buf.file_name().and_then(|n| n.to_str()) {
        let name_lower = name.to_lowercase();
        let generic_names = ["bin", "lib", "data", "config", "assets", "resources", "temp", "cache", "common", "shared", "local", "plugins"];
        return generic_names.contains(&name_lower.as_str());
    }
    false
}

fn has_recent_activity(path: &str, threshold_seconds: u64) -> bool {
    let path_buf = Path::new(path);
    if let Ok(metadata) = path_buf.metadata() {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs() < threshold_seconds;
            }
        }
    }
    false
}

pub fn purge_remnant_item(item: &RemnantItem) -> Result<(), String> {
    match item.item_type.as_str() {
        "File" | "Directory" => {
            let path = Path::new(&item.path);
            if path.exists() {
                match crate::locker::delete_file_with_escalation(&item.path) {
                    crate::locker::DeleteResult::Failed(err) => return Err(err),
                    _ => {}
                }
            }
        }
        "RegistryKey" => {
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
                "HKLM_WOW6432" => RegKey::predef(HKEY_LOCAL_MACHINE),
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

            // delete_subkey fails on keys that contain subkeys, which most
            // app remnant keys do. Delete recursively instead.
            parent_key.delete_subkey_all(key_to_delete)
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
