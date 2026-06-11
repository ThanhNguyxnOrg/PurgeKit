use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub enable_undocumented_force_unlock: bool,
    pub scan_level: String, // "safe" | "moderate" | "aggressive"
    pub backup_before_delete: bool,
    pub create_restore_point: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enable_undocumented_force_unlock: false,
            scan_level: "safe".to_string(),
            backup_before_delete: true,
            create_restore_point: true,
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
    
    settings
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    fs::write(path, content)
        .map_err(|e| format!("Failed to write settings file: {}", e))
}

pub fn check_is_admin() -> bool {
    is_elevated::is_elevated()
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
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings {
            enable_undocumented_force_unlock: true,
            scan_level: "aggressive".to_string(),
            backup_before_delete: false,
            create_restore_point: false,
        };
        let serialized = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.enable_undocumented_force_unlock, true);
        assert_eq!(deserialized.scan_level, "aggressive");
        assert_eq!(deserialized.backup_before_delete, false);
        assert_eq!(deserialized.create_restore_point, false);
    }
}
