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
}