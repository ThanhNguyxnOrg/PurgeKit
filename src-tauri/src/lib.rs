pub mod scanner;
pub mod commands;
pub mod db;
pub mod snapshot_engine;
pub mod settings;
pub mod backup;
pub mod broadcast;
pub mod locker;
pub mod tracker;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize Database
    let _ = db::init_db();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(tracker::ActiveTracking(std::sync::Arc::new(std::sync::Mutex::new(None))))
        .invoke_handler(tauri::generate_handler![
            commands::get_installed_apps,
            commands::get_dev_tools,
            commands::clean_dev_tool_cache,
            commands::get_app_remnants,
            commands::purge_remnants,
            commands::take_snapshot,
            commands::list_snapshots,
            commands::delete_snapshot,
            commands::compare_snapshots,
            commands::get_path_entries,
            commands::save_path_entries,
            commands::run_uninstall_command,
            commands::get_global_npm_packages,
            commands::uninstall_global_npm_package,
            commands::get_settings,
            commands::save_settings,
            commands::check_is_admin,
            commands::start_install_tracking,
            commands::stop_install_tracking
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

