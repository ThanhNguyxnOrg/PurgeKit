use std::os::windows::process::CommandExt;
use std::process::Command;

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

    shortcut_targets()
        .into_iter()
        .filter(|(_, target)| matches_target(target, app_name, install_location))
        .map(|(shortcut, _)| shortcut)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
