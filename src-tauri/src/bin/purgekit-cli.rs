//! PurgeKit standalone CLI. Console subsystem binary (no windows_subsystem
//! attribute) so stdout/stderr work natively in release builds.

use clap::{Parser, Subcommand};
use purgekit_lib::scanner::remnants::{purge_remnant_item, scan_app_remnants, RemnantItem};
use std::io::Write;

#[derive(Parser)]
#[command(
    name = "purgekit-cli",
    version,
    about = "PurgeKit CLI - deep remnant scanner & purger (Administrator required)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Clean {
        #[arg(help = "The name of the application to scan and clean remnants for")]
        app_name: String,

        #[arg(long, help = "Only show remnants but do not delete them")]
        dry_run: bool,

        #[arg(long, default_value_t = 55, help = "Minimum confidence score threshold to delete")]
        min_score: i32,

        #[arg(long, short = 'y', help = "Skip confirmation prompt")]
        yes: bool,
    },
}

fn main() {
    // Hardening: DLL hijacking defense
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::LibraryLoader::{
            SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32,
        };
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
    }

    // Privilege check
    if !is_elevated::is_elevated() {
        eprintln!("ERROR: Administrator privileges are required to run PurgeKit CLI.");
        std::process::exit(1);
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Clean {
            app_name,
            dry_run,
            min_score,
            yes,
        } => {
            println!("PurgeKit CLI - Initiating deep remnants clean for: {}", app_name);
            println!("Score threshold: {}", min_score);
            
            let remnants = scan_app_remnants(&app_name, None, None);
            println!("Analysis completed. Found {} remnant items.", remnants.len());

            if remnants.is_empty() {
                println!("No remnant items found.");
                return;
            }

            let mut filtered_remnants = Vec::new();
            for item in &remnants {
                let status_label = if item.score >= min_score { "TO BE PURGED" } else { "SKIPPED (low score)" };
                println!("  [{}] [{}] (Score: {}) {}", item.item_type, status_label, item.score, item.path);
                if item.score >= min_score {
                    filtered_remnants.push(item.clone());
                }
            }

            if filtered_remnants.is_empty() {
                println!("No remnant items match the minimum score threshold of {}.", min_score);
                return;
            }

            println!("\nSummary: {} of {} items will be purged.", filtered_remnants.len(), remnants.len());

            if dry_run {
                println!("[Dry Run] Dry run active. No changes were made.");
                return;
            }

            if !yes {
                print!("Proceed with purging these {} items? (y/N): ", filtered_remnants.len());
                let _ = std::io::stdout().flush();
                let mut input = String::new();
                if std::io::stdin().read_line(&mut input).is_err() {
                    println!("Error reading input. Aborting.");
                    std::process::exit(1);
                }
                let input = input.trim().to_lowercase();
                if input != "y" && input != "yes" {
                    println!("Aborted by user.");
                    return;
                }
            }

            println!("Purging remnants...");
            let mut success_count = 0;
            let mut fail_count = 0;

            for item in &filtered_remnants {
                print!("Purging [{}] {} ... ", item.item_type, item.path);
                let _ = std::io::stdout().flush();
                match purge_remnant_item(item) {
                    Ok(_) => {
                        println!("OK");
                        success_count += 1;
                    }
                    Err(e) => {
                        println!("FAILED ({})", e);
                        fail_count += 1;
                    }
                }
            }

            println!("Purge completed. Success: {}, Failed: {}", success_count, fail_count);
        }
    }
}
