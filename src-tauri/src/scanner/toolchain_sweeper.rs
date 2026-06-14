use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolchainVersion {
    pub manager: String,      // "rustup" | "fnm" | "nvm"
    pub version: String,      // e.g. "stable-x86_64-pc-windows-msvc" or "v20.10.0"
    pub path: String,         // Absolute directory path
    pub size_bytes: u64,      // Size of the directory on disk
    pub is_active: bool,      // Is it the currently active version?
}

fn calculate_dir_size(path: &Path) -> u64 {
    let mut size = 0;
    for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                size += meta.len();
            }
        }
    }
    size
}

fn is_in_toolchain_dir(manager: &str, path_str: &str) -> bool {
    let path = Path::new(path_str);
    let path_norm = crate::winutil::canonicalize_path_safety(&path.to_string_lossy());
    let path_norm_str = path_norm.to_string_lossy().to_lowercase();

    let userprofile = std::env::var("USERPROFILE").map(PathBuf::from).ok();

    let mut allowed_parents = Vec::new();
    match manager {
        "rustup" => {
            if let Some(up) = &userprofile {
                allowed_parents.push(up.join(".rustup").join("toolchains"));
            }
        }
        "nvm" => {
            if let Ok(nvm_home) = std::env::var("NVM_HOME").map(PathBuf::from) {
                allowed_parents.push(nvm_home);
            }
            if let Some(up) = &userprofile {
                allowed_parents.push(up.join("AppData").join("Roaming").join("nvm"));
            }
        }
        "fnm" => {
            if let Ok(fnm_dir) = std::env::var("FNM_DIR").map(PathBuf::from) {
                allowed_parents.push(fnm_dir.join("node-versions"));
            }
            if let Some(up) = &userprofile {
                allowed_parents.push(up.join(".fnm").join("node-versions"));
                allowed_parents.push(up.join(".local").join("share").join("fnm").join("node-versions"));
            }
            if let Some(appdata) = std::env::var("APPDATA").map(PathBuf::from).ok() {
                allowed_parents.push(appdata.join("fnm").join("node-versions"));
            }
            if let Some(localappdata) = std::env::var("LOCALAPPDATA").map(PathBuf::from).ok() {
                allowed_parents.push(localappdata.join("fnm").join("node-versions"));
            }
        }
        _ => return false,
    }

    for parent in allowed_parents {
        let parent_norm = crate::winutil::canonicalize_path_safety(&parent.to_string_lossy());
        let parent_norm_str = parent_norm.to_string_lossy().to_lowercase();
        if path_norm_str.starts_with(&parent_norm_str) {
            let remain = &path_norm_str[parent_norm_str.len()..];
            if remain.is_empty() || remain.starts_with('\\') || remain.starts_with('/') {
                return true;
            }
        }
    }
    false
}

fn get_active_node_version() -> Option<String> {
    let secure_path = crate::winutil::get_secure_system_path();
    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&["/C", "node -v"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_active_rust_version() -> Option<String> {
    let secure_path = crate::winutil::get_secure_system_path();
    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&["/C", "rustup show active-toolchain"])
        .output()
        .ok()?;
    if output.status.success() {
        let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let first_word = res.split_whitespace().next()?.to_string();
        Some(first_word)
    } else {
        None
    }
}

pub fn scan_toolchain_versions() -> Vec<ToolchainVersion> {
    let mut list = Vec::new();
    
    let active_node = get_active_node_version();
    let active_rust = get_active_rust_version();

    let userprofile = std::env::var("USERPROFILE").map(PathBuf::from);

    // 1. Rustup
    if let Ok(ref up) = userprofile {
        let rustup_toolchains = up.join(".rustup").join("toolchains");
        if rustup_toolchains.exists() && rustup_toolchains.is_dir() {
            if let Ok(entries) = fs::read_dir(&rustup_toolchains) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let size = calculate_dir_size(&path);
                        let is_active = active_rust.as_ref().map_or(false, |act| act.eq_ignore_ascii_case(&name));
                        
                        list.push(ToolchainVersion {
                            manager: "rustup".to_string(),
                            version: name,
                            path: path.to_string_lossy().to_string(),
                            size_bytes: size,
                            is_active,
                        });
                    }
                }
            }
        }
    }

    // 2. NVM for Windows
    let nvm_home = std::env::var("NVM_HOME").map(PathBuf::from)
        .or_else(|_| std::env::var("APPDATA").map(|a| PathBuf::from(a).join("nvm")));
    
    if let Ok(nvm_path) = nvm_home {
        if nvm_path.exists() && nvm_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&nvm_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        if name.starts_with('v') && name.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
                            let size = calculate_dir_size(&path);
                            let is_active = active_node.as_ref().map_or(false, |act| act.eq_ignore_ascii_case(&name));
                            
                            list.push(ToolchainVersion {
                                manager: "nvm".to_string(),
                                version: name,
                                path: path.to_string_lossy().to_string(),
                                size_bytes: size,
                                is_active,
                            });
                        }
                    }
                }
            }
        }
    }

    // 3. FNM
    let fnm_dir_env = std::env::var("FNM_DIR").map(PathBuf::from);
    let mut fnm_candidates = Vec::new();
    if let Ok(fd) = fnm_dir_env {
        fnm_candidates.push(fd.join("node-versions"));
    }
    if let Ok(ref up) = userprofile {
        fnm_candidates.push(up.join(".fnm").join("node-versions"));
        fnm_candidates.push(up.join(".local").join("share").join("fnm").join("node-versions"));
    }
    if let Ok(appdata) = std::env::var("APPDATA").map(PathBuf::from) {
        fnm_candidates.push(appdata.join("fnm").join("node-versions"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA").map(PathBuf::from) {
        fnm_candidates.push(localappdata.join("fnm").join("node-versions"));
    }

    let mut checked_paths = std::collections::HashSet::new();

    for candidate in fnm_candidates {
        let canonical = match fs::canonicalize(&candidate) {
            Ok(c) => c,
            Err(_) => candidate.clone(),
        };
        if checked_paths.contains(&canonical) {
            continue;
        }
        checked_paths.insert(canonical);

        if candidate.exists() && candidate.is_dir() {
            if let Ok(entries) = fs::read_dir(&candidate) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        if name.starts_with('v') && name.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
                            let size = calculate_dir_size(&path);
                            let is_active = active_node.as_ref().map_or(false, |act| act.eq_ignore_ascii_case(&name));
                            
                            list.push(ToolchainVersion {
                                manager: "fnm".to_string(),
                                version: name,
                                path: path.to_string_lossy().to_string(),
                                size_bytes: size,
                                is_active,
                            });
                        }
                    }
                }
            }
        }
    }

    list
}

pub fn uninstall_toolchain_version(manager: &str, version: &str, path: &str) -> Result<(), String> {
    let is_valid_version = !version.is_empty()
        && version.len() <= 100
        && version.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '@'
        });
    if !is_valid_version {
        return Err(format!("Invalid toolchain version name: {}", version));
    }

    if !is_in_toolchain_dir(manager, path) {
        return Err("Access Denied: Toolchain directory path is invalid or outside allowed manager directories.".to_string());
    }

    let secure_path = crate::winutil::get_secure_system_path();
    let status = match manager {
        "rustup" => {
            Command::new("rustup")
                .creation_flags(CREATE_NO_WINDOW)
                .env("PATH", &secure_path)
                .args(&["toolchain", "uninstall", version])
                .status()
        }
        "nvm" => {
            Command::new("nvm")
                .creation_flags(CREATE_NO_WINDOW)
                .env("PATH", &secure_path)
                .args(&["uninstall", version])
                .status()
        }
        "fnm" => {
            Command::new("fnm")
                .creation_flags(CREATE_NO_WINDOW)
                .env("PATH", &secure_path)
                .args(&["uninstall", version])
                .status()
        }
        _ => return Err(format!("Unsupported manager: {}", manager)),
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            let path_buf = Path::new(path);
            if path_buf.exists() && path_buf.is_dir() {
                match crate::locker::delete_file_with_escalation(path) {
                    crate::locker::DeleteResult::Deleted | crate::locker::DeleteResult::DeletedAfterUnlock | crate::locker::DeleteResult::ForceDeleted => {
                        Ok(())
                    }
                    crate::locker::DeleteResult::Failed(err) => {
                        Err(format!("CLI uninstall failed and directory purge failed: {}", err))
                    }
                    _ => Err("Direct directory deletion scheduled for reboot.".to_string()),
                }
            } else {
                Err(format!("CLI uninstall failed and directory path does not exist: {}", path))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_toolchain_versions_no_crash() {
        let results = scan_toolchain_versions();
        for item in results {
            assert!(!item.version.is_empty());
            assert!(!item.path.is_empty());
        }
    }

    #[test]
    fn test_is_in_toolchain_dir() {
        let userprofile = std::env::var("USERPROFILE").ok().map(PathBuf::from);
        if let Some(up) = userprofile {
            let rust_path = up.join(".rustup").join("toolchains").join("stable-x86_64-pc-windows-msvc");
            assert!(is_in_toolchain_dir("rustup", &rust_path.to_string_lossy()));
            
            let sub_file = rust_path.join("bin").join("rustc.exe");
            assert!(is_in_toolchain_dir("rustup", &sub_file.to_string_lossy()));
        }

        assert!(!is_in_toolchain_dir("rustup", r"C:\Windows\System32\cmd.exe"));
        assert!(!is_in_toolchain_dir("rustup", r"C:\Users\Public\Documents"));
        assert!(!is_in_toolchain_dir("invalid_manager", r"C:\Users"));
    }

    #[test]
    fn test_uninstall_toolchain_version_validation() {
        let res1 = uninstall_toolchain_version("rustup", "stable & calc.exe", "C:\\invalid_path");
        assert!(res1.is_err());
        assert!(res1.unwrap_err().contains("Invalid toolchain version name"));

        let res2 = uninstall_toolchain_version("rustup", "stable", r"C:\Windows\System32\cmd.exe");
        assert!(res2.is_err());
        assert!(res2.unwrap_err().contains("Access Denied"));
    }
}
