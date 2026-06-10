use serde::{Deserialize, Serialize};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::env;
use walkdir::WalkDir;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DevToolInfo {
    pub name: String,
    pub detected: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub cache_path: Option<String>,
    pub cache_size: Option<u64>,
    pub clean_command: Option<String>,
}

pub fn scan_dev_tools() -> Vec<DevToolInfo> {
    let mut tools = Vec::new();

    // List of dev tools to check
    let tool_checks = vec![
        ("npm", vec!["--version"], "npm cache clean --force"),
        ("pnpm", vec!["--version"], "pnpm store prune"),
        ("yarn", vec!["--version"], "yarn cache clean"),
        ("pip", vec!["--version"], "pip cache purge"),
        ("cargo", vec!["--version"], "cargo clean"), // We can handle cargo specific cleaning manually too
        ("go", vec!["version"], "go clean -cache -modcache"),
    ];

    for (name, args, clean_cmd) in tool_checks {
        let mut info = DevToolInfo {
            name: name.to_string(),
            detected: false,
            version: None,
            path: None,
            cache_path: None,
            cache_size: None,
            clean_command: Some(clean_cmd.to_string()),
        };

        // Try to execute the tool to check if it exists and get version
        if let Ok(output) = Command::new(name)
            .creation_flags(CREATE_NO_WINDOW)
            .args(&args)
            .output()
        {
            if output.status.success() {
                info.detected = true;
                let version_str = String::from_utf8_lossy(&output.stdout);
                info.version = Some(version_str.trim().to_string());
                
                // Get command path (like `where npm` on Windows)
                if let Ok(where_output) = Command::new("where")
                    .creation_flags(CREATE_NO_WINDOW)
                    .arg(name)
                    .output()
                {
                    if where_output.status.success() {
                        let paths = String::from_utf8_lossy(&where_output.stdout);
                        let first_path = paths.lines().next().unwrap_or("").trim();
                        if !first_path.is_empty() {
                            info.path = Some(first_path.to_string());
                        }
                    }
                }
            }
        }

        // If detected, find cache path and calculate its size
        if info.detected {
            let cache_path = get_cache_path(name);
            if let Some(ref path) = cache_path {
                if path.exists() {
                    info.cache_path = Some(path.to_string_lossy().to_string());
                    // Compute size in a separate step or synchronously (we'll do sync here since it's simple)
                    info.cache_size = Some(calculate_dir_size(path));
                }
            }
        }

        tools.push(info);
    }

    tools
}

fn get_cache_path(name: &str) -> Option<PathBuf> {
    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let userprofile = env::var_os("USERPROFILE").map(PathBuf::from);

    match name {
        "npm" => {
            // Default npm cache: %AppData%\npm-cache
            appdata.map(|p| p.join("npm-cache"))
        }
        "pnpm" => {
            // Default pnpm cache: %LocalAppData%\pnpm-store or %LocalAppData%\pnpm\store
            localappdata.clone().map(|p| p.join("pnpm").join("store"))
                .or_else(|| localappdata.map(|p| p.join("pnpm-store")))
        }
        "yarn" => {
            // Default yarn cache: %LocalAppData%\Yarn\Cache
            localappdata.map(|p| p.join("Yarn").join("Cache"))
        }
        "pip" => {
            // Default pip cache: %LocalAppData%\pip\Cache
            localappdata.map(|p| p.join("pip").join("cache"))
        }
        "cargo" => {
            // Default cargo registry cache: %UserProfile%\.cargo\registry\cache
            userprofile.map(|p| p.join(".cargo"))
        }
        "go" => {
            // Default go build cache: %LocalAppData%\go-build
            localappdata.map(|p| p.join("go-build"))
        }
        _ => None,
    }
}

pub fn calculate_dir_size(path: &Path) -> u64 {
    // Basic walk to count file sizes.
    // In production, we can optimize this with rayon, but walkdir is fast enough for standard caches.
    let mut total_size = 0;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
            }
        }
    }
    total_size
}
