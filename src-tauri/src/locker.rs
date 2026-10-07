use std::fs;
use std::path::Path;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::RestartManager::*;
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

#[derive(Debug, serde::Serialize, Clone)]
pub enum DeleteResult {
    Deleted,
    DeletedAfterUnlock,
    ForceDeleted,
    ScheduledForReboot,
    Failed(String),
}

pub fn delete_file_with_escalation(path: &str) -> DeleteResult {
    if let Err(e) = crate::winutil::is_safe_to_delete(path) {
        return DeleteResult::Failed(e);
    }

    let path_buf = Path::new(path);
    if !path_buf.exists() {
        return DeleteResult::Deleted;
    }

    let is_dir = path_buf.is_dir();

    // Attempt 1: Direct delete
    if is_dir {
        if fs::remove_dir_all(path_buf).is_ok() {
            return DeleteResult::Deleted;
        }
    } else if fs::remove_file(path_buf).is_ok() {
        return DeleteResult::Deleted;
    }

    // Attempt 2: Restart Manager (graceful unlock)
    if unlock_file_restart_manager(path).is_ok() {
        if is_dir {
            if fs::remove_dir_all(path_buf).is_ok() {
                return DeleteResult::DeletedAfterUnlock;
            }
        } else if fs::remove_file(path_buf).is_ok() {
            return DeleteResult::DeletedAfterUnlock;
        }
    }

    // Attempt 3: Schedule boot-time deletion (requires admin)
    if is_elevated::is_elevated() && schedule_boot_delete(path).is_ok() {
        return DeleteResult::ScheduledForReboot;
    }

    DeleteResult::Failed("All unlocking and file deletion methods failed.".to_string())
}

pub fn unlock_file_restart_manager(file_path: &str) -> Result<(), String> {
    unsafe {
        let mut session_handle = 0u32;
        let mut session_key = [0u16; 33];
        
        let res_start = RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr());
        if res_start != ERROR_SUCCESS {
            return Err(format!("RmStartSession failed with code {}", res_start));
        }

        let wide_path: Vec<u16> = file_path.encode_utf16().chain(Some(0)).collect();
        let paths = [wide_path.as_ptr()];

        let res_reg = RmRegisterResources(
            session_handle,
            1,
            paths.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        );

        if res_reg != ERROR_SUCCESS {
            RmEndSession(session_handle);
            return Err(format!("RmRegisterResources failed with code {}", res_reg));
        }

        // Request shutdown of processes locking this resource
        let res_shut = RmShutdown(session_handle, RmForceShutdown as u32, None);
        
        RmEndSession(session_handle);

        if res_shut == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("RmShutdown failed with code {}", res_shut))
        }
    }
}

pub fn schedule_boot_delete(file_path: &str) -> Result<(), String> {
    let p = Path::new(file_path);
    if p.is_dir() {
        // MoveFileExW with MOVEFILE_DELAY_UNTIL_REBOOT requires empty directories.
        // Schedule all directory contents bottom-up first.
        for entry in walkdir::WalkDir::new(p).contents_first(true).into_iter().filter_map(|e| e.ok()) {
            if entry.path() != p {
                let wide: Vec<u16> = entry.path().to_string_lossy().encode_utf16().chain(Some(0)).collect();
                unsafe {
                    MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT);
                }
            }
        }
    }

    let wide: Vec<u16> = file_path.encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT)
    };

    if result != 0 {
        Ok(())
    } else {
        Err(format!("MoveFileExW failed: {}", std::io::Error::last_os_error()))
    }
}
