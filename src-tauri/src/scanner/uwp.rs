use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::fs;
use std::path::{Path, PathBuf};
use super::registry::InstalledApp;
use base64::{Engine as _, engine::general_purpose};

// Hiding console window flag for Windows process execution
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UwpStartApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "AppID")]
    app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct UwpScanOutput {
    #[serde(default)]
    packages: Vec<UwpRawApp>,
    #[serde(default, rename = "startApps")]
    start_apps: Vec<UwpStartApp>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UwpRawApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "PublisherId")]
    publisher_id: Option<String>,
    #[serde(rename = "PublisherDisplayName", default)]
    publisher_display_name: Option<String>,
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
    let script = r#"$apps = try { Get-AppxPackage -AllUsers -ErrorAction Stop } catch { Get-AppxPackage };
$packages = @($apps | Where-Object { -not $_.IsFramework -and $_.NonRemovable -ne $true } | Select-Object Name, PublisherId, PublisherDisplayName, Version, InstallLocation, PackageFamilyName, PackageFullName);
$startApps = @();
try { $startApps = @(Get-StartApps | Select-Object Name, AppID) } catch {};
[pscustomobject]@{ packages = $packages; startApps = $startApps } | ConvertTo-Json -Compress -Depth 4"#;
    
    let windir = std::env::var("SystemRoot").or_else(|_| std::env::var("windir")).unwrap_or_else(|_| "C:\\Windows".to_string());
    let powershell_path = Path::new(&windir).join("System32").join("WindowsPowerShell").join("v1.0").join("powershell.exe");
    let secure_path = crate::winutil::get_secure_system_path();
    let output = match Command::new(powershell_path)
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
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

    let scan: UwpScanOutput = serde_json::from_str(stdout_str).unwrap_or_default();
    let mut start_names: HashMap<String, String> = HashMap::new();
    for start_app in scan.start_apps {
        let name = start_app.name.trim();
        if name.is_empty() || name.to_lowercase().starts_with("ms-resource:") {
            continue;
        }
        if let Some((family, _)) = start_app.app_id.split_once('!') {
            start_names
                .entry(family.to_lowercase())
                .or_insert_with(|| name.to_string());
        }
    }

    for raw in scan.packages {
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

        // Prefer the localized Start menu name; package identifiers are only
        // a fallback for background packages without a Start entry.
        let display_name = start_names
            .get(&package_family.to_lowercase())
            .cloned()
            .unwrap_or_else(|| clean_uwp_display_name(&name));

        // UWP Uninstall command: Remove-AppxPackage -Package <PackageFullName>.
        // When elevated we queried every user, so remove every registration too;
        // otherwise the app would reappear after switching accounts.
        let all_users = if is_elevated::is_elevated() {
            " -AllUsers"
        } else {
            ""
        };
        let uninstall_string = format!(
            "powershell -NoProfile -Command \"Remove-AppxPackage{} -Package {}\"",
            all_users, package_full
        );

        let icon_base64 = get_uwp_icon_base64(&loc);
        let install_date = crate::scanner::registry::path_created_date(&loc);

        apps.push(InstalledApp {
            id: package_family.clone(),
            display_name,
            display_version: raw.version.clone(),
            publisher: raw
                .publisher_display_name
                .clone()
                .filter(|name| {
                    let name = name.trim();
                    !name.is_empty() && !name.to_lowercase().starts_with("ms-resource:")
                })
                .or_else(|| raw.publisher_id.clone()),
            uninstall_string: Some(uninstall_string.clone()),
            quiet_uninstall_string: Some(uninstall_string),
            modify_path: None,
            release_type: None,
            install_location: Some(loc),
            install_date,
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

fn png_has_visible_pixels(bytes: &[u8]) -> bool {
    let Ok(image) = image::load_from_memory_with_format(bytes, image::ImageFormat::Png) else {
        return false;
    };
    image
        .to_rgba8()
        .pixels()
        .any(|pixel| pixel[3] > 8)
}

fn manifest_logo_candidates(install_location: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(install_location.join("AppxManifest.xml")) else {
        return Vec::new();
    };

    let mut candidates: Vec<String> = Vec::new();
    let mut push = |value: &str| {
        let value = value.trim();
        if !value.is_empty()
            && !candidates
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(value))
        {
            candidates.push(value.to_string());
        }
    };

    for attribute in ["Square44x44Logo", "Square150x150Logo", "Logo"] {
        let needle = format!("{}=\"", attribute);
        let mut rest = content.as_str();
        while let Some(index) = rest.find(&needle) {
            let after = &rest[index + needle.len()..];
            let Some(end) = after.find('"') else {
                break;
            };
            push(&after[..end]);
            rest = &after[end + 1..];
        }
    }

    if let Some(start) = content.find("<Logo>") {
        let after = &content[start + "<Logo>".len()..];
        if let Some(end) = after.find("</Logo>") {
            push(&after[..end]);
        }
    }

    candidates
}

fn icon_score(name: &str) -> u32 {
    if name.contains("contrast-") {
        return 0;
    }
    let score: u32 = if name.contains("targetsize-256") {
        100
    } else if name.contains("targetsize-48") {
        90
    } else if name.contains("targetsize-44") {
        80
    } else if name.contains("scale-200") {
        70
    } else if name.contains("scale-150") {
        60
    } else if name.contains("scale-125") {
        50
    } else if name.contains("scale-100") {
        40
    } else {
        10
    };
    if name.contains("altform") {
        score.saturating_sub(15)
    } else {
        score
    }
}

fn resolve_manifest_asset(install_location: &Path, raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() || raw.to_ascii_lowercase().starts_with("ms-resource:") {
        return None;
    }

    let relative = raw.replace('/', "\\");
    let relative = relative.trim_start_matches('\\');
    let base = crate::winutil::canonicalize_path_safety(&install_location.to_string_lossy());
    let candidate = crate::winutil::canonicalize_path_safety(
        &install_location.join(relative).to_string_lossy(),
    );
    if !candidate.starts_with(&base) {
        return None;
    }
    if candidate.is_file()
        && candidate
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return Some(candidate);
    }

    // Manifests usually name an unqualified asset while the package ships
    // scale/targetsize variants of it.
    let parent = candidate.parent()?;
    let stem = candidate.file_stem()?.to_string_lossy().to_ascii_lowercase();
    if stem.is_empty() {
        return None;
    }

    let mut best: Option<(u32, PathBuf)> = None;
    for entry in fs::read_dir(parent).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".png") || !name.starts_with(&stem) {
            continue;
        }
        let remainder = &name[stem.len()..];
        if !remainder.is_empty() && !remainder.starts_with('.') && !remainder.starts_with('_') {
            continue;
        }
        let score = icon_score(&name);
        if score == 0 {
            continue;
        }
        if best.as_ref().map_or(true, |(best_score, _)| score > *best_score) {
            best = Some((score, path));
        }
    }

    best.map(|(_, path)| path)
}

fn manifest_executable(install_location: &Path) -> Option<PathBuf> {
    let manifest = fs::read_to_string(install_location.join("AppxManifest.xml")).ok()?;
    let base = crate::winutil::canonicalize_path_safety(&install_location.to_string_lossy());
    let marker = "Executable=\"";
    let mut rest = manifest.as_str();
    while let Some(start) = rest.find(marker) {
        rest = &rest[start + marker.len()..];
        let Some(end) = rest.find('"') else {
            break;
        };
        let candidate = install_location.join(rest[..end].replace('/', "\\"));
        let candidate = crate::winutil::canonicalize_path_safety(&candidate.to_string_lossy());
        if candidate.starts_with(&base)
            && candidate.is_file()
            && candidate
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Some(candidate);
        }
    }
    None
}

fn get_uwp_icon_base64(install_location: &str) -> Option<String> {
    let path = Path::new(install_location);
    if !path.is_dir() {
        return None;
    }

    for logo in manifest_logo_candidates(path) {
        let Some(asset) = resolve_manifest_asset(path, &logo) else {
            continue;
        };
        if let Ok(bytes) = fs::read(asset) {
            if !bytes.is_empty() && png_has_visible_pixels(&bytes) {
                return Some(general_purpose::STANDARD.encode(&bytes));
            }
        }
    }

    // Some manifests use ms-resource logos or omit the asset. The application
    // executable still carries the package icon in most of those packages.
    if let Some(executable) = manifest_executable(path) {
        if let Ok(base64) = windows_icons::get_icon_base64_by_path(&executable) {
            return Some(base64);
        }
    }

    // Manifest paths can be resource references; fall back to a bounded walk
    // inside the package so a huge or malicious package cannot escape it.
    let mut candidates = Vec::new();
    for entry in walkdir::WalkDir::new(path)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .take(20_000)
    {
        let p = entry.path();
        let is_image = p.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("ico")
        });
        let is_plain_file = fs::symlink_metadata(p)
            .map(|metadata| metadata.is_file() && !crate::winutil::is_reparse_point(&metadata))
            .unwrap_or(false);
        if is_plain_file && is_image {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase()) {
                let is_high_contrast = name.contains("contrast-black") || name.contains("contrast-white") 
                    || name.contains("hc-black") || name.contains("hc-white");
                let is_altform = name.contains("altform");
                
                let mut score: u32 = if name.contains("storelogo") || name.contains("store_logo") || name.contains("storestorelogo") {
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
                if name.ends_with(".ico") {
                    score = score.saturating_sub(5);
                }
                
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
        if let Some(encoded) = encode_package_icon(best_path) {
            return Some(encoded);
        }
    }

    // Stub/streamed Store packages can have no staged logo asset. windows-icons
    // resolves the shell app-exec link for those; restrict it to signed,
    // read-only WindowsApps packages so temp/test paths never reach it.
    let in_windows_apps = path
        .to_string_lossy()
        .split(['\\', '/'])
        .any(|segment| segment.eq_ignore_ascii_case("WindowsApps"));
    if in_windows_apps {
        if let Ok(base64) = windows_icons::get_icon_base64_by_path(path) {
            if !base64.is_empty() {
                if let Ok(bytes) = general_purpose::STANDARD.decode(&base64) {
                    if !png_has_visible_pixels(&bytes) {
                        return None;
                    }
                }
                return Some(base64);
            }
        }
    }
    None
}

fn encode_package_icon(path: &Path) -> Option<String> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        let bytes = fs::read(path).ok()?;
        if bytes.is_empty() || !png_has_visible_pixels(&bytes) {
            return None;
        }
        return Some(general_purpose::STANDARD.encode(&bytes));
    }
    windows_icons::get_icon_base64_by_path(path)
        .ok()
        .filter(|base64| !base64.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fully_transparent_icons() {
        use image::{ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let encode = |image: RgbaImage| {
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, ImageFormat::Png).unwrap();
            bytes.into_inner()
        };
        let transparent = encode(RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 0])));
        let opaque = encode(RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 255])));
        assert!(!png_has_visible_pixels(&transparent));
        assert!(png_has_visible_pixels(&opaque));
    }

    fn test_png(color: [u8; 4]) -> Vec<u8> {
        use image::{ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let mut bytes = Cursor::new(Vec::new());
        RgbaImage::from_pixel(8, 8, Rgba(color))
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn resolves_manifest_icon_variants() {
        let root = std::env::temp_dir().join("purgekit_uwp_icon_manifest_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Properties><Logo>Assets\StoreLogo.png</Logo></Properties><Applications><Application><uap:VisualElements Square44x44Logo="Assets\Square44x44Logo.png" /></Application></Applications></Package>"#,
        )
        .unwrap();
        let exact_icon = test_png([230, 40, 30, 255]);
        let fallback_icon = test_png([20, 120, 220, 255]);
        fs::write(
            root.join("Assets").join("Square44x44Logo.scale-100.png"),
            &exact_icon,
        )
        .unwrap();
        fs::write(root.join("Assets").join("StoreLogo.png"), &fallback_icon).unwrap();

        let encoded = get_uwp_icon_base64(root.to_str().unwrap()).expect("manifest icon");
        let decoded = general_purpose::STANDARD.decode(encoded).unwrap();
        assert_eq!(decoded, exact_icon);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolves_manifest_executable_inside_the_package() {
        let root = std::env::temp_dir().join("purgekit_uwp_executable_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin").join("app.exe"), b"exe").unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Applications><Application Executable="bin\app.exe" /></Applications></Package>"#,
        )
        .unwrap();
        assert_eq!(
            manifest_executable(&root)
                .and_then(|path| path.file_name().map(|name| name.to_owned()))
                .and_then(|name| name.to_str().map(|name| name.to_string())),
            Some("app.exe".to_string())
        );

        let outside = std::env::temp_dir().join("purgekit_uwp_executable_outside.exe");
        fs::write(&outside, b"exe").unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Applications><Application Executable="..\purgekit_uwp_executable_outside.exe" /></Applications></Package>"#,
        )
        .unwrap();
        assert!(manifest_executable(&root).is_none());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn rejects_manifest_assets_outside_the_package() {
        let base = std::env::temp_dir();
        let root = base.join("purgekit_uwp_icon_traversal_test");
        let outside = base.join("purgekit_uwp_icon_traversal_outside.png");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, [9u8, 9, 9]).unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Properties><Logo>..\purgekit_uwp_icon_traversal_outside.png</Logo></Properties></Package>"#,
        )
        .unwrap();

        assert!(get_uwp_icon_base64(root.to_str().unwrap()).is_none());

        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn manifest_logo_candidates_are_priority_ordered_and_deduplicated() {
        let root = std::env::temp_dir().join("purgekit_uwp_manifest_candidates_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package>
                <Properties><Logo>Assets\StoreLogo.png</Logo></Properties>
                <Applications>
                    <Application>
                        <uap:VisualElements Square44x44Logo="Assets\Small.png" Square150x150Logo="Assets\Wide.png" />
                        <uap:DefaultTile Square44x44Logo="Assets\Small.png" />
                    </Application>
                </Applications>
            </Package>"#,
        )
        .unwrap();

        let candidates = manifest_logo_candidates(&root);
        assert_eq!(
            candidates,
            vec![
                r"Assets\Small.png".to_string(),
                r"Assets\Wide.png".to_string(),
                r"Assets\StoreLogo.png".to_string()
            ]
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn icon_scores_prefer_large_normal_assets() {
        assert!(icon_score("square44x44logo.targetsize-256.png") > icon_score("square44x44logo.scale-200.png"));
        assert!(icon_score("square44x44logo.targetsize-48.png") > icon_score("square44x44logo.scale-100.png"));
        assert!(icon_score("square44x44logo.altform-unplated_targetsize-256.png") > icon_score("square44x44logo.scale-100.png"));
        assert_eq!(icon_score("square44x44logo.contrast-black.png"), 0);
    }

    #[test]
    fn prefers_exact_manifest_assets_and_falls_back_for_resource_logos() {
        let root = std::env::temp_dir().join("purgekit_uwp_icon_exact_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Properties><Logo>ms-resource://StoreLogo</Logo></Properties><Applications><Application><uap:VisualElements Square44x44Logo="Assets\Exact.png" /></Application></Applications></Package>"#,
        )
        .unwrap();
        let exact_icon = test_png([230, 40, 30, 255]);
        let fallback_icon = test_png([20, 120, 220, 255]);
        fs::write(root.join("Assets\\Exact.png"), &exact_icon).unwrap();
        fs::write(root.join("Assets\\Exact.scale-100.png"), &fallback_icon).unwrap();

        let exact = get_uwp_icon_base64(root.to_str().unwrap()).expect("exact icon");
        assert_eq!(
            general_purpose::STANDARD.decode(exact).unwrap(),
            exact_icon
        );

        fs::write(
            root.join("AppxManifest.xml"),
            r#"<Package><Properties><Logo>ms-resource://StoreLogo</Logo></Properties></Package>"#,
        )
        .unwrap();
        let fallback = get_uwp_icon_base64(root.to_str().unwrap()).expect("fallback icon");
        let decoded = general_purpose::STANDARD.decode(fallback).unwrap();
        assert!(!decoded.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

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

