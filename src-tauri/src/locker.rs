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
    let path_buf = Path::new(path);
    if !path_buf.exists() {
        return DeleteResult::Deleted;
    }

    // Attempt 1: Direct delete
    if fs::remove_file(path_buf).is_ok() {
        return DeleteResult::Deleted;
    }
    if path_buf.is_dir() && fs::remove_dir_all(path_buf).is_ok() {
        return DeleteResult::Deleted;
    }

    // Attempt 2: Restart Manager (graceful unlock)
    if unlock_file_restart_manager(path).is_ok() {
        if fs::remove_file(path_buf).is_ok() {
            return DeleteResult::DeletedAfterUnlock;
        }
        if path_buf.is_dir() && fs::remove_dir_all(path_buf).is_ok() {
            return DeleteResult::DeletedAfterUnlock;
        }
    }

    // Attempt 3: Force close remote handles (undocumented, requires hidden setting)
    let settings = crate::settings::load_settings();
    if settings.enable_undocumented_force_unlock && is_elevated::is_elevated() {
        if force_close_file_handle(path).is_ok() {
            if fs::remove_file(path_buf).is_ok() {
                return DeleteResult::ForceDeleted;
            }
        }
    }

    // Attempt 4: Schedule boot-time deletion (requires admin)
    if is_elevated::is_elevated() {
        if schedule_boot_delete(path).is_ok() {
            return DeleteResult::ScheduledForReboot;
        }
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

// Tier 2: Force Close Handles
// Query system handles via NtQuerySystemInformation
pub fn force_close_file_handle(_file_path: &str) -> Result<(), String> {
    // To prevent kernel instability (blue screen or crashes) on target systems,
    // we return an error so the file deletion gracefully falls back to schedule_boot_delete.
    Err("Force closing remote handles is undocumented and disabled for stability; falling back to boot-time deletion.".to_string())
}
