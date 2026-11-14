//! PurgeKit standalone CLI. Console subsystem binary (no windows_subsystem
//! attribute) so stdout/stderr work natively in release builds.

use clap::{Parser, Subcommand};
use purgekit_lib::scanner::cli_dev::{prune_dev_tool_cache, scan_dev_tools};
use purgekit_lib::scanner::path_cleaner::{get_path_entries, prune_broken_path_entries};
use purgekit_lib::scanner::remnants::{purge_remnant_item, scan_app_remnants};
use purgekit_lib::scanner::wsl_shrinker::{
    compact_vhdx_core, scan_wsl_distributions, set_wsl_distro_sparse_core,
};
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
    #[command(about = "Manage and compact WSL2 virtual disks")]
    Wsl {
        #[command(subcommand)]
        action: WslCommands,
    },
    #[command(about = "Clean developer tool caches, build outputs and package registries")]
    Cache {
        #[command(subcommand)]
        action: CacheCommands,
    },
    #[command(about = "Inspect and repair Windows User/System PATH entries")]
    CleanPath {
        #[arg(long, help = "Remove entries whose directory no longer exists")]
        prune_broken: bool,
    },
}

#[derive(Subcommand)]
enum CacheCommands {
    #[command(about = "Prune caches for the listed tools (or every detected tool with --all)")]
    Prune {
        #[arg(help = "Tool cache names (e.g. npm pip cargo go yarn pnpm composer gradle vscode)")]
        tools: Vec<String>,

        #[arg(long, help = "Prune every detected developer tool cache")]
        all: bool,
    },
}

#[derive(Subcommand)]
enum WslCommands {
    #[command(about = "List all registered WSL distributions, their disk paths and sizes")]
    List,
    #[command(about = "Shut down WSL and compact the distribution's VHDX using DiskPart")]
    Compact {
        #[arg(help = "The exact WSL distribution name")]
        distro: String,
    },
    #[command(about = "Set the NTFS sparse (auto-shrink) attribute on the distribution's VHDX")]
    Sparse {
        #[arg(help = "The exact WSL distribution name")]
        distro: String,
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

    if let Err(e) = purgekit_lib::db::init_db() {
        eprintln!("ERROR: Failed to initialize quarantine database: {}", e);
        std::process::exit(1);
    }

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
        Commands::Wsl { action } => match action {
            WslCommands::List => {
                let distros = scan_wsl_distributions();
                if distros.is_empty() {
                    println!("No WSL distributions found.");
                }
                for distro in &distros {
                    println!("{}  [{}]", distro.name, distro.id);
                    println!("  Base path: {}", distro.base_path);
                    match &distro.vhdx_path {
                        Some(vhdx) => println!(
                            "  VHDX: {} ({:.2} GB{})",
                            vhdx,
                            distro.vhdx_size_bytes as f64 / 1_073_741_824.0,
                            if distro.is_sparse { ", sparse" } else { "" }
                        ),
                        None => println!("  VHDX: not found"),
                    }
                }
            }
            WslCommands::Compact { distro } => {
                let target = match scan_wsl_distributions().into_iter().find(|d| d.name == distro) {
                    Some(target) => target,
                    None => {
                        eprintln!(
                            "ERROR: WSL distribution '{}' not found. Run 'purgekit-cli wsl list' first.",
                            distro
                        );
                        std::process::exit(1);
                    }
                };
                let vhdx_path = match target.vhdx_path {
                    Some(path) => path,
                    None => {
                        eprintln!("ERROR: No VHDX file found for '{}'.", target.name);
                        std::process::exit(1);
                    }
                };

                println!("Compacting '{}' VHDX: {}", target.name, vhdx_path);
                let mut on_progress = |phase: &str, message: &str| {
                    println!("  [{}] {}", phase, message.trim_end());
                };
                match compact_vhdx_core(&vhdx_path, &mut on_progress) {
                    Ok(_) => println!("Compaction completed."),
                    Err(error) => {
                        eprintln!("ERROR: {}", error);
                        std::process::exit(1);
                    }
                }
            }
            WslCommands::Sparse { distro } => match set_wsl_distro_sparse_core(&distro, true) {
                Ok(()) => println!("'{}' is now running in sparse (auto-shrink) mode.", distro),
                Err(error) => {
                    eprintln!("ERROR: {}", error);
                    std::process::exit(1);
                }
            },
        },
        Commands::Cache { action } => match action {
            CacheCommands::Prune { tools, all } => {
                let available = scan_dev_tools();
                let requested: Vec<String> = if all {
                    available
                        .iter()
                        .filter(|tool| tool.detected)
                        .map(|tool| tool.name.clone())
                        .collect()
                } else {
                    tools
                };

                if requested.is_empty() {
                    eprintln!("ERROR: no tools requested. Pass tool names or --all.");
                    std::process::exit(1);
                }

                let mut total_freed: u64 = 0;
                let mut failures = 0u32;

                for name in &requested {
                    match available.iter().find(|tool| tool.name == *name) {
                        None => {
                            eprintln!("  [{}] SKIP (unknown tool; see docs/CLI.md for valid names)", name);
                            failures += 1;
                        }
                        Some(tool) if !tool.detected => {
                            println!("  [{}] SKIP (not installed)", name);
                        }
                        Some(tool) => {
                            println!(
                                "  [{}] cache size: {:.1} MB",
                                name,
                                tool.cache_size.unwrap_or(0) as f64 / 1_048_576.0
                            );
                            let mut on_progress = |phase: &str, percent: u32, message: &str| {
                                println!("    [{:>9} {:>3}%] {}", phase, percent, message.trim_end());
                            };
                            match prune_dev_tool_cache(name, &mut on_progress) {
                                Ok(freed) => {
                                    println!("  [{}] freed {:.1} MB", name, freed as f64 / 1_048_576.0);
                                    total_freed += freed;
                                }
                                Err(error) => {
                                    eprintln!("  [{}] FAILED: {}", name, error);
                                    failures += 1;
                                }
                            }
                        }
                    }
                }

                println!("Total freed: {:.1} MB", total_freed as f64 / 1_048_576.0);
                if failures > 0 {
                    eprintln!("{} tool(s) failed.", failures);
                    std::process::exit(1);
                }
            }
        },
        Commands::CleanPath { prune_broken } => {
            if !prune_broken {
                // Report-only mode: show the current state of both scopes.
                let entries = match get_path_entries() {
                    Ok(entries) => entries,
                    Err(error) => {
                        eprintln!("ERROR: {}", error);
                        std::process::exit(1);
                    }
                };
                for entry in &entries {
                    let status = if !entry.is_valid {
                        "BROKEN"
                    } else if entry.is_duplicate {
                        "DUP"
                    } else {
                        "OK"
                    };
                    println!("  [{:>6}][{:>6}] {}", entry.scope, status, entry.value);
                }
                println!("{} PATH entries inspected.", entries.len());
            } else {
                let removed = match prune_broken_path_entries() {
                    Ok(removed) => removed,
                    Err(error) => {
                        eprintln!("ERROR: {}", error);
                        std::process::exit(1);
                    }
                };
                if removed.is_empty() {
                    println!("No broken PATH entries found.");
                } else {
                    for entry in &removed {
                        println!("  [{}] removed: {}", entry.scope, entry.value);
                    }
                    println!("Removed {} broken PATH entr{}.", removed.len(), if removed.len() == 1 { "y" } else { "ies" });
                }
            }
        }
    }
}
