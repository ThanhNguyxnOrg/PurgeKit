use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

const CREATE_NO_WINDOW: u32 = 0x08000000;

struct ShortcutEntry {
    shortcut: String,
    target: String,
    icon: Option<String>,
}

fn shortcut_targets() -> Vec<ShortcutEntry> {
    let script = r#"$ErrorActionPreference='SilentlyContinue';
$shell = New-Object -ComObject WScript.Shell;
$dirs = @(
  (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'),
  (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs')
);
foreach ($dir in $dirs) {
  Get-ChildItem -LiteralPath $dir -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
    $shortcut = $shell.CreateShortcut($_.FullName);
    $target = $shortcut.TargetPath;
    $icon = $shortcut.IconLocation;
    if ($target -or $icon) { Write-Output ($_.FullName + '|' + $target + '|' + $icon) }
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
            let mut parts = line.splitn(3, '|');
            let shortcut = parts.next()?.trim().to_string();
            let target = parts.next().unwrap_or_default().trim().to_string();
            let icon = parts.next().unwrap_or_default().trim().to_string();
            if shortcut.is_empty() || (target.is_empty() && icon.is_empty()) {
                return None;
            }
            Some(ShortcutEntry {
                shortcut,
                target,
                icon: (!icon.is_empty()).then_some(icon),
            })
        })
        .collect()
}

fn cached_shortcuts() -> &'static [ShortcutEntry] {
    static CACHE: OnceLock<Vec<ShortcutEntry>> = OnceLock::new();
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

    for entry in cached_shortcuts() {
        let target_path = Path::new(&entry.target);
        if !target_path.is_file() {
            continue;
        }
        let Some(parent) = target_path.parent() else {
            continue;
        };
        if crate::winutil::is_safe_remnant_path(&parent.to_string_lossy()).is_err() {
            continue;
        }
        let shortcut_stem = Path::new(&entry.shortcut)
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

fn shortcut_icon_path(raw: &str) -> Option<String> {
    let cleaned = raw.trim().trim_matches('"');
    if cleaned.is_empty() {
        return None;
    }
    let path = match cleaned.rsplit_once(',') {
        Some((path, index)) if index.trim().parse::<u32>().is_ok() => path,
        _ => cleaned,
    };
    let path = crate::winutil::expand_env_strings(path.trim().trim_matches('"'));
    if path.is_empty() || !Path::new(&path).is_file() {
        return None;
    }
    Some(path)
}

fn icon_source_for_entry(entry: &ShortcutEntry) -> Option<String> {
    if let Some(icon) = entry.icon.as_deref().and_then(shortcut_icon_path) {
        return Some(icon);
    }
    let target = Path::new(&entry.target);
    if target.is_file()
        && target
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Some(entry.target.clone());
    }
    None
}

/// Find a Start Menu shortcut icon for an app whose registry metadata has no
/// usable icon. Matching stays conservative so unrelated shortcuts cannot
/// change the app's displayed logo.
pub fn find_icon_source_for_name(display_name: &str) -> Option<String> {
    if normalized(display_name).len() < 4 {
        return None;
    }

    let mut best: Option<(i32, String)> = None;
    for entry in cached_shortcuts() {
        let shortcut_stem = Path::new(&entry.shortcut)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let target_path = Path::new(&entry.target);
        let target_stem = target_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let parent_name = target_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("");

        let mut score = 0;
        if folders_match(shortcut_stem, display_name) {
            score += 40;
        }
        if !parent_name.is_empty() && folders_match(parent_name, display_name) {
            score += 30;
        }
        if !target_stem.is_empty() && folders_match(target_stem, display_name) {
            score += 20;
        }
        if score == 0 {
            continue;
        }

        let haystack = format!("{} {} {}", shortcut_stem, target_stem, parent_name).to_lowercase();
        if [
            "uninstall",
            "unins",
            "setup",
            "update",
            "updater",
            "helper",
            "crash",
            "repair",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
        {
            score -= 25;
        }
        if score <= 0 {
            continue;
        }

        if let Some(source) = icon_source_for_entry(entry) {
            if best
                .as_ref()
                .map_or(true, |(best_score, _)| score > *best_score)
            {
                best = Some((score, source));
            }
        }
    }

    best.map(|(_, source)| source)
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

    cached_shortcuts()
        .iter()
        .filter(|entry| matches_target(&entry.target, app_name, install_location))
        .map(|entry| entry.shortcut.clone())
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
    fn parses_shortcut_icon_locations() {
        let path = std::env::temp_dir().join("purgekit_shortcut_icon_test.ico");
        std::fs::write(&path, b"icon").unwrap();
        let raw = format!("{},0", path.to_string_lossy());
        assert_eq!(
            shortcut_icon_path(&raw).map(|value| value.to_lowercase()),
            Some(path.to_string_lossy().to_lowercase())
        );
        assert_eq!(shortcut_icon_path(""), None);
        assert_eq!(shortcut_icon_path("C:\\missing\\icon.ico,0"), None);
        let _ = std::fs::remove_file(&path);
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
