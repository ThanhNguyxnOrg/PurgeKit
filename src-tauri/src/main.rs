// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "clean" {
        if args.len() > 2 {
            let app_name = &args[2];
            println!("PurgeKit CLI - Initiating deep remnants clean for: {}", app_name);
            
            // Call scanner from the library
            let remnants = tauri_app_lib::scanner::remnants::scan_app_remnants(app_name, None, None);
            println!("Analysis completed. Found {} remnant items.", remnants.len());
            
            for item in &remnants {
                println!("  [{}] {}", item.item_type, item.path);
            }
            
            println!("Purging remnants...");
            let (success, fail) = tauri_app_lib::scanner::remnants::purge_all_remnants(&remnants);
            println!("Purge completed. Success: {}, Failed: {}", success, fail);
        } else {
            println!("Usage: purgekit.exe clean <app-name>");
        }
        return;
    }

    tauri_app_lib::run()
}
