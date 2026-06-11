use std::sync::Mutex;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Ioctl::*;
use windows_sys::Win32::System::IO::DeviceIoControl;

pub struct TrackingSession {
    pub start_usn: u64,
    pub name: String,
    pub start_time: std::time::SystemTime,
    pub reg_baseline: Vec<String>,
}

pub struct ActiveTracking(pub std::sync::Arc<Mutex<Option<TrackingSession>>>);

pub unsafe fn query_current_usn(drive: char) -> Result<u64, String> {
    let vol_path = format!("\\\\.\\{}:\0", drive);
    let vol_wide: Vec<u16> = vol_path.encode_utf16().collect();
    
    let handle = CreateFileW(
        vol_wide.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        std::ptr::null(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        std::ptr::null_mut(),
    );
    
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("Failed to open volume handle for drive {}: {}", drive, std::io::Error::last_os_error()));
    }
    
    #[repr(C)]
    struct UsnJournalData {
        usn_journal_id: u64,
        first_usn: u64,
        next_usn: u64,
        lowest_valid_usn: u64,
        max_usn: u64,
        maximum_size: u64,
        allocation_granularity: u64,
    }
    
    let mut journal_data = std::mem::zeroed::<UsnJournalData>();
    let mut bytes_returned = 0u32;
    
    let success = DeviceIoControl(
        handle,
        FSCTL_QUERY_USN_JOURNAL,
        std::ptr::null(),
        0,
        &mut journal_data as *mut _ as *mut c_void,
        std::mem::size_of::<UsnJournalData>() as u32,
        &mut bytes_returned,
        std::ptr::null_mut(),
    );
    
    CloseHandle(handle);
    
    if success != 0 {
        Ok(journal_data.next_usn)
    } else {
        Err(format!("FSCTL_QUERY_USN_JOURNAL failed: {}", std::io::Error::last_os_error()))
    }
}

pub unsafe fn read_usn_changes(drive: char, start_usn: u64) -> Result<Vec<String>, String> {
    let vol_path = format!("\\\\.\\{}:\0", drive);
    let vol_wide: Vec<u16> = vol_path.encode_utf16().collect();
    
    let handle = CreateFileW(
        vol_wide.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        std::ptr::null(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        std::ptr::null_mut(),
    );
    
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("Failed to open volume: {}", std::io::Error::last_os_error()));
    }
    
    #[repr(C)]
    struct UsnJournalData {
        usn_journal_id: u64,
        first_usn: u64,
        next_usn: u64,
        lowest_valid_usn: u64,
        max_usn: u64,
        maximum_size: u64,
        allocation_granularity: u64,
    }
    
    let mut journal_data = std::mem::zeroed::<UsnJournalData>();
    let mut bytes_returned = 0u32;
    
    let success = DeviceIoControl(
        handle,
        FSCTL_QUERY_USN_JOURNAL,
        std::ptr::null(),
        0,
        &mut journal_data as *mut _ as *mut c_void,
        std::mem::size_of::<UsnJournalData>() as u32,
        &mut bytes_returned,
        std::ptr::null_mut(),
    );
    
    if success == 0 {
        CloseHandle(handle);
        return Err("Failed to query USN journal metadata".into());
    }
    
    #[repr(C)]
    struct ReadUsnJournalData {
        start_usn: u64,
        reason_mask: u32,
        return_only_on_close: u32,
        timeout: u64,
        bytes_to_wait: u64,
        usn_journal_id: u64,
    }
    
    let read_data = ReadUsnJournalData {
        start_usn,
        reason_mask: 0xFFFFFFFF,
        return_only_on_close: 0,
        timeout: 0,
        bytes_to_wait: 0,
        usn_journal_id: journal_data.usn_journal_id,
    };
    
    let mut buf = vec![0u8; 65536];
    let mut bytes_returned = 0u32;
    
    let success = DeviceIoControl(
        handle,
        FSCTL_READ_USN_JOURNAL,
        &read_data as *const _ as *const c_void,
        std::mem::size_of::<ReadUsnJournalData>() as u32,
        buf.as_mut_ptr() as *mut c_void,
        buf.len() as u32,
        &mut bytes_returned,
        std::ptr::null_mut(),
    );
    
    CloseHandle(handle);
    
    let mut files = Vec::new();
    if success != 0 && bytes_returned > 8 {
        let mut offset = 8usize;
        while offset < bytes_returned as usize {
            let record_ptr = buf.as_ptr().add(offset);
            let length = std::ptr::read_unaligned(record_ptr as *const u32);
            if length == 0 { break; }
            
            let file_name_length = std::ptr::read_unaligned(record_ptr.add(56) as *const u16) as usize;
            let file_name_offset = std::ptr::read_unaligned(record_ptr.add(58) as *const u16) as usize;
            
            let file_name_ptr = record_ptr.add(file_name_offset) as *const u16;
            let file_name_slice = std::slice::from_raw_parts(file_name_ptr, file_name_length / 2);
            let name = String::from_utf16_lossy(file_name_slice);
            
            if !name.starts_with('$') {
                files.push(name);
            }
            
            offset += length as usize;
        }
    }
    
    Ok(files)
}
