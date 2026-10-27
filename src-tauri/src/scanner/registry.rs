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
    pub modify_path: Option<String>,
    #[serde(default)]
    pub release_type: Option<String>,
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
    super::derived_cache::finish_scan();

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
        let modify_path: Option<String> = subkey.get_value("ModifyPath").ok();
        let release_type: Option<String> = subkey.get_value("ReleaseType").ok();
        let mut install_location: Option<String> = subkey.get_value("InstallLocation").ok();
        let install_date: Option<String> = subkey.get_value("InstallDate").ok();
        let mut display_icon: Option<String> = subkey.get_value("DisplayIcon").ok();

        // EstimatedSize: Windows stores KiB -> multiply by 1024 to get bytes
        let estimated_size: Option<u64> = subkey
            .get_value("EstimatedSize")
            .map(|val: u32| (val as u64) * 1024)
            .ok();

        let registry_path = format!(r"{}\{}", subpath, name);

        // Filter system components / hidden components (SystemComponent = 1)
        let system_component: u32 = subkey.get_value("SystemComponent").unwrap_or(0);
        if system_component == 1 {
            continue;
        }

        let fingerprint = derived_fingerprint(
            display_name,
            display_version.as_deref(),
            uninstall_string.as_deref(),
            display_icon.as_deref(),
            install_location.as_deref(),
            install_date.as_deref(),
            estimated_size,
        );
        let cache_key = format!("{}|{}", hive_name, registry_path).to_lowercase();

        // Cached location/icon/size values skip MSI property queries, folder
        // scans and icon extraction. The fingerprint invalidates the entry when
        // registry values change; stale paths are discarded before display.
        if let Some(cached) = super::derived_cache::lookup(&cache_key, &fingerprint) {
            let cached_install = cached
                .install_location
                .filter(|location| Path::new(location.trim().trim_matches('"')).is_dir());
            let cached_icon = cached.display_icon.or(display_icon);
            apps.push(InstalledApp {
                id: name,
                display_name: display_name.to_string(),
                display_version,
                publisher,
                uninstall_string,
                quiet_uninstall_string,
                modify_path,
                release_type,
                install_location: cached_install,
                install_date,
                display_icon: cached_icon,
                icon_base64: cached.icon_base64,
                estimated_size: cached.estimated_size.or(estimated_size),
                registry_path,
                hive: hive_name.to_string(),
                is_uwp: false,
                is_verified: cached.is_verified,
            });
            continue;
        }

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

        if install_location.is_none() {
            install_location = infer_install_location(
                uninstall_string.as_deref(),
                display_icon.as_deref(),
                display_name,
            );
        }
        
        let mut icon_base64 = None;
        if let Some(ref icon_path) = display_icon {
            let clean_path = clean_icon_path(icon_path);
            if !clean_path.is_empty() {
                if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(&clean_path) {
                    icon_base64 = Some(base64_str);
                }
            }
        }

        // Fallback 1: Start Menu shortcut icon (same source Geek consults before
        // falling back to install folders and uninstallers).
        if icon_base64.is_none() {
            if let Some(icon_source) =
                super::shortcuts::find_icon_source_for_name(display_name)
            {
                if let Ok(base64_str) = windows_icons::get_icon_base64_by_path(&icon_source) {
                    icon_base64 = Some(base64_str);
                }
            }
        }

        // Fallback 2: Try to extract icon from the uninstaller executable itself
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

        // Fallback 3: Try to find executables in the InstallLocation folder
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
                                        let name_without_ext = file_name.strip_suffix(".exe").unwrap_or(&file_name);

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

        // Fallback 4: Try to find executables in the system PATH environment variable
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
        
        // Entries without UninstallString are still useful: they may be
        // orphaned registrations or components that can be removed with the
        // "Remove Entry" action.

        let is_verified = detect_is_verified(
            uninstall_string.as_deref(),
            display_icon.as_deref(),
        );

        super::derived_cache::store(
            cache_key,
            super::derived_cache::CachedDerived {
                fingerprint,
                install_location: install_location.clone(),
                display_icon: display_icon.clone(),
                icon_base64: icon_base64.clone(),
                estimated_size,
                is_verified,
            },
        );

        apps.push(InstalledApp {
            id: name,
            display_name: display_name.to_string(),
            display_version,
            publisher,
            uninstall_string,
            quiet_uninstall_string,
            modify_path,
            release_type,
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

fn derived_fingerprint(
    display_name: &str,
    display_version: Option<&str>,
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
    install_location: Option<&str>,
    install_date: Option<&str>,
    estimated_size: Option<u64>,
) -> String {
    format!(
        "v3\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        display_name,
        display_version.unwrap_or_default(),
        uninstall_string.unwrap_or_default(),
        display_icon.unwrap_or_default(),
        install_location.unwrap_or_default(),
        install_date.unwrap_or_default(),
        estimated_size.unwrap_or(0),
        verification_stamp(uninstall_string, display_icon),
    )
}

fn file_stamp(path: &str) -> String {
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return "empty;".to_string();
    }
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            format!("{}:{}:{};", path.to_lowercase(), metadata.len(), modified)
        }
        Err(_) => format!("{}:missing;", path.to_lowercase()),
    }
}

fn verification_stamp(
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
) -> String {
    let mut stamp = String::new();
    if let Some(uninstall) = uninstall_string {
        if let Some(executable) = get_executable_from_uninstall_string(uninstall) {
            stamp.push_str(&file_stamp(&executable));
        }
    }
    if let Some(icon) = display_icon {
        stamp.push_str(&file_stamp(&clean_icon_path(icon)));
    }
    stamp
}

fn detect_is_verified(
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
) -> Option<bool> {
    let mut is_verified = None;
    if let Some(uninst) = uninstall_string {
        if let Some(exe_path) = get_executable_from_uninstall_string(uninst) {
            is_verified = Some(crate::winutil::verify_file_signature(&exe_path));
        }
    }
    if is_verified != Some(true) {
        if let Some(icon_path) = display_icon {
            let clean_icon = clean_icon_path(icon_path);
            if !clean_icon.is_empty() && crate::winutil::verify_file_signature(&clean_icon) {
                is_verified = Some(true);
            }
        }
    }
    is_verified
}

fn clean_icon_path(icon: &str) -> String {
    let cleaned = icon.trim().trim_matches('"');
    let cleaned = if let Some((path, suffix)) = cleaned.rsplit_once(',') {
        if suffix.chars().all(|character| character.is_ascii_digit()) {
            path
        } else {
            cleaned
        }
    } else {
        cleaned
    };
    crate::winutil::expand_env_strings(cleaned.trim().trim_matches('"'))
}

fn infer_install_location(
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
    display_name: &str,
) -> Option<String> {
    let name_words: Vec<String> = display_name
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() >= 3)
        .map(|word| word.to_string())
        .collect();

    for candidate in [uninstall_string, display_icon].into_iter().flatten() {
        let command = clean_icon_path(candidate);
        let Some(executable) = crate::scanner::registry::get_executable_from_uninstall_string(&command)
        else {
            continue;
        };
        let path = Path::new(executable.trim().trim_matches('"'));
        if !path.is_file() {
            continue;
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        if crate::winutil::is_safe_remnant_path(&parent.to_string_lossy()).is_err() {
            continue;
        }
        let folder = parent
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_lowercase();
        if folder.is_empty() {
            continue;
        }
        let folder_tokens: Vec<&str> = folder
            .split(|character: char| !character.is_alphanumeric())
            .filter(|token| token.len() >= 3)
            .collect();
        let matches_name = name_words.iter().any(|word| {
            folder.contains(word)
                || folder_tokens.iter().any(|token| {
                    token.contains(word)
                        || (token.len() >= 6
                            && word.len() >= 6
                            && token.len().abs_diff(word.len()) <= 2
                            && super::remnants::levenshtein_distance(token, word) <= 1)
                })
        });
        if matches_name {
            return Some(parent.to_string_lossy().to_string());
        }
    }

    None
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
    
    let s_lower = s.to_lowercase();
    if let Some(idx_in_lower) = s_lower.find(".exe") {
        let char_count = s_lower[..idx_in_lower + 4].chars().count();
        let exe: String = s.chars().take(char_count).collect();
        return Some(exe);
    }
    
    let first_space = s.find(' ').unwrap_or(s.len());
    Some(s[..first_space].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_executable_from_uninstall_string() {
        assert_eq!(
            get_executable_from_uninstall_string(r#""C:\Program Files\App\uninst.exe" /S"#),
            Some(r"C:\Program Files\App\uninst.exe".to_string())
        );
        assert_eq!(
            get_executable_from_uninstall_string(r#"C:\Program Files\App\uninst.exe /S"#),
            Some(r"C:\Program Files\App\uninst.exe".to_string())
        );
        assert_eq!(
            get_executable_from_uninstall_string(r#"C:\Windows\System32\cmd.exe"#),
            Some(r"C:\Windows\System32\cmd.exe".to_string())
        );
        assert_eq!(
            get_executable_from_uninstall_string(r#"C:\ChươngTrình\uninstall.exe /S"#),
            Some(r"C:\ChươngTrình\uninstall.exe".to_string())
        );
        assert_eq!(
            get_executable_from_uninstall_string(""),
            None
        );
    }

    #[test]
    fn test_infer_install_location_from_uninstall_string() {
        let root = std::env::temp_dir().join("purgekit_infer_location_test");
        let app_dir = root.join("My App");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&app_dir).unwrap();
        let uninstaller = app_dir.join("uninstall.exe");
        std::fs::write(&uninstaller, b"stub").unwrap();

        let command = format!("\"{}\" /S", uninstaller.to_string_lossy());
        let inferred = infer_install_location(Some(&command), None, "My App");
        assert!(inferred.is_some(), "expected inferred install location");
        assert!(inferred.unwrap().to_lowercase().contains("my app"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "manual smoke test scans the host registry twice"]
    fn scan_registry_cache_smoke() {
        let first_start = std::time::Instant::now();
        let first = scan_registry();
        let first_elapsed = first_start.elapsed();
        let summary = super::super::derived_cache::summary();
        assert!(summary.entries > 0, "first scan should populate the cache");

        let second_start = std::time::Instant::now();
        let second = scan_registry();
        let second_elapsed = second_start.elapsed();
        assert_eq!(first.len(), second.len());

        eprintln!(
            "registry cache smoke: {} apps, first {:?}, cached {:?}, {} cache entries",
            second.len(),
            first_elapsed,
            second_elapsed,
            summary.entries
        );
    }

    #[test]
    fn test_scan_registry_no_crash() {
        let apps = scan_registry();
        println!("Scan registry returned {} apps", apps.len());
    }
}

