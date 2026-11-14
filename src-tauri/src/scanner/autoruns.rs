use std::fs;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;
use crate::scanner::RemnantItem;

pub fn scan_aseps(app_token: &str, install_dir: Option<&str>) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();


    // 1. Scan Startup Run / RunOnce keys
    scan_startup_keys(app_token, install_dir, &mut remnants);

    // 2. Scan Services and Kernel Drivers
    scan_services(app_token, install_dir, &mut remnants);

    // 3. Scan Scheduled Tasks XML files
    scan_scheduled_tasks(app_token, install_dir, &mut remnants);

    // 4. Scan Firewall Rules
    scan_firewall_rules(app_token, install_dir, &mut remnants);

    // 5. Scan Active Setup installed components
    scan_active_setup(app_token, install_dir, &mut remnants);

    // 6. Scan Winlogon shell / userinit overrides
    scan_winlogon(app_token, install_dir, &mut remnants);

    // 7. Scan AppInit_DLLs
    scan_appinit_dlls(app_token, install_dir, &mut remnants);

    // 8. Scan Boot Execute entries
    scan_boot_execute(app_token, install_dir, &mut remnants);

    remnants
}

fn scan_startup_keys(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let app_token_lower = app_token.to_lowercase();
    let run_locations = vec![
        (HKEY_CURRENT_USER, "HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_CURRENT_USER, "HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
    ];

    for (hive, hive_name, subpath) in run_locations {
        let root = RegKey::predef(hive);
        let run_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for name in run_key.enum_values().filter_map(|x| x.ok()).map(|(name, _)| name) {
            let val: String = match run_key.get_value(&name) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let exe_path = crate::winutil::extract_executable_path(&val);

            let expanded = crate::winutil::expand_env_strings(&exe_path);
            let path_lower = expanded.to_lowercase();
            
            let mut is_match = false;
            let mut score = 55;

            if let Some(loc) = install_dir {
                let loc_lower = loc.to_lowercase();
                if !loc_lower.is_empty() && path_lower.starts_with(&loc_lower) {
                    is_match = true;
                    score = 85;
                }
            }

            if !is_match && (name.to_lowercase().contains(&app_token_lower) || path_lower.contains(&app_token_lower)) {
                is_match = true;
                score = 70;
            }

            if is_match {
                remnants.push(RemnantItem {
                    path: format!(r"{}\{}\{}", hive_name, subpath, name),
                    item_type: "RegistryValue".to_string(), // The run value itself
                    size: 0,
                    confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                    score,
                });
            }
        }
    }
}

fn scan_services(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let app_token_lower = app_token.to_lowercase().trim().to_string();
    if app_token_lower.len() < 3 {
        return;
    }

    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let services_path = r"SYSTEM\CurrentControlSet\Services";
    let services_key = match root.open_subkey_with_flags(services_path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    let windir = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string())
        .to_lowercase();

    for service_name in services_key.enum_keys().filter_map(|x| x.ok()) {
        let service_name_lower = service_name.to_lowercase();

        // 1. Safety check: NEVER scan or target critical core Windows services
        if crate::winutil::CRITICAL_WINDOWS_SERVICES.contains(&service_name_lower.as_str()) {
            continue;
        }

        let service_key_path = format!(r"{}\{}", services_path, service_name);
        let service_key = match services_key.open_subkey_with_flags(&service_name, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        // Services store executable path in ImagePath
        let image_path: String = match service_key.get_value("ImagePath") {
            Ok(p) => p,
            Err(_) => continue,
        };

        let resolved_path = if image_path.trim_start_matches('"').starts_with(r"\SystemRoot\") {
            let sys_root = std::env::var("SystemRoot")
                .or_else(|_| std::env::var("windir"))
                .unwrap_or_else(|_| "C:\\Windows".to_string());
            image_path.replacen(r"\SystemRoot\", &sys_root, 1)
        } else {
            image_path
        };
        let exe_path = crate::winutil::extract_executable_path(&resolved_path);

        let expanded = crate::winutil::expand_env_strings(&exe_path);
        let path_lower = expanded.to_lowercase();

        // 2. Safety check: if binary resides in Windows System32 / Windows dir and not in install_dir, skip
        if (path_lower.starts_with(&windir) || path_lower.starts_with(r"c:\windows")) && install_dir.is_none() {
            continue;
        }

        let file_exists = Path::new(&expanded).exists();

        let mut is_match = false;
        let mut score = 60;

        // Match Strategy A: ImagePath is inside the uninstalled application directory
        if let Some(loc) = install_dir {
            let loc_lower = loc.to_lowercase();
            if !loc_lower.is_empty() && path_lower.starts_with(&loc_lower) {
                is_match = true;
                score = 90;
            }
        }

        // Match Strategy B: Binary does NOT exist (orphaned service) AND path contains app_token
        if !is_match && !file_exists && path_lower.contains(&app_token_lower) {
            is_match = true;
            score = 80;
        }

        if is_match {
            remnants.push(RemnantItem {
                path: format!(r"HKLM\{}", service_key_path),
                item_type: "RegistryKey".to_string(),
                size: 0,
                confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                score,
            });
        }
    }
}

fn scan_scheduled_tasks(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    // Scheduled tasks files are stored in %SystemRoot%\System32\Tasks
    let windir = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    
    if tasks_dir.exists() {
        scan_tasks_dir_recursive(&tasks_dir, app_token, install_dir, remnants, 0);
    }
}

fn scan_tasks_dir_recursive(
    dir: &Path,
    app_token: &str,
    install_dir: Option<&str>,
    remnants: &mut Vec<RemnantItem>,
    depth: u32,
) {
    if depth > 4 { // Prevent infinite recursion
        return;
    }
    
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let app_token_lower = app_token.to_lowercase();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            scan_tasks_dir_recursive(&path, app_token, install_dir, remnants, depth + 1);
        } else if path.is_file() {
            let task_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let xml_content = match crate::winutil::read_file_utf8_or_utf16(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let xml_lower = xml_content.to_lowercase();
            let task_name_lower = task_name.to_lowercase();

            let mut is_match = false;
            let mut score = 55;

            if let Some(loc) = install_dir {
                let loc_lower = loc.to_lowercase();
                if !loc_lower.is_empty() && xml_lower.contains(&loc_lower) {
                    is_match = true;
                    score = 85;
                }
            }

            if !is_match && (task_name_lower.contains(&app_token_lower) || xml_lower.contains(&app_token_lower)) {
                is_match = true;
                score = 70;
            }

            if is_match {
                // Check if binaries mentioned in XML <Command> tags exist
                let mut binary_exists = true;
                if let Some(cmd_start) = xml_lower.find("<command>") {
                    if let Some(cmd_end) = xml_lower[cmd_start..].find("</command>") {
                        // Slicing xml_lower instead of xml_content ensures we slice using safe indices
                        let cleaned_cmd = xml_lower[cmd_start + 9 .. cmd_start + cmd_end].trim();
                        let exe = crate::winutil::extract_executable_path(cleaned_cmd);
                        let expanded_cmd = crate::winutil::expand_env_strings(&exe);
                        if !expanded_cmd.is_empty() && !Path::new(&expanded_cmd).exists() {
                            binary_exists = false;
                        }
                    }
                }

                if !binary_exists {
                    score += 10; // Orphaned scheduled task
                }

                remnants.push(RemnantItem {
                    path: path.to_string_lossy().to_string(),
                    item_type: "ScheduledTask".to_string(),
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                    confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                    score,
                });
            }
        }
    }
}

fn scan_firewall_rules(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let app_token_lower = app_token.to_lowercase();
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let fw_path = r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules";
    let fw_key = match root.open_subkey_with_flags(fw_path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    for name in fw_key.enum_values().filter_map(|x| x.ok()).map(|(n, _)| n) {
        let val: String = match fw_key.get_value(&name) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Firewall rules are strings like: "v2.10|Action=Allow|Active=TRUE|Dir=In|App=C:\Program Files\App\app.exe|..."
        let val_lower = val.to_lowercase();
        let name_lower = name.to_lowercase();

        let mut is_match = false;
        let mut score = 50;

        if let Some(loc) = install_dir {
            let loc_lower = loc.to_lowercase();
            if !loc_lower.is_empty() && val_lower.contains(&loc_lower) {
                is_match = true;
                score = 80;
            }
        }

        if !is_match && (name_lower.contains(&app_token_lower) || val_lower.contains(&app_token_lower)) {
            is_match = true;
            score = 65;
        }

        if is_match {
            remnants.push(RemnantItem {
                path: format!(r"HKLM\{}\{}", fw_path, name),
                item_type: "RegistryValue".to_string(),
                size: 0,
                confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                score,
            });
        }
    }
}

/// Shared matcher: returns a confidence score when `candidate` references the app,
/// either through its install directory (higher confidence) or its name token.
/// The name token must be at least 3 characters to avoid false positives.
fn remnant_score(
    app_token: &str,
    install_dir: Option<&str>,
    candidate: &str,
    dir_score: u32,
    token_score: u32,
) -> Option<u32> {
    let candidate_lower = candidate.to_lowercase();

    if let Some(loc) = install_dir {
        let loc_lower = loc.to_lowercase();
        if !loc_lower.is_empty() && candidate_lower.contains(&loc_lower) {
            return Some(dir_score);
        }
    }

    let token_lower = app_token.to_lowercase().trim().to_string();
    if token_lower.len() >= 3 && candidate_lower.contains(&token_lower) {
        return Some(token_score);
    }

    None
}

/// Splits an `AppInit_DLLs` value (semicolon and/or whitespace separated) into DLL entries.
fn parse_appinit_dlls(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ';' || c.is_whitespace())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Returns the best confidence score when any `BootExecute` entry references the app.
fn boot_execute_match(entries: &[String], app_token: &str, install_dir: Option<&str>) -> Option<u32> {
    entries
        .iter()
        .filter_map(|entry| remnant_score(app_token, install_dir, entry, 85, 70))
        .max()
}

/// Scans Active Setup installed components. Installers register a `StubPath` command here
/// that is executed once per user on first logon, and uninstallers frequently leave the
/// component key behind.
fn scan_active_setup(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let locations = vec![
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Active Setup\Installed Components"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Active Setup\Installed Components"),
    ];

    for (hive, hive_name, subpath) in locations {
        let root = RegKey::predef(hive);
        let components_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for component_id in components_key.enum_keys().filter_map(|x| x.ok()) {
            let component_key = match components_key.open_subkey_with_flags(&component_id, KEY_READ) {
                Ok(k) => k,
                Err(_) => continue,
            };

            let stub_path: String = match component_key.get_value("StubPath") {
                Ok(v) => v,
                Err(_) => continue,
            };

            let expanded = crate::winutil::expand_env_strings(&stub_path);
            if let Some(score) = remnant_score(app_token, install_dir, &expanded, 85, 70) {
                remnants.push(RemnantItem {
                    path: format!(r"{}\{}\{}", hive_name, subpath, component_id),
                    item_type: "RegistryKey".to_string(),
                    size: 0,
                    confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                    score,
                });
            }
        }
    }
}

/// Scans Winlogon `Shell` / `Userinit` / `TaskMan` / `AppSetup` overrides. Applications that
/// replace or extend these values leave logon-time execution behind after uninstall.
fn scan_winlogon(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let locations = vec![
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows NT\CurrentVersion\Winlogon"),
    ];
    let value_names = ["Shell", "Userinit", "TaskMan", "AppSetup"];

    for (hive, hive_name, subpath) in locations {
        let root = RegKey::predef(hive);
        let winlogon_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for value_name in value_names {
            let value: String = match winlogon_key.get_value(value_name) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let expanded = crate::winutil::expand_env_strings(&value);
            if let Some(score) = remnant_score(app_token, install_dir, &expanded, 85, 70) {
                remnants.push(RemnantItem {
                    path: format!(r"{}\{}\{}", hive_name, subpath, value_name),
                    item_type: "RegistryValue".to_string(),
                    size: 0,
                    confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                    score,
                });
            }
        }
    }
}

/// Scans `AppInit_DLLs`. The whole value is reported (and backed up) when any listed DLL
/// references the app; modern Windows only honors it when `LoadAppInit_DLLs` is enabled.
fn scan_appinit_dlls(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let locations = vec![
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Wow6432Node\Microsoft\Windows NT\CurrentVersion\Windows"),
    ];

    for (hive, hive_name, subpath) in locations {
        let root = RegKey::predef(hive);
        let windows_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let raw: String = match windows_key.get_value("AppInit_DLLs") {
            Ok(v) => v,
            Err(_) => continue,
        };

        let dlls = parse_appinit_dlls(&raw);
        let best_score = dlls
            .iter()
            .filter_map(|dll| remnant_score(app_token, install_dir, dll, 85, 70))
            .max();

        if let Some(score) = best_score {
            remnants.push(RemnantItem {
                path: format!(r"{}\{}\AppInit_DLLs", hive_name, subpath),
                item_type: "RegistryValue".to_string(),
                size: 0,
                confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
                score,
            });
        }
    }
}

/// Scans `BootExecute` (Session Manager, REG_MULTI_SZ). Default entries such as
/// `autocheck autochk *` never match, so only app-owned boot commands surface here.
fn scan_boot_execute(app_token: &str, install_dir: Option<&str>, remnants: &mut Vec<RemnantItem>) {
    let subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager";
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let session_key = match root.open_subkey_with_flags(subpath, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    let entries: Vec<String> = match session_key.get_value("BootExecute") {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(score) = boot_execute_match(&entries, app_token, install_dir) {
        remnants.push(RemnantItem {
            path: format!(r"HKLM\{}\BootExecute", subpath),
            item_type: "RegistryValue".to_string(),
            size: 0,
            confidence: if score >= 80 { "VeryHigh".to_string() } else { "High".to_string() },
            score,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_tasks_dir_recursive() {
        let temp_dir = std::env::temp_dir().join("purgekit_test_autoruns");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let task_xml = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Actions Context="Author">
    <Exec>
      <Command>"C:\Program Files\MyCoolApp\app.exe"</Command>
      <Arguments>--silent</Arguments>
    </Exec>
  </Actions>
</Task>"#;

        let task_file = temp_dir.join("MyCoolAppTask");
        fs::write(&task_file, task_xml).unwrap();

        let mut remnants = Vec::new();
        scan_tasks_dir_recursive(&temp_dir, "MyCoolApp", Some(r"C:\Program Files\MyCoolApp"), &mut remnants, 0);

        let _ = fs::remove_dir_all(&temp_dir);

        assert!(!remnants.is_empty(), "Should find the mock task by install dir");
        let item = &remnants[0];
        assert_eq!(item.item_type, "ScheduledTask");
        assert!(item.path.contains("MyCoolAppTask"));
        assert!(item.score >= 80);
    }

    #[test]
    fn test_remnant_score_prefers_install_dir_and_guards_short_tokens() {
        assert_eq!(
            remnant_score(
                "MyApp",
                Some(r"C:\Program Files\MyApp"),
                r"C:\Program Files\MyApp\svc.exe",
                85,
                70
            ),
            Some(85)
        );
        assert_eq!(remnant_score("MyApp", None, r"C:\tools\myapp\svc.exe", 85, 70), Some(70));
        assert_eq!(remnant_score("MyApp", Some(r"D:\Other"), "explorer.exe", 85, 70), None);
        // Tokens shorter than 3 characters must never match by name.
        assert_eq!(remnant_score("ab", None, "ab.dll", 85, 70), None);
    }

    #[test]
    fn test_parse_appinit_dlls() {
        let dlls = parse_appinit_dlls(" myapp.dll;helper.dll  another.dll ");
        assert_eq!(dlls, vec!["myapp.dll", "helper.dll", "another.dll"]);
        assert!(parse_appinit_dlls("   ").is_empty());
    }

    #[test]
    fn test_boot_execute_match() {
        let defaults = vec!["autocheck autochk *".to_string()];
        assert_eq!(boot_execute_match(&defaults, "MyApp", Some(r"C:\Program Files\MyApp")), None);

        let entries = vec![
            "autocheck autochk *".to_string(),
            r"C:\Program Files\MyApp\boot.exe".to_string(),
        ];
        assert_eq!(boot_execute_match(&entries, "MyApp", Some(r"C:\Program Files\MyApp")), Some(85));
    }
}

