use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct PortableApp {
    pub name: String,
    pub path: String,
    pub executable: String,
    pub size_bytes: u64,
}

fn is_reparse(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| crate::winutil::is_reparse_point(&metadata))
        .unwrap_or(false)
}

fn calculate_size(root: &Path) -> u64 {
    let mut total = 0;
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() || is_reparse(entry.path()) {
            continue;
        }
        total += crate::winutil::allocated_size(entry.path());
    }
    total
}

fn best_executable(root: &Path) -> Option<(PathBuf, u64)> {
    let mut executables = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .max_depth(2)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let is_exe = entry
            .path()
            .extension()
            .map_or(false, |extension| extension.eq_ignore_ascii_case("exe"));
        if !is_exe || is_reparse(entry.path()) {
            continue;
        }

        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        executables.push((entry.path().to_path_buf(), size));
        if executables.len() > 40 {
            return None;
        }
    }

    executables.into_iter().max_by_key(|(_, size)| *size)
}

fn is_claimed(path: &Path, claimed_paths: &[String]) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    claimed_paths.iter().any(|claimed| {
        let claimed = claimed.to_lowercase();
        !claimed.is_empty() && (path.starts_with(&claimed) || claimed.starts_with(&path))
    })
}

fn scan_portable_root(root: &Path, claimed_paths: &[String]) -> Vec<PortableApp> {
    let mut apps = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return apps;
    };

    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if !path.is_dir() || is_reparse(&path) {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if matches!(
            name.to_lowercase().as_str(),
            "microsoft" | "windows" | "common files" | "windowsapps" | "packages" | "node_modules"
        ) {
            continue;
        }

        if is_claimed(&path, claimed_paths) {
            continue;
        }

        let Some((executable, _)) = best_executable(&path) else {
            continue;
        };

        apps.push(PortableApp {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            executable: executable.to_string_lossy().to_string(),
            size_bytes: calculate_size(&path),
        });
    }

    apps
}

pub fn scan_portable_apps(claimed_paths: &[String]) -> Vec<PortableApp> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    let root = local_app_data.join("Programs");
    scan_portable_root(&root, claimed_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_unclaimed_folder_with_exe() {
        let root = std::env::temp_dir().join("purgekit_portable_test");
        let app_dir = root.join("MyPortableTool");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("tool.exe"), b"not a real exe").unwrap();

        let apps = scan_portable_root(&root, &[]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "MyPortableTool");
        assert!(apps[0].executable.ends_with("tool.exe"));

        let claimed = vec![app_dir.to_string_lossy().to_string()];
        assert!(scan_portable_root(&root, &claimed).is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
