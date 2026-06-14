//! Centralized Win32 utility primitives shared by scanner modules and the CLI.

#[cfg(windows)]
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
#[cfg(windows)]
use winreg::enums::KEY_READ;
#[cfg(windows)]
use winreg::RegKey;

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

/// Retrieve the secure, system-only PATH variable from HKLM, falling back to a hardcoded safe default.
pub fn get_secure_system_path() -> String {
    #[cfg(windows)]
    {
        let hk = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
        let subpath = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
        if let Ok(key) = hk.open_subkey_with_flags(subpath, KEY_READ) {
            if let Ok(val) = key.get_value::<String, _>("PATH").or_else(|_| key.get_value("Path")) {
                let expanded = expand_env_strings(&val);
                return expanded;
            }
        }
    }
    r"C:\Windows\System32;C:\Windows;C:\Windows\System32\Wbem;C:\Windows\System32\WindowsPowerShell\v1.0\".to_string()
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

use std::collections::HashSet;
use std::path::PathBuf;

/// Extract the executable path from a command line string, handling quotes.
pub fn extract_executable_path(cmd: &str) -> String {
    let trimmed = cmd.trim();
    if trimmed.starts_with('"') {
        if let Some(end) = trimmed[1..].find('"') {
            return trimmed[1..end + 1].to_string();
        }
    }
    if let Some(space_idx) = trimmed.find(' ') {
        return trimmed[..space_idx].to_string();
    }
    trimmed.to_string()
}

/// Canonicalize path with fallback to manual components traversal to prevent TOCTOU/Traversal bypasses.
pub fn canonicalize_path_safety(path_str: &str) -> PathBuf {
    let expanded = expand_env_strings(path_str);
    let path = Path::new(&expanded);
    if let Ok(canon) = std::fs::canonicalize(path) {
        let canon_str = canon.to_string_lossy().to_string();
        let stripped = if canon_str.starts_with(r"\\?\") {
            canon_str[4..].to_string()
        } else {
            canon_str
        };
        PathBuf::from(stripped)
    } else {
        let mut base = if path.is_absolute() {
            PathBuf::new()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd
        } else {
            PathBuf::new()
        };

        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) => {
                    base.push(component.as_os_str());
                }
                std::path::Component::RootDir => {
                    base.push(component.as_os_str());
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    base.pop();
                }
                std::path::Component::Normal(c) => {
                    base.push(c);
                }
            }
        }
        base
    }
}

/// Centralized safety gate to check if a file system path is safe to delete or quarantine.
pub fn is_safe_to_delete(path_str: &str) -> Result<(), String> {
    let expanded = expand_env_strings(path_str);
    let target = canonicalize_path_safety(&expanded);
    
    // Case-insensitive comparisons for Windows
    let target_str = target.to_string_lossy().to_string().to_lowercase();
    let target_path = Path::new(&target_str);

    let system_root = std::env::var("SystemRoot")
        .unwrap_or_else(|_| std::env::var("windir").unwrap_or_else(|_| r"C:\Windows".to_string()))
        .to_lowercase();
    let system_drive = std::env::var("SystemDrive")
        .unwrap_or_else(|_| r"C:".to_string())
        .to_lowercase() + "\\";
    let program_files = std::env::var("ProgramFiles")
        .unwrap_or_else(|_| r"C:\Program Files".to_string())
        .to_lowercase();
    let program_files_x86 = std::env::var("ProgramFiles(x86)")
        .unwrap_or_else(|_| r"C:\Program Files (x86)".to_string())
        .to_lowercase();
    let program_data = std::env::var("ProgramData")
        .unwrap_or_else(|_| r"C:\ProgramData".to_string())
        .to_lowercase();
    let user_profile = std::env::var("USERPROFILE")
        .unwrap_or_else(|_| r"C:\Users\Default".to_string())
        .to_lowercase();
    let app_data = std::env::var("APPDATA")
        .unwrap_or_else(|_| format!(r"{}\appdata\roaming", user_profile))
        .to_lowercase();
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| format!(r"{}\appdata\local", user_profile))
        .to_lowercase();

    let mut blacklisted_exact = HashSet::new();

    // 1. System Roots and Drive Roots
    blacklisted_exact.insert(system_drive.trim_end_matches('\\').to_string());
    blacklisted_exact.insert(system_drive.clone());
    
    for drive in b'a'..=b'z' {
        let drive_str = format!("{}:", drive as char);
        blacklisted_exact.insert(drive_str.clone());
        blacklisted_exact.insert(drive_str + "\\");
    }

    // 2. Core Windows Directories
    blacklisted_exact.insert(system_root.clone());
    
    // 3. Core Program Directories
    blacklisted_exact.insert(program_files.clone());
    blacklisted_exact.insert(program_files_x86.clone());
    blacklisted_exact.insert(program_data.clone());
    
    let common_files = format!(r"{}\common files", program_files);
    blacklisted_exact.insert(common_files);
    let common_files_x86 = format!(r"{}\common files", program_files_x86);
    blacklisted_exact.insert(common_files_x86);

    // 4. Core User Folders
    let users_dir = format!(r"{}\..", user_profile);
    let users_dir_canon = canonicalize_path_safety(&users_dir).to_string_lossy().to_string().to_lowercase();
    blacklisted_exact.insert(users_dir_canon.clone());
    blacklisted_exact.insert(users_dir_canon.clone() + "\\");
    
    blacklisted_exact.insert(format!(r"{}\public", users_dir_canon));
    blacklisted_exact.insert(format!(r"{}\default", users_dir_canon));
    blacklisted_exact.insert(user_profile.clone());
    
    let user_desktop = format!(r"{}\desktop", user_profile);
    let user_documents = format!(r"{}\documents", user_profile);
    let user_downloads = format!(r"{}\downloads", user_profile);
    let user_pictures = format!(r"{}\pictures", user_profile);
    let user_music = format!(r"{}\music", user_profile);
    let user_videos = format!(r"{}\videos", user_profile);
    let user_appdata = format!(r"{}\appdata", user_profile);

    blacklisted_exact.insert(user_desktop);
    blacklisted_exact.insert(user_documents);
    blacklisted_exact.insert(user_downloads);
    blacklisted_exact.insert(user_pictures);
    blacklisted_exact.insert(user_music);
    blacklisted_exact.insert(user_videos);
    blacklisted_exact.insert(user_appdata);
    blacklisted_exact.insert(app_data);
    blacklisted_exact.insert(local_app_data.clone());
    
    let local_temp = format!(r"{}\temp", local_app_data);
    blacklisted_exact.insert(local_temp);

    // System Temp/Prefetch roots
    blacklisted_exact.insert(format!(r"{}\temp", system_root));
    blacklisted_exact.insert(format!(r"{}\prefetch", system_root));

    // 5. Critical Root System Files
    let root_files = vec![
        "pagefile.sys", "swapfile.sys", "hiberfil.sys", "bootmgr", "bootnxt", "ntldr", 
        "system volume information", "$recycle.bin", "recovery", "boot"
    ];
    for rf in root_files {
        let fpath = format!("{}{}", system_drive, rf);
        blacklisted_exact.insert(fpath);
    }

    // 6. Developer Toolchains Home Directories
    let dev_homes = vec![
        ".cargo", ".rustup", ".m2", ".gradle", "go", ".conda", "anaconda3", "miniconda3"
    ];
    for dh in dev_homes {
        let dpath = format!(r"{}\{}", user_profile, dh);
        blacklisted_exact.insert(dpath);
    }

    // Check exact matches
    if blacklisted_exact.contains(&target_str) {
        return Err(format!("Deletion blocked: Path '{}' is a protected system or root directory.", path_str));
    }

    // Check if target is a parent of any protected path
    for blacklisted in &blacklisted_exact {
        let b_path = Path::new(blacklisted);
        if b_path.starts_with(target_path) {
            return Err(format!("Deletion blocked: Path '{}' is a parent of protected path '{}'.", path_str, blacklisted));
        }
    }

    // 7. Check if resides inside SystemRoot (outside Temp or Prefetch)
    let system_root_path = Path::new(&system_root);
    if target_path.starts_with(system_root_path) {
        let sys_temp = format!(r"{}\temp", system_root);
        let sys_prefetch = format!(r"{}\prefetch", system_root);
        let sys_temp_path = Path::new(&sys_temp);
        let sys_prefetch_path = Path::new(&sys_prefetch);
        
        if !target_path.starts_with(sys_temp_path) && !target_path.starts_with(sys_prefetch_path) {
            return Err(format!("Deletion blocked: Path '{}' is inside the protected SystemRoot (outside Temp or Prefetch).", path_str));
        }
    }

    // 8. Prevent deleting registry hive files: ntuser.dat, usrclass.dat (and logs)
    let name_lower = target.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name_lower.starts_with("ntuser.dat") || name_lower.starts_with("usrclass.dat") {
        if target_path.starts_with(Path::new(&user_profile)) {
            return Err(format!("Deletion blocked: User registry hive file '{}' cannot be deleted.", path_str));
        }
    }

    Ok(())
}

/// Centralized safety gate to check if a registry key is safe to delete.
pub fn is_safe_registry_key(hive_name: &str, subpath: &str) -> Result<(), String> {
    let hive = hive_name.to_lowercase();
    let sub = subpath.replace('/', "\\").to_lowercase();
    
    let normalized_hive = match hive.as_str() {
        "hkey_local_machine" | "hklm" => "hklm",
        "hkey_current_user" | "hkcu" => "hkcu",
        "hkey_classes_root" | "hkcr" => "hkcr",
        "hkey_users" | "hku" => "hku",
        _ => &hive,
    };

    let full_path = if sub.is_empty() {
        normalized_hive.to_string()
    } else {
        format!(r"{}\{}", normalized_hive, sub.trim_matches('\\'))
    };

    let exact_and_parents = [
        "hklm",
        "hkcu",
        "hkcr",
        "hku",
        "hklm\\software",
        "hkcu\\software",
        "hklm\\software\\classes",
        "hkcu\\software\\classes",
        "hklm\\software\\clients",
        "hkcu\\software\\clients",
        "hklm\\software\\policies",
        "hkcu\\software\\policies",
        "hklm\\software\\wow6432node",
        "hklm\\system",
        "hklm\\system\\currentcontrolset",
        "hklm\\system\\currentcontrolset\\services",
        "hklm\\sam",
        "hklm\\security",
        "hklm\\hardware",
    ];

    let subkey_protected = [
        "hklm\\software\\microsoft",
        "hkcu\\software\\microsoft",
        "hklm\\software\\policies",
        "hkcu\\software\\policies",
    ];

    let allowed_exceptions = [
        "hklm\\software\\microsoft\\windows\\currentversion\\run\\",
        "hkcu\\software\\microsoft\\windows\\currentversion\\run\\",
        "hklm\\software\\microsoft\\windows\\currentversion\\runonce\\",
        "hkcu\\software\\microsoft\\windows\\currentversion\\runonce\\",
        "hklm\\software\\wow6432node\\microsoft\\windows\\currentversion\\run\\",
        "hklm\\software\\wow6432node\\microsoft\\windows\\currentversion\\runonce\\",
        "hklm\\software\\microsoft\\windows\\currentversion\\uninstall\\",
        "hkcu\\software\\microsoft\\windows\\currentversion\\uninstall\\",
        "hklm\\software\\wow6432node\\microsoft\\windows\\currentversion\\uninstall\\",
    ];

    if exact_and_parents.contains(&full_path.as_str()) {
        return Err(format!("Registry deletion blocked: Key '{}' is a protected system root.", full_path));
    }

    for p in &exact_and_parents {
        if p.starts_with(&format!("{}\\", full_path)) {
            return Err(format!("Registry deletion blocked: Key '{}' is a parent of protected key '{}'.", full_path, p));
        }
    }

    let mut is_exception = false;
    for exc in &allowed_exceptions {
        if full_path.starts_with(exc) {
            is_exception = true;
            break;
        }
    }

    if !is_exception {
        if subkey_protected.contains(&full_path.as_str()) {
            return Err(format!("Registry deletion blocked: Key '{}' is a protected system root.", full_path));
        }
        for p in &subkey_protected {
            if p.starts_with(&format!("{}\\", full_path)) {
                return Err(format!("Registry deletion blocked: Key '{}' is a parent of protected key '{}'.", full_path, p));
            }
        }
        for p in &subkey_protected {
            if full_path.starts_with(&format!("{}\\", p)) {
                return Err(format!("Registry deletion blocked: Key '{}' resides under protected key '{}'.", full_path, p));
            }
        }
    }

    Ok(())
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

    #[test]
    fn test_is_safe_to_delete() {
        // Safe relative paths or hypothetical safe paths
        assert!(is_safe_to_delete(r"C:\Users\Default\AppData\Local\Temp\some_app_temp").is_ok());
        assert!(is_safe_to_delete(r"C:\Users\Default\AppData\Local\Temp\some_app_temp\..\other").is_ok());

        // Dangerous paths: System Drive / Root
        assert!(is_safe_to_delete(r"C:").is_err());
        assert!(is_safe_to_delete(r"C:\").is_err());
        assert!(is_safe_to_delete(r"D:").is_err());
        
        // Dangerous paths: System directories
        assert!(is_safe_to_delete(r"C:\Windows").is_err());
        assert!(is_safe_to_delete(r"C:\Windows\System32").is_err());
        assert!(is_safe_to_delete(r"C:\Windows\System32\drivers\etc").is_err());
        assert!(is_safe_to_delete(r"C:\Windows\System32\..\System32").is_err()); // Traversal check
        
        // Allowed inside Temp
        assert!(is_safe_to_delete(r"C:\Windows\Temp\test_file.txt").is_ok());
        
        // Dangerous paths: User directories and roots
        assert!(is_safe_to_delete(r"C:\Users").is_err());
        assert!(is_safe_to_delete(r"C:\Users\Public").is_err());
        
        // Developer toolchains
        assert!(is_safe_to_delete(r"%USERPROFILE%\.cargo").is_err());
        assert!(is_safe_to_delete(r"%USERPROFILE%\.rustup").is_err());

        // Registry hive files
        assert!(is_safe_to_delete(r"%USERPROFILE%\ntuser.dat").is_err());
    }

    #[test]
    fn test_is_safe_registry_key() {
        // Safe key
        assert!(is_safe_registry_key("HKCU", r"Software\SomeApp").is_ok());
        assert!(is_safe_registry_key("HKLM", r"Software\Classes\CLSID\{12345678-1234-1234-1234-1234567890AB}").is_ok());

        // Dangerous exact keys
        assert!(is_safe_registry_key("HKLM", "").is_err());
        assert!(is_safe_registry_key("HKLM", "Software").is_err());
        assert!(is_safe_registry_key("HKLM", "Software\\Classes").is_err());

        // Dangerous subkeys under Microsoft / Policies
        assert!(is_safe_registry_key("HKLM", "Software\\Microsoft").is_err());
        assert!(is_safe_registry_key("HKLM", "Software\\Microsoft\\Windows").is_err());
        assert!(is_safe_registry_key("HKCU", "Software\\Microsoft\\Office").is_err());
        assert!(is_safe_registry_key("HKLM", "Software\\Policies\\Microsoft").is_err());

        // Exception subkeys (Run / RunOnce / Uninstall)
        assert!(is_safe_registry_key("HKLM", r"Software\Microsoft\Windows\CurrentVersion\Run\MyStartup").is_ok());
        assert!(is_safe_registry_key("HKLM", r"Software\Microsoft\Windows\CurrentVersion\Run").is_err()); // exactly Run is blocked
        assert!(is_safe_registry_key("HKLM", r"Software\Microsoft\Windows\CurrentVersion\Uninstall\SomeApp").is_ok());
        assert!(is_safe_registry_key("HKLM", r"Software\Microsoft\Windows\CurrentVersion\Uninstall").is_err()); // exactly Uninstall is blocked

        // Parent block
        assert!(is_safe_registry_key("HKLM", "Software").is_err());
    }
}