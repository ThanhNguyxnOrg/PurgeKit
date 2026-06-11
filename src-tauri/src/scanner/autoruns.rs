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

            let cleaned_val = val.trim().trim_matches('"').to_string();
            // Extract executable path (strip arguments)
            let exe_path = if let Some(first_space) = cleaned_val.find(' ') {
                if cleaned_val.starts_with('"') {
                    // It starts with quote, should have closed quote
                    cleaned_val.trim_matches('"').to_string()
                } else {
                    cleaned_val[..first_space].to_string()
                }
            } else {
                cleaned_val
            };

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
    let app_token_lower = app_token.to_lowercase();
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let services_path = r"SYSTEM\CurrentControlSet\Services";
    let services_key = match root.open_subkey_with_flags(services_path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    for service_name in services_key.enum_keys().filter_map(|x| x.ok()) {
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

        let cleaned_path = image_path.trim().trim_matches('"').to_string();
        let exe_path = if cleaned_path.starts_with(r"\SystemRoot\") {
            cleaned_path.replacen(r"\SystemRoot\", &std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string()), 1)
        } else {
            cleaned_path
        };

        // Strip arguments
        let exe_path = if let Some(pos) = exe_path.find(" -") {
            exe_path[..pos].trim().to_string()
        } else if let Some(pos) = exe_path.find(" /") {
            exe_path[..pos].trim().to_string()
        } else {
            exe_path
        };

        let expanded = crate::winutil::expand_env_strings(&exe_path);
        let path_lower = expanded.to_lowercase();
        let service_name_lower = service_name.to_lowercase();

        let mut is_match = false;
        let mut score = 60;

        if let Some(loc) = install_dir {
            let loc_lower = loc.to_lowercase();
            if !loc_lower.is_empty() && path_lower.starts_with(&loc_lower) {
                is_match = true;
                score = 90;
            }
        }

        if !is_match && (service_name_lower.contains(&app_token_lower) || path_lower.contains(&app_token_lower)) {
            is_match = true;
            score = 75;
        }

        if is_match {
            let file_exists = Path::new(&expanded).exists();
            if !file_exists {
                score += 10; // Orphaned service (binary missing) -> boost score
            }

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
    let app_token_lower = app_token.to_lowercase();
    
    // Scheduled tasks files are stored in C:\Windows\System32\Tasks
    let windir = std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string());
    let tasks_dir = Path::new(&windir).join("System32").join("Tasks");
    
    if !tasks_dir.exists() {
        return;
    }

    // Read tasks directory recursively
    let entries = match fs::read_dir(&tasks_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let task_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let xml_content = match fs::read_to_string(&path) {
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
                        let cmd_tag = &xml_content[cmd_start + 9 .. cmd_start + cmd_end];
                        let cleaned_cmd = cmd_tag.trim().trim_matches('"');
                        let expanded_cmd = crate::winutil::expand_env_strings(cleaned_cmd);
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
                    item_type: "File".to_string(),
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

