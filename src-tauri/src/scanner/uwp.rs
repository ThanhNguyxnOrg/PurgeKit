use serde::{Deserialize, Serialize};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::fs;
use std::path::Path;
use super::registry::InstalledApp;
use base64::{Engine as _, engine::general_purpose};

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
    // We try to query all users (-AllUsers) if running as admin, else fall back to the current user.
    let script = r#"$apps = try { Get-AppxPackage -AllUsers -ErrorAction Stop } catch { Get-AppxPackage }; $apps | Where-Object { -not $_.IsFramework -and $_.NonRemovable -ne $true } | Select-Object Name, PublisherId, Version, InstallLocation, PackageFamilyName, PackageFullName | ConvertTo-Json -Compress"#;
    
    let windir = std::env::var("SystemRoot").or_else(|_| std::env::var("windir")).unwrap_or_else(|_| "C:\\Windows".to_string());
    let powershell_path = Path::new(&windir).join("System32").join("WindowsPowerShell").join("v1.0").join("powershell.exe");

    let output = match Command::new(powershell_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-Command", script])
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
            Some(pf) => {
                if pf.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-') {
                    pf
                } else {
                    continue; // Skip invalid or malicious PackageFullName
                }
            }
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

        let icon_base64 = get_uwp_icon_base64(&loc);

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
            icon_base64,
            estimated_size: None, // Can calculate size from directory later
            registry_path: format!(r"UWP\{}", package_family),
            hive: "UWP".to_string(),
            is_uwp: true,
            is_verified: Some(true),
        });
    }

    apps
}

fn clean_uwp_display_name(name: &str) -> String {
    let mut clean = name.to_string();
    
    // List of common publisher prefixes to strip
    let prefixes = [
        "MicrosoftCorporationII.",
        "Microsoft.Windows.",
        "MicrosoftWindows.",
        "Microsoft.",
        "System.",
        "PythonSoftwareFoundation.",
    ];
    for prefix in &prefixes {
        if clean.starts_with(prefix) {
            clean = clean.replacen(prefix, "", 1);
            break;
        }
    }
    
    // Also if the name is publisher.appName where appName is similar (e.g., Clipchamp.Clipchamp -> Clipchamp)
    if let Some(dot_idx) = clean.find('.') {
        let (pub_part, app_part) = clean.split_at(dot_idx);
        let app_part_clean = &app_part[1..]; // skip the dot
        if pub_part.eq_ignore_ascii_case(app_part_clean) {
            clean = app_part_clean.to_string();
        } else if pub_part == "C27EB4BA" { // Dropbox OEM prefix
            clean = app_part_clean.to_string();
        }
    }
    
    // Replace remaining dots with spaces, but keep them if they are part of version numbers like "3.13" or "1.8"
    let mut dot_cleaned = String::new();
    let chars: Vec<char> = clean.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if c == '.' {
            // Check if it's between digits
            let is_version_dot = i > 0 && i < chars.len() - 1 
                && chars[i - 1].is_ascii_digit() 
                && chars[i + 1].is_ascii_digit();
            if is_version_dot {
                dot_cleaned.push('.');
            } else {
                dot_cleaned.push(' ');
            }
        } else {
            dot_cleaned.push(c);
        }
    }
    clean = dot_cleaned;
    
    // Space out CamelCase names (e.g., WindowsStore -> Windows Store)
    let mut spaced = String::new();
    let chars: Vec<char> = clean.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if i > 0 && c.is_uppercase() {
            if let Some(prev) = chars.get(i - 1) {
                // Add space if preceding char is not a space/dash/dot AND not an uppercase letter
                // (so we don't space out acronyms like HEIF or OEM or VP9)
                if *prev != ' ' && *prev != '-' && *prev != '.' && !prev.is_uppercase() && !prev.is_ascii_digit() {
                    spaced.push(' ');
                }
            }
        }
        spaced.push(c);
    }
    
    // Collapse multiple spaces and trim
    let mut final_name = String::new();
    for word in spaced.split_whitespace() {
        if !final_name.is_empty() {
            final_name.push(' ');
        }
        final_name.push_str(word);
    }
    
    final_name
}

fn get_uwp_icon_base64(install_location: &str) -> Option<String> {
    let path = Path::new(install_location);
    if !path.exists() {
        return None;
    }
    
    // We walk up to depth 3 recursively to find png assets
    let mut candidates = Vec::new();
    for entry in walkdir::WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png")) {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase()) {
                let is_high_contrast = name.contains("contrast-black") || name.contains("contrast-white") 
                    || name.contains("hc-black") || name.contains("hc-white");
                let is_altform = name.contains("altform");
                
                let mut score = if name.contains("storelogo") || name.contains("store_logo") || name.contains("storestorelogo") {
                    100
                } else if name.contains("square150x150logo") || name.contains("square150") {
                    80
                } else if name.contains("square44x44logo") || name.contains("square44") {
                    60
                } else if name.contains("targetsize-44") || name.contains("targetsize-48") || name.contains("targetsize-256") {
                    50
                } else if name.contains("logo") {
                    30
                } else if name.contains("icon") {
                    20
                } else {
                    10
                };
                
                if is_high_contrast {
                    score -= 40;
                }
                if is_altform {
                    score -= 10;
                }
                
                candidates.push((score, p.to_path_buf()));
            }
        }
    }
    
    candidates.sort_by_key(|b| std::cmp::Reverse(b.0));
    if let Some((_, best_path)) = candidates.first() {
        if let Ok(bytes) = fs::read(best_path) {
            return Some(general_purpose::STANDARD.encode(&bytes));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_uwp_apps() {
        let apps = scan_uwp_apps();
        println!("Found {} UWP apps", apps.len());
        for app in &apps {
            println!("App: {} ({}) - {}", app.display_name, app.id, app.install_location.as_deref().unwrap_or(""));
        }
        assert!(!apps.is_empty(), "UWP apps list should not be empty on Windows 10/11");
    }
}

