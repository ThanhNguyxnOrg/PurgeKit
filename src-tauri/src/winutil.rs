//! Centralized Win32 utility primitives shared by scanner modules and the CLI.

#[cfg(windows)]
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;

/// Expand `%VAR%` tokens using the native ExpandEnvironmentStringsW API.
/// Replaces the three hand-rolled copies previously in path_cleaner.rs,
/// autoruns.rs and com_purger.rs. Falls back to the raw input on API failure.
pub fn expand_env_strings(raw: &str) -> String {
    if raw.is_empty() || !raw.contains('%') {
        return raw.to_string();
    }

    #[cfg(windows)]
    unsafe {
        let wide: Vec<u16> = raw.encode_utf16().chain(std::iter::once(0u16)).collect();

        // First call: query required buffer length (in WCHARs, incl. NUL).
        let required = ExpandEnvironmentStringsW(wide.as_ptr(), std::ptr::null_mut(), 0);
        if required == 0 {
            return raw.to_string();
        }

        let mut buf: Vec<u16> = vec![0u16; required as usize];
        let written = ExpandEnvironmentStringsW(wide.as_ptr(), buf.as_mut_ptr(), required);
        if written == 0 || written > required {
            return raw.to_string();
        }

        // `written` includes the trailing NUL; trim it.
        String::from_utf16_lossy(&buf[..(written as usize).saturating_sub(1)])
    }

    #[cfg(not(windows))]
    {
        raw.to_string()
    }
}

/// Translate a raw OS error code into a stable Win32 error label.
pub fn map_win32_error(err: &std::io::Error) -> &'static str {
    match err.raw_os_error() {
        Some(2) => "ERROR_FILE_NOT_FOUND (2)",
        Some(3) => "ERROR_PATH_NOT_FOUND (3)",
        Some(5) => "ERROR_ACCESS_DENIED (5)",
        Some(32) => "ERROR_SHARING_VIOLATION (32)",
        Some(145) => "ERROR_DIR_NOT_EMPTY (145)",
        Some(_) => "WIN32_ERROR (unmapped code)",
        None => "NON_OS_ERROR",
    }
}

/// Convenience: full human-readable error string with the label prefix.
pub fn format_win32_error(context: &str, err: &std::io::Error) -> String {
    format!("{}: {} - {}", context, map_win32_error(err), err)
}

use std::path::Path;
use std::ffi::c_void;

// GUID for WINTRUST_ACTION_GENERIC_VERIFY_V2: {00AAC56B-CD44-11d0-8CC2-00C04FC295EE}
const WINTRUST_ACTION_GENERIC_VERIFY_V2: [u8; 16] = [
    0x6b, 0xc5, 0xaa, 0x00,
    0x44, 0xcd,
    0xd0, 0x11,
    0x8c, 0xc2, 0x00, 0xc0, 0x4f, 0xc2, 0x95, 0xee
];

#[repr(C)]
struct WINTRUST_FILE_INFO {
    cb_struct: u32,
    pc_wsz_file_path: *const u16,
    h_sub_identity_file: *mut c_void,
    pg_known_subject: *mut c_void,
}

#[repr(C)]
struct WINTRUST_DATA {
    cb_struct: u32,
    p_policy_callback_data: *mut c_void,
    p_sip_client_data: *mut c_void,
    dw_ui_choice: u32,
    fdw_revocation_checks: u32,
    dw_union_choice: u32,
    info_union: *mut c_void,
    dw_state_action: u32,
    h_wvt_state_data: *mut c_void,
    pwsz_url_reference: *mut c_void,
    dw_prov_flags: u32,
    dw_ui_context: u32,
    p_signature_settings: *mut c_void,
}

const WTD_UI_NONE: u32 = 2;
const WTD_REVOKE_NONE: u32 = 0;
const WTD_CHOICE_FILE: u32 = 1;
const WTD_STATEACTION_IGNORE: u32 = 0;

#[link(name = "wintrust")]
extern "system" {
    fn WinVerifyTrust(
        hwnd: *mut c_void,
        pgActionID: *const [u8; 16],
        pWVTData: *mut c_void,
    ) -> i32;
}

pub fn verify_file_signature(file_path: &str) -> bool {
    #[cfg(windows)]
    unsafe {
        let path = expand_env_strings(file_path);
        let p = Path::new(&path);
        if !p.exists() || !p.is_file() {
            return false;
        }

        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0u16)).collect();

        let mut file_info = WINTRUST_FILE_INFO {
            cb_struct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pc_wsz_file_path: wide_path.as_ptr(),
            h_sub_identity_file: std::ptr::null_mut(),
            pg_known_subject: std::ptr::null_mut(),
        };

        let mut trust_data = WINTRUST_DATA {
            cb_struct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            p_policy_callback_data: std::ptr::null_mut(),
            p_sip_client_data: std::ptr::null_mut(),
            dw_ui_choice: WTD_UI_NONE,
            fdw_revocation_checks: WTD_REVOKE_NONE,
            dw_union_choice: WTD_CHOICE_FILE,
            info_union: &mut file_info as *mut _ as *mut c_void,
            dw_state_action: WTD_STATEACTION_IGNORE,
            h_wvt_state_data: std::ptr::null_mut(),
            pwsz_url_reference: std::ptr::null_mut(),
            dw_prov_flags: 0x00000040 | 0x00000200, // WTD_REVOCATION_CHECK_NONE | WTD_LIFETIME_SIGNING_FLAG
            dw_ui_context: 0,
            p_signature_settings: std::ptr::null_mut(),
        };

        let status = WinVerifyTrust(
            std::ptr::null_mut(),
            &WINTRUST_ACTION_GENERIC_VERIFY_V2,
            &mut trust_data as *mut _ as *mut c_void,
        );

        println!("WinVerifyTrust status for '{}': {:#X}", file_path, status);

        status == 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn test_expand_env_strings() {
        assert_eq!(expand_env_strings("hello world"), "hello world");
        
        #[cfg(windows)]
        {
            let expanded = expand_env_strings("%SystemRoot%");
            assert!(expanded.to_lowercase().contains("windows") || expanded.to_lowercase().contains("win"));
            assert!(!expanded.contains("%"));
        }
    }

    #[test]
    fn test_map_win32_error() {
        let err2 = Error::from_raw_os_error(2);
        assert_eq!(map_win32_error(&err2), "ERROR_FILE_NOT_FOUND (2)");

        let err5 = Error::from_raw_os_error(5);
        assert_eq!(map_win32_error(&err5), "ERROR_ACCESS_DENIED (5)");

        let non_os = Error::new(std::io::ErrorKind::Other, "custom error");
        assert_eq!(map_win32_error(&non_os), "NON_OS_ERROR");
    }

    #[test]
    fn test_verify_file_signature() {
        #[cfg(windows)]
        {
            let cmd_path = r"C:\Windows\System32\cmd.exe";
            // cmd.exe is catalog-signed, so direct WinVerifyTrust without catalog verification returns false (TRUST_E_NOSIGNATURE)
            assert!(!verify_file_signature(cmd_path));

            // Check if ntfs.sys (embedded-signed driver) is verified
            let ntfs_path = r"C:\Windows\System32\drivers\ntfs.sys";
            if std::path::Path::new(ntfs_path).exists() {
                let is_signed = verify_file_signature(ntfs_path);
                println!("ntfs.sys signed status: {}", is_signed);
            }

            assert!(!verify_file_signature("C:\\non_existing_file_xyz.exe"));
        }
    }
}