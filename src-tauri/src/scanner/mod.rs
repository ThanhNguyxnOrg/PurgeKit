pub mod registry;
pub mod uwp;
pub mod cli_dev;
pub mod remnants;
pub mod path_cleaner;

pub use registry::{InstalledApp, scan_registry};
pub use uwp::scan_uwp_apps;
pub use cli_dev::{DevToolInfo, scan_dev_tools, GlobalCliPackage, scan_global_npm_packages, uninstall_global_npm_package};
pub use remnants::{RemnantItem, scan_app_remnants, purge_all_remnants};
pub use path_cleaner::{PathEntry, get_path_entries, set_path_entries};

pub fn scan_all_apps() -> Vec<InstalledApp> {
    let mut apps = scan_registry();
    let uwp_apps = scan_uwp_apps();
    apps.extend(uwp_apps);
    
    // Sort apps alphabetically by display name
    apps.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    
    apps
}
