use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn shortcut_targets() -> Vec<(String, String)> {
    let script = r#"$ErrorActionPreference='SilentlyContinue';
$shell = New-Object -ComObject WScript.Shell;
$dirs = @(
  (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'),
  (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs')
);
foreach ($dir in $dirs) {
  Get-ChildItem -LiteralPath $dir -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
    $target = $shell.CreateShortcut($_.FullName).TargetPath;
    if ($target) { Write-Output ($_.FullName + '|' + $target) }
  }
}"#;

    let secure_path = crate::winutil::get_secure_system_path();
    let output = match Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(["-NoProfile", "-Command", script])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (shortcut, target) = line.split_once('|')?;
            Some((shortcut.trim().to_string(), target.trim().to_string()))
        })
        .filter(|(shortcut, target)| !shortcut.is_empty() && !target.is_empty())
        .collect()
}

fn cached_shortcut_targets() -> &'static [(String, String)] {
    static CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    CACHE.get_or_init(shortcut_targets).as_slice()
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn name_words(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(normalized)
        .filter(|word| word.len() >= 4)
        .collect()
}

fn folders_match(folder: &str, display_name: &str) -> bool {
    let folder_norm = normalized(folder);
    let name_norm = normalized(display_name);
    if folder_norm.len() < 4 || name_norm.len() < 4 {
        return false;
    }
    if folder_norm == name_norm {
        return true;
    }

    let words = name_words(display_name);
    if words.iter().any(|word| folder_norm.contains(word)) {
        return true;
    }

    folder_norm
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() >= 6)
        .any(|token| {
            words.iter().any(|word| {
                token.len().abs_diff(word.len()) <= 2
                    && super::remnants::levenshtein_distance(token, word) <= 1
            })
        })
}

/// Find an install root from Start Menu shortcut targets when the registry
/// omits InstallLocation. Conservative name/folder matching keeps unrelated
/// shortcuts from becoming deletion targets.
pub fn find_install_location_for_name(display_name: &str) -> Option<String> {
    if normalized(display_name).len() < 4 {
        return None;
    }

    for (shortcut, target) in cached_shortcut_targets() {
        let target_path = Path::new(target);
        if !target_path.is_file() {
            continue;
        }
        let Some(parent) = target_path.parent() else {
            continue;
        };
        if crate::winutil::is_safe_remnant_path(&parent.to_string_lossy()).is_err() {
            continue;
        }
        let shortcut_stem = Path::new(shortcut)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let parent_name = parent
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if folders_match(shortcut_stem, display_name) || folders_match(parent_name, display_name) {
            return Some(parent.to_string_lossy().to_string());
        }
    }
    None
}

fn matches_target(target: &str, app_name: Option<&str>, install_location: Option<&str>) -> bool {
    let target = target.to_lowercase();
    if let Some(location) = install_location {
        let location = location.trim().to_lowercase();
        if !location.is_empty() && target.starts_with(&location) {
            return true;
        }
    }
    if let Some(name) = app_name {
        let name = name.trim().to_lowercase();
        if name.len() >= 4 && target.contains(&name) {
            return true;
        }
    }
    false
}

/// Finds Start Menu shortcuts whose target points into the app install folder.
/// Desktop/public shortcuts are intentionally not touched.
pub fn find_app_shortcuts(app_name: Option<&str>, install_location: Option<&str>) -> Vec<String> {
    if install_location.map_or(true, |location| location.trim().is_empty())
        && app_name.map_or(true, |name| name.trim().len() < 4)
    {
        return Vec::new();
    }

    cached_shortcut_targets()
        .iter()
        .filter(|(_, target)| matches_target(target, app_name, install_location))
        .map(|(shortcut, _)| shortcut.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_folder_matching_is_conservative() {
        assert!(folders_match("PhotoScape", "PhotoScape"));
        assert!(folders_match("Photoscap", "PhotoScape"));
        assert!(folders_match("Google Chrome", "Google Chrome"));
        assert!(!folders_match("OtherApp", "PhotoScape"));
        assert!(!folders_match("Programs", "PhotoScape"));
    }

    #[test]
    fn matches_only_install_targets() {
        assert!(matches_target(
            r"C:\Program Files\MyApp\MyApp.exe",
            Some("MyApp"),
            Some(r"C:\Program Files\MyApp")
        ));
        assert!(!matches_target(
            r"C:\Program Files\Other\Other.exe",
            Some("MyApp"),
            Some(r"C:\Program Files\MyApp")
        ));
    }
}
