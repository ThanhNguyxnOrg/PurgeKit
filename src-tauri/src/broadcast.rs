use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG
};
use windows_sys::Win32::UI::Shell::{
    SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST
};

pub fn broadcast_environment_change() {
    unsafe {
        let env_str = "Environment\0";
        let env_wide: Vec<u16> = env_str.encode_utf16().collect();
        let mut result = 0;
        
        // SendMessageTimeoutW to notify that environment variables changed
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_wide.as_ptr() as _,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

pub fn broadcast_shell_refresh() {
    unsafe {
        // Force Explorer to reload file associations and icons
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );

    }
}

pub fn broadcast_all_system_changes() {
    broadcast_environment_change();
    broadcast_shell_refresh();
}
