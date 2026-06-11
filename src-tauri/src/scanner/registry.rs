use serde::{Deserialize, Serialize};
use winreg::enums::*;
use winreg::RegKey;
use std::path::Path;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledApp {
    pub id: String,
    pub display_name: String,
    pub display_version: Option<String>,
    pub publisher: Option<String>,
    pub uninstall_string: Option<String>,
    pub quiet_uninstall_string: Option<String>,
    pub install_location: Option<String>,
    pub install_date: Option<String>,
    pub display_icon: Option<String>,
    pub icon_base64: Option<String>,
    pub estimated_size: Option<u64>,
    pub registry_path: String,
    pub hive: String,
    pub is_uwp: bool,
    pub is_verified: Option<bool>,
}

pub fn scan_registry() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    let unsub = r"Microsoft\Windows\CurrentVersion\Uninstall";

    // Hive 1: HKLM 64-bit system apps
    scan_uninstall_key(
        HKEY_LOCAL_MACHINE,
        &format!(r"SOFTWARE\{}", unsub),
        "HKLM",
        &mut apps,
    );

    // Hive 2: HKLM WOW6432Node (32-bit apps on 64-bit OS)
    scan_uninstall_key(
        HKEY_LOCAL_MACHINE,
        &format!(r"SOFTWARE\Wow6432Node\{}", unsub),
        "HKLM_WOW",
        &mut apps,
    );

    // Hive 3: HKCU (User specific apps)
    scan_uninstall_key(
        HKEY_CURRENT_USER,
        &format!(r"SOFTWARE\{}", unsub),
        "HKCU",
        &mut apps,
    );

    // Hive 4: HKCU WOW6432Node (32-bit user apps)
    scan_uninstall_key(
        HKEY_CURRENT_USER,
        &format!(r"SOFTWARE\Wow6432Node\{}", unsub),
        "HKCU_WOW",
        &mut apps,
    );

    // Hive 5: VirtualStore (32-bit legacy apps redirected by Windows)
    scan_uninstall_key(
        HKEY_CURRENT_USER,
        &format!(r"Software\Classes\VirtualStore\MACHINE\SOFTWARE\{}", unsub),
        "VirtualStore",
        &mut apps,
    );

    // Hive 6: VirtualStore WOW6432Node variant
    scan_uninstall_key(
        HKEY_CURRENT_USER,
        &format!(r"Software\Classes\VirtualStore\MACHINE\SOFTWARE\Wow6432Node\{}", unsub),
        "VirtualStore_WOW",
        &mut apps,
    );

    deduplicate_apps(&mut apps);

    apps
}

fn app_richness_score(app: &InstalledApp) -> u32 {
    let mut score = 0;
    if app.icon_base64.is_some() { score += 5; }
    if app.uninstall_string.is_some() { score += 3; }
    if app.install_location.is_some() { score += 2; }
    if app.estimated_size.is_some() { score += 2; }
    if app.display_version.is_some() { score += 1; }
    score
}

fn deduplicate_apps(apps: &mut Vec<InstalledApp>) {
    let mut unique_apps: Vec<InstalledApp> = Vec::new();
    for app in apps.drain(..) {
        let exists_idx = unique_apps.iter().position(|a| {
            a.id == app.id || (a.display_name.to_lowercase() == app.display_name.to_lowercase() 
                && a.publisher.as_ref().map(|p| p.to_lowercase()) == app.publisher.as_ref().map(|p| p.to_lowercase()))
        });
        if let Some(idx) = exists_idx {
            let current = &unique_apps[idx];
            let current_score = app_richness_score(current);
            let new_score = app_richness_score(&app);
            if new_score > current_score {
                unique_apps[idx] = app;
            }
        } else {
            unique_apps.push(app);
        }
    }
    *apps = unique_apps;
}

fn scan_uninstall_key(
    hkey: winreg::HKEY,
    subpath: &str,
    hive_name: &str,
    apps: &mut Vec<InstalledApp>,
) {
    let hk = RegKey::predef(hkey);
    let uninstall_key = match hk.open_subkey_with_flags(subpath, KEY_READ) {
        Ok(key) => key,
        Err(_) => return, // Key doesn't exist or access denied, skip
    };

    for name in uninstall_key.enum_keys().filter_map(|x| x.ok()) {
        let subkey = match uninstall_key.open_subkey_with_flags(&name, KEY_READ) {
            Ok(key) => key,
            Err(_) => continue, // Skip subkeys we cannot read
        };

        // DisplayName is mandatory. If an app doesn't have a DisplayName, we usually skip it (it's often an update or system component)
        let display_name: String = match subkey.get_value("DisplayName") {
            Ok(val) => val,
            Err(_) => continue,
        };

        // Clean up display name (skip empty names, or system components if desired)
        let display_name = display_name.trim();
        if display_name.is_empty() {
            continue;
        }

        // ParentName indicates it might be an update or subcomponent, often skipped or marked
        let parent_key_name: Option<String> = subkey.get_value("ParentKeyName").ok();
        if parent_key_name.is_some() {
            continue;
        }

        let display_version: Option<String> = subkey.get_value("DisplayVersion").ok();
        let publisher: Option<String> = subkey.get_value("Publisher").ok();
        let uninstall_string: Option<String> = subkey.get_value("UninstallString").ok();
        let quiet_uninstall_string: Option<String> = subkey.get_value("QuietUninstallString").ok();
        let mut install_location: Option<String> = subkey.get_value("InstallLocation").ok();
        let install_date: Option<String> = subkey.get_value("InstallDate").ok();
        let mut display_icon: Option<String> = subkey.get_value("DisplayIcon").ok();

        let is_msi = name.starts_with('{') && name.ends_with('}') && name.len() == 38;
        if is_msi {
            if install_location.is_none() {
                if let Some(msi_loc) = get_msi_product_property(&name, "InstallLocation") {
                    if !msi_loc.is_empty() {
                        install_location = Some(msi_loc);
                    }
                }
            }
            if display_icon.is_none() {
                if let Some(msi_icon) = get_msi_product_property(&name, "ProductIcon") {
                    if !msi_icon.is_empty() {
                        display_icon = Some(msi_icon);
                    }
                }
            }
        }
        
        let mut icon_base64 = None;
        if let Some(ref icon_path) = display_icon {
            let clean_path = icon_path.trim().trim_matches('"');
            let clean_path = if let Some(comma_pos) = clean_path.rfind(',') {
                let suffix = &clean_path[comma_pos + 1..];
                if suffix.chars().all(|c| c.is_ascii_digit()) {
                    &clean_path[..comma_pos]
                } else {
                    clean_path
                }
            } else {
                clean_path
            };

            if !clean_path.is_empty() {
                if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(clean_path) {
                    icon_base64 = Some(base64_str);
                }
            }
        }
        
        // Fallback 1: Try to extract icon from the uninstaller executable itself
        if icon_base64.is_none() {
            if let Some(ref uninst) = uninstall_string {
                if let Some(exe_path) = get_executable_from_uninstall_string(uninst) {
                    let clean_exe = exe_path.trim().trim_matches('"');
                    if !clean_exe.is_empty() && Path::new(clean_exe).exists() {
                        if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(clean_exe) {
                            icon_base64 = Some(base64_str);
                        }
                    }
                }
            }
        }

        // Fallback 2: Try to find executables in the InstallLocation folder
        if icon_base64.is_none() {
            if let Some(ref loc) = install_location {
                let loc_path = Path::new(loc.trim().trim_matches('"'));
                if loc_path.exists() && loc_path.is_dir() {
                    let mut search_paths = vec![loc_path.to_path_buf()];
                    let bin_path = loc_path.join("bin");
                    if bin_path.exists() && bin_path.is_dir() {
                        search_paths.push(bin_path);
                    }
                    let cmd_path = loc_path.join("cmd");
                    if cmd_path.exists() && cmd_path.is_dir() {
                        search_paths.push(cmd_path);
                    }

                    let mut exe_candidates = Vec::new();
                    let app_name_lower = display_name.to_lowercase();

                    for dir in search_paths {
                        if let Ok(entries) = std::fs::read_dir(dir) {
                            for entry in entries.filter_map(|e| e.ok()) {
                                let p = entry.path();
                                if p.is_file() && p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe")) {
                                    if let Some(file_name) = p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase()) {
                                        if file_name.contains("uninst") || file_name.contains("setup") || file_name.contains("helper") || file_name.contains("crash") {
                                            continue;
                                        }

                                        let mut score = 0;
                                        let name_without_ext = if file_name.len() > 4 {
                                            &file_name[..file_name.len() - 4]
                                        } else {
                                            &file_name
                                        };

                                        if app_name_lower.contains(name_without_ext) || name_without_ext.contains(&app_name_lower) {
                                            score += 100;
                                        }

                                        for word in app_name_lower.split_whitespace() {
                                            if word.len() > 2 && name_without_ext.contains(word) {
                                                score += 30;
                                            }
                                        }

                                        if let Ok(meta) = p.metadata() {
                                            score += std::cmp::min(50, (meta.len() / 1024 / 1024) as i32);
                                        }

                                        exe_candidates.push((score, p));
                                    }
                                }
                            }
                        }
                    }

                    exe_candidates.sort_by_key(|b| std::cmp::Reverse(b.0));
                    if let Some((_, best_exe_path)) = exe_candidates.first() {
                        if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(best_exe_path) {
                            icon_base64 = Some(base64_str);
                        }
                    }
                }
            }
        }

        // Fallback 3: Try to find executables in the system PATH environment variable
        if icon_base64.is_none() {
            let app_name_lower = display_name.to_lowercase();
            let mut search_exes = Vec::new();
            
            // Special cases
            if app_name_lower.contains("github cli") {
                search_exes.push("gh.exe".to_string());
            } else if app_name_lower.contains("go programming") || app_name_lower.contains("golang") {
                search_exes.push("go.exe".to_string());
            }

            // Generic candidates: Extract words from the display name
            let cleaned_name = app_name_lower
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                .collect::<String>();

            for word in cleaned_name.split_whitespace() {
                // Ignore short or extremely common keywords to avoid false matches
                if word.len() >= 3 && word != "cli" && word != "sdk" && word != "win" && word != "amd" && word != "user" && word != "program" && word != "language" && word != "version" {
                    let exe_name = format!("{}.exe", word);
                    if !search_exes.contains(&exe_name) {
                        search_exes.push(exe_name);
                    }
                }
            }

            if !search_exes.is_empty() {
                if let Some(path_env) = std::env::var_os("PATH") {
                    for dir in std::env::split_paths(&path_env) {
                        for exe in &search_exes {
                            let candidate = dir.join(exe);
                            if candidate.exists() && candidate.is_file() {
                                if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(&candidate) {
                                    icon_base64 = Some(base64_str);
                                    break;
                                }
                            }
                        }
                        if icon_base64.is_some() {
                            break;
                        }
                    }
                }
            }
        }
        
        // EstimatedSize: Windows stores KiB -> multiply by 1024 to get bytes
        let estimated_size: Option<u64> = subkey.get_value("EstimatedSize")
            .map(|val: u32| (val as u64) * 1024)
            .ok();

        let registry_path = format!(r"{}\{}", subpath, name);

        // Filter system components / hidden components (SystemComponent = 1)
        let system_component: u32 = subkey.get_value("SystemComponent").unwrap_or(0);
        if system_component == 1 {
            continue;
        }

        // Also check if UninstallString is empty, usually we cannot uninstall without it (though some store apps might use different mechanism)
        if uninstall_string.is_none() {
            // Check if it's UWP, if not skip
            continue;
        }

        // Verify signature of the uninstaller or display icon
        let mut is_verified = None;
        if let Some(ref uninst) = uninstall_string {
            if let Some(exe_path) = get_executable_from_uninstall_string(uninst) {
                if crate::winutil::verify_file_signature(&exe_path) {
                    is_verified = Some(true);
                } else {
                    is_verified = Some(false);
                }
            }
        }
        
        if is_verified != Some(true) {
            if let Some(ref icon_path) = display_icon {
                let clean_icon = icon_path.trim().trim_matches('"');
                let clean_icon = if let Some(comma_pos) = clean_icon.rfind(',') {
                    let suffix = &clean_icon[comma_pos + 1..];
                    if suffix.chars().all(|c| c.is_ascii_digit()) {
                        &clean_icon[..comma_pos]
                    } else {
                        clean_icon
                    }
                } else {
                    clean_icon
                };
                if !clean_icon.is_empty() && crate::winutil::verify_file_signature(clean_icon) {
                    is_verified = Some(true);
                }
            }
        }

        apps.push(InstalledApp {
            id: name,
            display_name: display_name.to_string(),
            display_version,
            publisher,
            uninstall_string,
            quiet_uninstall_string,
            install_location,
            install_date,
            display_icon,
            icon_base64,
            estimated_size,
            registry_path,
            hive: hive_name.to_string(),
            is_uwp: false,
            is_verified,
        });
    }
}

fn get_msi_product_property(product_code: &str, property_name: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    
    if !product_code.starts_with('{') || !product_code.ends_with('}') {
        return None;
    }
    
    let pc_wide: Vec<u16> = std::ffi::OsStr::new(product_code)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
        
    let property_wide: Vec<u16> = std::ffi::OsStr::new(property_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
        
    let mut len: u32 = 512;
    let mut buf: Vec<u16> = vec![0; len as usize];
    
    unsafe {
        let res = windows_sys::Win32::System::ApplicationInstallationAndServicing::MsiGetProductInfoW(
            pc_wide.as_ptr(),
            property_wide.as_ptr(),
            buf.as_mut_ptr(),
            &mut len
        );
        
        if res == 0 {
            let slice = &buf[..len as usize];
            let os_str = OsString::from_wide(slice);
            return Some(os_str.to_string_lossy().trim().to_string());
        } else if res == 234 { // ERROR_MORE_DATA
            buf.resize(len as usize + 1, 0);
            let res2 = windows_sys::Win32::System::ApplicationInstallationAndServicing::MsiGetProductInfoW(
                pc_wide.as_ptr(),
                property_wide.as_ptr(),
                buf.as_mut_ptr(),
                &mut len
            );
            if res2 == 0 {
                let slice = &buf[..len as usize];
                let os_str = OsString::from_wide(slice);
                return Some(os_str.to_string_lossy().trim().to_string());
            }
        }
    }
    None
}

fn get_executable_from_uninstall_string(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    
    if s.starts_with('"') {
        let parts: Vec<&str> = s[1..].split('"').collect();
        if !parts.is_empty() {
            return Some(parts[0].to_string());
        }
    }
    
    let first_space = s.find(' ').unwrap_or(s.len());
    let mut exe = s[..first_space].to_string();
    if !exe.to_lowercase().ends_with(".exe") && s.to_lowercase().contains(".exe") {
        if let Some(exe_idx) = s.to_lowercase().find(".exe") {
            exe = s[..exe_idx + 4].to_string();
        }
    }
    Some(exe)
}

