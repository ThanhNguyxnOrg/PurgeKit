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

fn get_active_node_version() -> Option<String> {
    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
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
    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
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
    let status = match manager {
        "rustup" => {
            Command::new("rustup")
                .creation_flags(CREATE_NO_WINDOW)
                .args(&["toolchain", "uninstall", version])
                .status()
        }
        "nvm" => {
            Command::new("nvm")
                .creation_flags(CREATE_NO_WINDOW)
                .args(&["uninstall", version])
                .status()
        }
        "fnm" => {
            Command::new("fnm")
                .creation_flags(CREATE_NO_WINDOW)
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
        // Just verify scanning works without crashing on target test environment
        let results = scan_toolchain_versions();
        // Check active nodes and rustups match
        for item in results {
            assert!(!item.version.is_empty());
            assert!(!item.path.is_empty());
        }
    }
}
