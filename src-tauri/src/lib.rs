pub mod scanner;
pub mod commands;
pub mod db;
pub mod snapshot_engine;
pub mod settings;
pub mod backup;
pub mod broadcast;
pub mod locker;
pub mod tracker;
pub mod winutil;
pub mod startup_manager;

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
            commands::get_global_cli_packages,
            commands::uninstall_global_cli_package,
            commands::get_cli_package_bin_names,
            commands::get_cli_package_remnants,
            commands::get_settings,
            commands::save_settings,
            commands::check_is_admin,
            commands::start_install_tracking,
            commands::stop_install_tracking,
            commands::get_startup_items,
            commands::set_startup_item_status,
            commands::delete_startup_item,
            commands::list_quarantine_items,
            commands::restore_quarantine_item,
            commands::delete_quarantine_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

