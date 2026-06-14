use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WslDistroInfo {
    pub id: String,
    pub name: String,
    pub base_path: String,
    pub vhdx_path: Option<String>,
    pub vhdx_size_bytes: u64,
    pub is_sparse: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct WslCompactProgress {
    pub phase: String, // "shutdown" | "compacting" | "log" | "completed" | "error"
    pub message: String,
}

#[cfg(windows)]
pub fn check_file_is_sparse(path_str: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let wide_path: Vec<u16> = std::ffi::OsStr::new(path_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        let attrs = windows_sys::Win32::Storage::FileSystem::GetFileAttributesW(wide_path.as_ptr());
        if attrs == windows_sys::Win32::Storage::FileSystem::INVALID_FILE_ATTRIBUTES {
            false
        } else {
            (attrs & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SPARSE_FILE) != 0
        }
    }
}

#[cfg(not(windows))]
pub fn check_file_is_sparse(_path_str: &str) -> bool {
    false
}

pub fn scan_wsl_distributions() -> Vec<WslDistroInfo> {
    let mut distros = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    
    let lxss_path = r"Software\Microsoft\Windows\CurrentVersion\Lxss";
    let lxss_key = match hkcu.open_subkey_with_flags(lxss_path, KEY_READ) {
        Ok(key) => key,
        Err(_) => return distros,
    };

    for subkey_name in lxss_key.enum_keys().filter_map(|x| x.ok()) {
        let subkey = match lxss_key.open_subkey_with_flags(&subkey_name, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let name: String = subkey.get_value("DistributionName").unwrap_or_default();
        if name.is_empty() {
            continue;
        }

        let base_path: String = subkey.get_value("BasePath").unwrap_or_default();
        if base_path.is_empty() {
            continue;
        }

        let base_path_buf = PathBuf::from(&base_path);
        let ext4_vhdx = base_path_buf.join("ext4.vhdx");
        let data_ext4_vhdx = base_path_buf.join("data").join("ext4.vhdx");

        let mut resolved_vhdx = None;
        let mut vhdx_size = 0;

        if ext4_vhdx.exists() {
            resolved_vhdx = Some(ext4_vhdx.to_string_lossy().to_string());
            if let Ok(meta) = ext4_vhdx.metadata() {
                vhdx_size = meta.len();
            }
        } else if data_ext4_vhdx.exists() {
            resolved_vhdx = Some(data_ext4_vhdx.to_string_lossy().to_string());
            if let Ok(meta) = data_ext4_vhdx.metadata() {
                vhdx_size = meta.len();
            }
        } else {
            // Find any vhdx file in base_path
            if let Ok(entries) = fs::read_dir(&base_path_buf) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "vhdx") {
                        resolved_vhdx = Some(path.to_string_lossy().to_string());
                        if let Ok(meta) = path.metadata() {
                            vhdx_size = meta.len();
                        }
                        break;
                    }
                }
            }
        }

        let is_sparse = if let Some(ref path_str) = resolved_vhdx {
            check_file_is_sparse(path_str)
        } else {
            false
        };

        distros.push(WslDistroInfo {
            id: subkey_name,
            name,
            base_path,
            vhdx_path: resolved_vhdx,
            vhdx_size_bytes: vhdx_size,
            is_sparse,
        });
    }

    distros
}

pub fn run_wsl_shutdown(app: &AppHandle) -> Result<(), String> {
    let _ = app.emit("wsl-compact-progress", WslCompactProgress {
        phase: "shutdown".into(),
        message: "Requesting WSL shutdown (wsl --shutdown)...".into(),
    });

    let secure_path = crate::winutil::get_secure_system_path();
    let status = Command::new("wsl")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .arg("--shutdown")
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("WSL shutdown failed with status: {}", s)),
        Err(e) => Err(format!("Failed to execute wsl --shutdown: {}", e)),
    }
}

pub fn compact_vhdx_diskpart(app: &AppHandle, vhdx_path: &str) -> Result<String, String> {
    // Validate path security and injection prevention
    if vhdx_path.contains('"') || vhdx_path.contains('\n') || vhdx_path.contains('\r') {
        return Err("VHDX path contains invalid or dangerous characters (Diskpart Injection Blocked)".to_string());
    }

    let path = std::path::Path::new(vhdx_path);
    if !path.exists() || !path.is_file() {
        return Err(format!("VHDX file does not exist: {}", vhdx_path));
    }

    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext != "vhdx" {
        return Err("File path must point to a valid .vhdx virtual disk.".to_string());
    }

    // 1. Shutdown WSL to release file lock
    run_wsl_shutdown(app)?;

    // 2. Prepare diskpart script in temp directory
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("purgekit_compact_{}.txt", uuid::Uuid::new_v4()));
    
    let script_content = format!(
        "select vdisk file=\"{}\"\nattach vdisk readonly\ncompact vdisk\ndetach vdisk\nexit\n",
        vhdx_path
    );

    if let Err(e) = fs::write(&script_path, script_content) {
        return Err(format!("Failed to write diskpart script: {}", e));
    }

    let _ = app.emit("wsl-compact-progress", WslCompactProgress {
        phase: "compacting".into(),
        message: "Executing VHDX disk compaction using diskpart...".into(),
    });

    // 3. Run diskpart
    let secure_path = crate::winutil::get_secure_system_path();
    let output = Command::new("diskpart")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .arg("/s")
        .arg(&script_path)
        .output();

    // Clean up temp file
    let _ = fs::remove_file(&script_path);

    match output {
        Ok(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                let _ = app.emit("wsl-compact-progress", WslCompactProgress {
                    phase: "log".into(),
                    message: stdout_str.clone(),
                });
                
                let _ = app.emit("wsl-compact-progress", WslCompactProgress {
                    phase: "completed".into(),
                    message: "WSL2 virtual disk compacted successfully!".into(),
                });

                Ok(stdout_str)
            } else {
                let err_msg = format!("Diskpart failed.\nStdout: {}\nStderr: {}", stdout_str, stderr_str);
                let _ = app.emit("wsl-compact-progress", WslCompactProgress {
                    phase: "error".into(),
                    message: err_msg.clone(),
                });
                Err(err_msg)
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to spawn diskpart: {}", e);
            let _ = app.emit("wsl-compact-progress", WslCompactProgress {
                phase: "error".into(),
                message: err_msg.clone(),
            });
            Err(err_msg)
        }
    }
}

pub fn set_wsl_distro_sparse(app: &AppHandle, distro_name: &str, sparse: bool) -> Result<(), String> {
    if distro_name.chars().any(|c| c.is_control() || c == ' ' || c == '&' || c == '|' || c == '"') {
        return Err("Invalid WSL distribution name".to_string());
    }

    // 1. Shutdown WSL first
    run_wsl_shutdown(app)?;

    // 2. Set sparse flag
    let status_str = if sparse { "true" } else { "false" };
    
    let secure_path = crate::winutil::get_secure_system_path();
    let status = Command::new("wsl")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&["--manage", distro_name, "--set-sparse", status_str])
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("WSL set-sparse command failed with status: {}", s)),
        Err(e) => Err(format!("Failed to execute wsl --manage set-sparse: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_file_is_sparse_on_normal_file() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("purgekit_test_normal.txt");
        std::fs::write(&temp_file, "hello").unwrap();
        
        let path_str = temp_file.to_string_lossy().to_string();
        let is_sparse = check_file_is_sparse(&path_str);
        
        let _ = std::fs::remove_file(&temp_file);
        assert!(!is_sparse);
    }
}
