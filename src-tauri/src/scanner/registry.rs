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
    pub estimated_size: Option<u64>,
    pub registry_path: String,
    pub hive: String,
    pub is_uwp: bool,
}

pub fn scan_registry() -> Vec<InstalledApp> {
    let mut apps = Vec::new();

    // 1. HKLM (64-bit & 32-bit apps on 32-bit OS)
    scan_uninstall_key(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM",
        &mut apps,
    );

    // 2. HKLM WOW6432Node (32-bit apps on 64-bit OS)
    scan_uninstall_key(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM_WOW6432",
        &mut apps,
    );

    // 3. HKCU (User specific apps)
    scan_uninstall_key(
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKCU",
        &mut apps,
    );

    apps
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

        // Avoid duplicates if already scanned
        if apps.iter().any(|app| app.id == name && app.display_name == display_name) {
            continue;
        }

        // ParentName indicates it might be an update or subcomponent, often skipped or marked
        let parent_key_name: Option<String> = subkey.get_value("ParentKeyName").ok();
        if parent_key_name.is_some() {
            // Keep updates for now or skip? Geek uninstaller hides updates by default. Let's skip subcomponents for simplicity or keep them.
            // For now, let's keep them, but in front-end we can filter. Actually, skipping is better to keep list clean.
            continue;
        }

        let display_version: Option<String> = subkey.get_value("DisplayVersion").ok();
        let publisher: Option<String> = subkey.get_value("Publisher").ok();
        let uninstall_string: Option<String> = subkey.get_value("UninstallString").ok();
        let quiet_uninstall_string: Option<String> = subkey.get_value("QuietUninstallString").ok();
        let install_location: Option<String> = subkey.get_value("InstallLocation").ok();
        let install_date: Option<String> = subkey.get_value("InstallDate").ok();
        let display_icon: Option<String> = subkey.get_value("DisplayIcon").ok();
        
        let estimated_size: Option<u64> = subkey.get_value("EstimatedSize")
            .map(|val: u32| val as u64)
            .ok()
            .or_else(|| {
                // If EstimatedSize is missing, we could later calculate it from the InstallLocation size
                None
            });

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
            estimated_size,
            registry_path,
            hive: hive_name.to_string(),
            is_uwp: false,
        });
    }
}
