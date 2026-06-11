use serde::{Deserialize, Serialize};
use winreg::enums::*;
use winreg::RegKey;

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
        let install_location: Option<String> = subkey.get_value("InstallLocation").ok();
        let install_date: Option<String> = subkey.get_value("InstallDate").ok();
        let display_icon: Option<String> = subkey.get_value("DisplayIcon").ok();
        
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
        });
    }
}

