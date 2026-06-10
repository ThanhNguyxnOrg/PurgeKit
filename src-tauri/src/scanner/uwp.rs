use serde::{Deserialize, Serialize};
use std::process::Command;
use std::os::windows::process::CommandExt;
use super::registry::InstalledApp;

// Hiding console window flag for Windows process execution
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UwpRawApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "PublisherId")]
    publisher_id: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "InstallLocation")]
    install_location: Option<String>,
    #[serde(rename = "PackageFamilyName")]
    package_family_name: Option<String>,
    #[serde(rename = "PackageFullName")]
    package_full_name: Option<String>,
}

pub fn scan_uwp_apps() -> Vec<InstalledApp> {
    let mut apps = Vec::new();

    // Run PowerShell command to get appx packages. We filter out framework packages to keep the list clean.
    // Framework packages like VCLibs, .NET, etc. are not uninstallable by typical users.
    let script = r#"Get-AppxPackage | Where-Object { -not $_.IsFramework -and $_.NonRemovable -ne $true } | Select-Object Name, PublisherId, Version, InstallLocation, PackageFamilyName, PackageFullName | ConvertTo-Json -Compress"#;
    
    let output = match Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["-NoProfile", "-Command", script])
        .output()
    {
        Ok(out) => out,
        Err(_) => return apps, // PowerShell not available or failed
    };

    if !output.status.success() {
        return apps;
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stdout_str = stdout_str.trim();
    if stdout_str.is_empty() {
        return apps;
    }

    // ConvertTo-Json returns a single object if there's only 1 item, or an array if there are multiple.
    // We try to parse it as an array first, then as a single object if that fails.
    let raw_apps: Vec<UwpRawApp> = if stdout_str.starts_with('[') {
        serde_json::from_str(stdout_str).unwrap_or_default()
    } else {
        match serde_json::from_str::<UwpRawApp>(stdout_str) {
            Ok(app) => vec![app],
            Err(_) => Vec::new(),
        }
    };

    for raw in raw_apps {
        let name = raw.name;
        let package_family = match raw.package_family_name {
            Some(pf) => pf,
            None => continue,
        };
        let package_full = match raw.package_full_name {
            Some(pf) => pf,
            None => continue,
        };

        // Skip apps without install location (they might be registration stubs)
        let loc = match &raw.install_location {
            Some(l) if !l.trim().is_empty() => l.clone(),
            _ => continue,
        };

        // Clean up the name for user display (e.g. "Microsoft.WindowsCalculator" -> "WindowsCalculator")
        let display_name = clean_uwp_display_name(&name);

        // UWP Uninstall command: Remove-AppxPackage -Package <PackageFullName>
        let uninstall_string = format!("powershell -NoProfile -Command \"Remove-AppxPackage -Package {}\"", package_full);

        apps.push(InstalledApp {
            id: package_family.clone(),
            display_name,
            display_version: raw.version.clone(),
            publisher: raw.publisher_id.clone(),
            uninstall_string: Some(uninstall_string.clone()),
            quiet_uninstall_string: Some(uninstall_string),
            install_location: Some(loc),
            install_date: None, // PowerShell Get-AppxPackage does not easily return install date without slower commands
            display_icon: None, // Store app icons are resolved using AppxManifest.xml, which we can implement later
            estimated_size: None, // Can calculate size from directory later
            registry_path: format!(r"UWP\{}", package_family),
            hive: "UWP".to_string(),
            is_uwp: true,
        });
    }

    apps
}

fn clean_uwp_display_name(name: &str) -> String {
    // Basic cleanup of Microsoft / Publisher prefixes
    let mut clean = name.to_string();
    if clean.starts_with("Microsoft.") {
        clean = clean.replacen("Microsoft.", "", 1);
    }
    
    // Add spaces before uppercase letters for readability, except for first letter
    let mut spaced = String::new();
    for (i, c) in clean.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            spaced.push(' ');
        }
        spaced.push(c);
    }
    
    spaced
}
