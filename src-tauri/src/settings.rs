use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub enable_undocumented_force_unlock: bool,
    pub scan_level: String, // "safe" | "moderate" | "aggressive"
    pub backup_before_delete: bool,
    pub create_restore_point: bool,
    #[serde(default)]
    pub project_scan_paths: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enable_undocumented_force_unlock: false,
            scan_level: "safe".to_string(),
            backup_before_delete: true,
            create_restore_point: true,
            project_scan_paths: Vec::new(),
        }
    }
}

fn get_settings_path() -> PathBuf {
    let base_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Local"))
                .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
        });
    
    let app_dir = base_dir.join("PurgeKit");
    let _ = fs::create_dir_all(&app_dir);
    app_dir.join("settings.json")
}

pub fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if !path.exists() {
        return AppSettings::default();
    }
    
    let mut settings = fs::read_to_string(path)
        .and_then(|content| serde_json::from_str::<AppSettings>(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
        .unwrap_or_else(|_| AppSettings::default());

    // Validate scan_level and fallback if invalid
    if settings.scan_level != "safe" && settings.scan_level != "moderate" && settings.scan_level != "aggressive" {
        settings.scan_level = "safe".to_string();
    }

    // Machine-wide registry overrides win over the user's settings.json.
    apply_overrides(&mut settings, &read_registry_overrides());

    settings
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    // Write a sibling temp file first so a crash cannot leave truncated JSON
    // that silently falls back to defaults on the next launch.
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, content)
        .map_err(|e| format!("Failed to write settings temp file: {}", e))?;

    if fs::rename(&temp_path, &path).is_err() {
        // Windows does not replace an existing file with rename.
        let _ = fs::remove_file(&path);
        fs::rename(&temp_path, &path)
            .map_err(|e| format!("Failed to replace settings file: {}", e))?;
    }

    Ok(())
}

pub fn check_is_admin() -> bool {
    is_elevated::is_elevated()
}

/// Machine-wide overrides, WizTree style: administrators can pin settings
/// through `HKLM\SOFTWARE\PurgeKit\ConfigOverride` (REG_SZ values named after
/// the fields below). Overrides are applied on every load and win over
/// settings.json, so a deployment can enforce policy without blocking the
/// user's own settings file.
fn read_registry_overrides() -> HashMap<String, String> {
    let mut overrides = HashMap::new();
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = match root.open_subkey_with_flags(r"SOFTWARE\PurgeKit\ConfigOverride", KEY_READ) {
        Ok(key) => key,
        Err(_) => return overrides,
    };

    for (name, _) in key.enum_values().filter_map(|value| value.ok()) {
        if let Ok(value) = key.get_value::<String, _>(&name) {
            overrides.insert(name.to_lowercase(), value);
        }
    }

    overrides
}

fn parse_override_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Applies override values on top of `settings`; unknown keys are ignored and
/// invalid values keep the previous field value.
fn apply_overrides(settings: &mut AppSettings, overrides: &HashMap<String, String>) {
    if let Some(value) = overrides
        .get("enable_undocumented_force_unlock")
        .and_then(|raw| parse_override_bool(raw))
    {
        settings.enable_undocumented_force_unlock = value;
    }

    if let Some(value) = overrides.get("scan_level") {
        if value == "safe" || value == "moderate" || value == "aggressive" {
            settings.scan_level = value.clone();
        }
    }

    if let Some(value) = overrides
        .get("backup_before_delete")
        .and_then(|raw| parse_override_bool(raw))
    {
        settings.backup_before_delete = value;
    }

    if let Some(value) = overrides
        .get("create_restore_point")
        .and_then(|raw| parse_override_bool(raw))
    {
        settings.create_restore_point = value;
    }

    if let Some(value) = overrides.get("project_scan_paths") {
        let paths: Vec<String> = value
            .split(';')
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        if !paths.is_empty() {
            settings.project_scan_paths = paths;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let default_settings = AppSettings::default();
        assert_eq!(default_settings.enable_undocumented_force_unlock, false);
        assert_eq!(default_settings.scan_level, "safe");
        assert_eq!(default_settings.backup_before_delete, true);
        assert_eq!(default_settings.create_restore_point, true);
        assert!(default_settings.project_scan_paths.is_empty());
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings {
            enable_undocumented_force_unlock: true,
            scan_level: "aggressive".to_string(),
            backup_before_delete: false,
            create_restore_point: false,
            project_scan_paths: vec!["D:\\Code".to_string()],
        };
        let serialized = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.enable_undocumented_force_unlock, true);
        assert_eq!(deserialized.scan_level, "aggressive");
        assert_eq!(deserialized.backup_before_delete, false);
        assert_eq!(deserialized.create_restore_point, false);
        assert_eq!(deserialized.project_scan_paths, vec!["D:\\Code".to_string()]);
    }

    #[test]
    fn registry_overrides_win_over_file_settings() {
        let mut settings = AppSettings::default();
        let mut overrides = HashMap::new();
        overrides.insert("scan_level".to_string(), "aggressive".to_string());
        overrides.insert("backup_before_delete".to_string(), "no".to_string());
        overrides.insert("project_scan_paths".to_string(), r"D:\Code; E:\Repos ;".to_string());
        overrides.insert("enable_undocumented_force_unlock".to_string(), "on".to_string());
        overrides.insert("some_unknown_key".to_string(), "whatever".to_string());

        apply_overrides(&mut settings, &overrides);

        assert_eq!(settings.scan_level, "aggressive");
        assert!(!settings.backup_before_delete);
        assert!(settings.enable_undocumented_force_unlock);
        assert_eq!(
            settings.project_scan_paths,
            vec![r"D:\Code".to_string(), r"E:\Repos".to_string()]
        );
    }

    #[test]
    fn invalid_overrides_are_ignored() {
        let mut settings = AppSettings::default();
        let mut overrides = HashMap::new();
        overrides.insert("scan_level".to_string(), "turbo".to_string());
        overrides.insert("backup_before_delete".to_string(), "maybe".to_string());

        apply_overrides(&mut settings, &overrides);

        assert_eq!(settings.scan_level, "safe");
        assert!(settings.backup_before_delete);
    }
}
