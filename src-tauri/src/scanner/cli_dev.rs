use serde::{Deserialize, Serialize};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::env;
use std::fs;
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

    // List of 12 dev tools to check
    let tool_checks = vec![
        ("npm", vec!["--version"], "npm cache clean --force"),
        ("pnpm", vec!["--version"], "pnpm store prune"),
        ("yarn", vec!["--version"], "yarn cache clean"),
        ("pip", vec!["--version"], "pip cache purge"),
        ("cargo", vec!["--version"], "cargo clean"),
        ("go", vec!["version"], "go clean -cache -modcache"),
        ("bun", vec!["--version"], "bun pm clean"),
        ("deno", vec!["--version"], "deno clean"),
        ("gradle", vec!["--version"], "gradle clean"), // Handled manually
        ("maven", vec!["--version"], "mvn clean"), // Handled manually
        ("dotnet", vec!["--version"], "dotnet nuget locals all --clear"),
        ("docker", vec!["--version"], "docker system prune -f"),
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

        // Check if the command is executable
        // For maven on Windows, the command is 'mvn'
        let cmd_to_run = if name == "maven" { "mvn" } else { name };

        // Run through `cmd /C`: npm, pnpm, yarn, gradle and mvn ship as
        // .cmd/.bat shims on Windows, which CreateProcess (Command::new)
        // cannot launch directly, so detection always failed for them.
        let version_cmd = format!("{} {}", cmd_to_run, args.join(" "));
        if let Ok(output) = Command::new("cmd")
            .creation_flags(CREATE_NO_WINDOW)
            .args(&["/C", &version_cmd])
            .output()
        {
            if output.status.success() {
                info.detected = true;
                let version_str = String::from_utf8_lossy(&output.stdout);
                info.version = Some(version_str.trim().to_string());
                
                // Find executable location
                if let Ok(where_output) = Command::new("where")
                    .creation_flags(CREATE_NO_WINDOW)
                    .arg(cmd_to_run)
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
        "npm" => appdata.map(|p| p.join("npm-cache")),
        "pnpm" => localappdata.clone().map(|p| p.join("pnpm").join("store"))
            .or_else(|| localappdata.map(|p| p.join("pnpm-store"))),
        "yarn" => localappdata.map(|p| p.join("Yarn").join("Cache")),
        "pip" => localappdata.map(|p| p.join("pip").join("cache")),
        "cargo" => userprofile.map(|p| p.join(".cargo")),
        "go" => localappdata.map(|p| p.join("go-build")),
        "bun" => localappdata.map(|p| p.join("bun").join("install").join("cache")),
        "deno" => localappdata.map(|p| p.join("deno")),
        "gradle" => userprofile.clone().map(|p| p.join(".gradle").join("caches")),
        "maven" => userprofile.map(|p| p.join(".m2").join("repository")),
        "dotnet" => userprofile.map(|p| p.join(".nuget").join("packages")),
        _ => None,
    }
}

pub fn calculate_dir_size(path: &Path) -> u64 {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalCliPackage {
    pub name: String,
    pub version: String,
    pub size_bytes: u64,
    pub path: String,
    pub description: Option<String>,
}

pub fn scan_global_npm_packages() -> Vec<GlobalCliPackage> {
    let mut packages = Vec::new();
    let appdata = match env::var_os("APPDATA").map(PathBuf::from) {
        Some(p) => p,
        None => return packages,
    };
    
    let npm_modules_path = appdata.join("npm").join("node_modules");
    if !npm_modules_path.exists() {
        return packages;
    }
    
    if let Ok(entries) = fs::read_dir(&npm_modules_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                
                if name.starts_with('.') {
                    continue;
                }
                
                if name.starts_with('@') {
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                            let sub_path = sub_entry.path();
                            if sub_path.is_dir() {
                                let sub_name = match sub_path.file_name().and_then(|n| n.to_str()) {
                                    Some(sn) => sn.to_string(),
                                    None => continue,
                                };
                                let full_name = format!("{}/{}", name, sub_name);
                                let info = parse_npm_package_info(&sub_path, &full_name);
                                packages.push(info);
                            }
                        }
                    }
                } else {
                    let info = parse_npm_package_info(&path, &name);
                    packages.push(info);
                }
            }
        }
    }
    
    packages
}

fn parse_npm_package_info(path: &Path, name: &str) -> GlobalCliPackage {
    let mut version = "unknown".to_string();
    let mut description = None;
    
    let pkg_json_path = path.join("package.json");
    if pkg_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&pkg_json_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                    version = v.to_string();
                }
                if let Some(d) = json.get("description").and_then(|d| d.as_str()) {
                    description = Some(d.to_string());
                }
            }
        }
    }
    
    let size_bytes = calculate_dir_size(path);
    
    GlobalCliPackage {
        name: name.to_string(),
        version,
        size_bytes,
        path: path.to_string_lossy().to_string(),
        description,
    }
}

pub fn uninstall_global_npm_package(name: &str) -> Result<(), String> {
    // SECURITY: the name is interpolated into a `cmd /C` string. Without
    // validation, a crafted name like "pkg & calc.exe" would be executed as
    // an arbitrary command. Enforce the npm package name charset.
    let is_valid = !name.is_empty()
        && name.len() <= 214
        && !name.starts_with('.')
        && name.matches('/').count() <= 1
        && name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit()
                || matches!(c, '-' | '_' | '.' | '@' | '/' | '~')
        });
    if !is_valid {
        return Err(format!("Invalid npm package name: {}", name));
    }

    let output = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["/C", &format!("npm uninstall -g {}", name)])
        .output()
        .map_err(|e| format!("Failed to execute npm uninstall: {}", e))?;
        
    if !output.status.success() {
        let appdata = env::var_os("APPDATA").map(PathBuf::from)
            .ok_or_else(|| "APPDATA environment variable not found".to_string())?;
        let package_path = appdata.join("npm").join("node_modules").join(name);
        
        if package_path.exists() {
            fs::remove_dir_all(&package_path)
                .map_err(|e| format!("Failed to manually remove package folder: {}", e))?;
        }
        
        let bin_name = if name.contains('/') {
            name.split('/').last().unwrap_or(name)
        } else {
            name
        };
        
        let npm_dir = appdata.join("npm");
        let _ = fs::remove_file(npm_dir.join(bin_name));
        let _ = fs::remove_file(npm_dir.join(format!("{}.cmd", bin_name)));
        let _ = fs::remove_file(npm_dir.join(format!("{}.ps1", bin_name)));
    }
    
    Ok(())
}
