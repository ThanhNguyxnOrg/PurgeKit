use serde::{Deserialize, Serialize};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::env;
use std::fs;
use walkdir::WalkDir;
use super::remnants::RemnantItem;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn get_secure_system_path() -> String {
    crate::winutil::get_secure_system_path()
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DevToolRule {
    pub name: String,
    pub cmd_to_run: Option<String>,
    pub args: Vec<String>,
    pub clean_command: Option<String>,
    pub cache_path_templates: Option<Vec<String>>,
    pub dynamic_cache_cmd: Option<String>,
}

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

pub fn get_devtools_rules_path() -> PathBuf {
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir());
    localappdata.join("PurgeKit").join("devtools_rules.json")
}

pub fn resolve_template_path(template: &str) -> Option<PathBuf> {
    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let userprofile = env::var_os("USERPROFILE").map(PathBuf::from);

    let mut path_str = template.to_string();
    if path_str.contains("{APPDATA}") {
        if let Some(ref p) = appdata {
            path_str = path_str.replace("{APPDATA}", &p.to_string_lossy());
        } else {
            return None;
        }
    }
    if path_str.contains("{LOCALAPPDATA}") {
        if let Some(ref p) = localappdata {
            path_str = path_str.replace("{LOCALAPPDATA}", &p.to_string_lossy());
        } else {
            return None;
        }
    }
    if path_str.contains("{USERPROFILE}") {
        if let Some(ref p) = userprofile {
            path_str = path_str.replace("{USERPROFILE}", &p.to_string_lossy());
        } else {
            return None;
        }
    }

    Some(PathBuf::from(path_str))
}

pub fn get_single_dynamic_cache_path(name: &str, cmd: &str) -> Option<PathBuf> {
    let secure_path = get_secure_system_path();
    if let Ok(output) = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .env("PATH", &secure_path)
        .args(&["/C", cmd])
        .output()
    {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn get_default_rules() -> Vec<DevToolRule> {
    vec![
        DevToolRule {
            name: "npm".to_string(),
            cmd_to_run: Some("npm".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("npm cache clean --force".to_string()),
            cache_path_templates: Some(vec!["{APPDATA}\\npm-cache".to_string()]),
            dynamic_cache_cmd: Some("npm config get cache".to_string()),
        },
        DevToolRule {
            name: "pnpm".to_string(),
            cmd_to_run: Some("pnpm".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("pnpm store prune".to_string()),
            cache_path_templates: Some(vec![
                "{LOCALAPPDATA}\\pnpm\\store".to_string(),
                "{LOCALAPPDATA}\\pnpm-store".to_string(),
            ]),
            dynamic_cache_cmd: Some("pnpm store path".to_string()),
        },
        DevToolRule {
            name: "yarn".to_string(),
            cmd_to_run: Some("yarn".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("yarn cache clean".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\Yarn\\Cache".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "pip".to_string(),
            cmd_to_run: Some("pip".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("pip cache purge".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\pip\\cache".to_string()]),
            dynamic_cache_cmd: Some("pip cache dir".to_string()),
        },
        DevToolRule {
            name: "cargo".to_string(),
            cmd_to_run: Some("cargo".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("cargo clean".to_string()),
            cache_path_templates: Some(vec![
                "{USERPROFILE}\\.cargo\\registry\\cache".to_string(),
                "{USERPROFILE}\\.cargo\\git\\db".to_string(),
            ]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "go".to_string(),
            cmd_to_run: Some("go".to_string()),
            args: vec!["version".to_string()],
            clean_command: Some("go clean -cache -modcache".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\go-build".to_string()]),
            dynamic_cache_cmd: Some("go env GOCACHE".to_string()),
        },
        DevToolRule {
            name: "bun".to_string(),
            cmd_to_run: Some("bun".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("bun pm clean".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\bun\\install\\cache".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "deno".to_string(),
            cmd_to_run: Some("deno".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("deno clean".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\deno".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "gradle".to_string(),
            cmd_to_run: Some("gradle".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("gradle clean".to_string()),
            cache_path_templates: Some(vec!["{USERPROFILE}\\.gradle\\caches".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "maven".to_string(),
            cmd_to_run: Some("mvn".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("mvn clean".to_string()),
            cache_path_templates: Some(vec!["{USERPROFILE}\\.m2\\repository".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "dotnet".to_string(),
            cmd_to_run: Some("dotnet".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("dotnet nuget locals all --clear".to_string()),
            cache_path_templates: Some(vec!["{USERPROFILE}\\.nuget\\packages".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "docker".to_string(),
            cmd_to_run: Some("docker".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("docker system prune -f".to_string()),
            cache_path_templates: None,
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "conda".to_string(),
            cmd_to_run: Some("conda".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("conda clean -a -y".to_string()),
            cache_path_templates: Some(vec![
                "{USERPROFILE}\\.conda".to_string(),
                "{USERPROFILE}\\anaconda3\\pkgs".to_string(),
                "{USERPROFILE}\\miniconda3\\pkgs".to_string(),
            ]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "gem".to_string(),
            cmd_to_run: Some("gem".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("gem cleanup".to_string()),
            cache_path_templates: Some(vec!["{USERPROFILE}\\.gem".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "flutter".to_string(),
            cmd_to_run: Some("flutter".to_string()),
            args: vec!["--version".to_string()],
            clean_command: Some("flutter pub cache clean --force".to_string()),
            cache_path_templates: Some(vec!["{LOCALAPPDATA}\\Pub\\Cache".to_string()]),
            dynamic_cache_cmd: None,
        },
        DevToolRule {
            name: "vscode".to_string(),
            cmd_to_run: Some("code".to_string()),
            args: vec!["--version".to_string()],
            clean_command: None,
            cache_path_templates: Some(vec![
                "{APPDATA}\\Code\\Cache".to_string(),
                "{APPDATA}\\Code\\CachedData".to_string(),
                "{APPDATA}\\Code\\User\\workspaceStorage".to_string(),
            ]),
            dynamic_cache_cmd: None,
        },
    ]
}

pub fn load_dev_tools_rules() -> Vec<DevToolRule> {
    let path = get_devtools_rules_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let defaults = get_default_rules();
        if let Ok(json_str) = serde_json::to_string_pretty(&defaults) {
            let _ = fs::write(&path, json_str);
        }
        return defaults;
    }
    
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(rules) = serde_json::from_str::<Vec<DevToolRule>>(&content) {
            return rules;
        }
    }
    get_default_rules()
}

pub fn scan_dev_tools() -> Vec<DevToolInfo> {
    let rules = load_dev_tools_rules();
    let mut tools = Vec::new();

    for rule in rules {
        let name = rule.name.clone();
        let cmd_to_run = rule.cmd_to_run.as_deref().unwrap_or(&name);
        
        let mut info = DevToolInfo {
            name: name.clone(),
            detected: false,
            version: None,
            path: None,
            cache_path: None,
            cache_size: None,
            clean_command: rule.clean_command.clone(),
        };

        // Run through `cmd /C`: npm, pnpm, yarn, gradle and mvn ship as
        // .cmd/.bat shims on Windows, which CreateProcess (Command::new)
        // cannot launch directly, so detection always failed for them.
        let secure_path = get_secure_system_path();
        let version_cmd = format!("{} {}", cmd_to_run, rule.args.join(" "));
        if let Ok(output) = Command::new("cmd")
            .creation_flags(CREATE_NO_WINDOW)
            .env("PATH", &secure_path)
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
                    .env("PATH", &secure_path)
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

        if !info.detected && name == "vscode" {
            if let Some(code_path) = find_vscode_fallback_path() {
                info.detected = true;
                info.path = Some(code_path.to_string_lossy().to_string());
                
                // Try running the absolute path of code.cmd
                let version_cmd = format!("\"{}\" --version", code_path.to_string_lossy());
                if let Ok(output) = Command::new("cmd")
                    .creation_flags(CREATE_NO_WINDOW)
                    .env("PATH", &secure_path)
                    .args(&["/C", &version_cmd])
                    .output()
                {
                    if output.status.success() {
                        let version_str = String::from_utf8_lossy(&output.stdout);
                        info.version = Some(version_str.trim().to_string());
                    }
                }
            }
        }

        // If detected, find cache path and calculate its size
        if info.detected {
            let mut resolved_paths = Vec::new();
            if let Some(ref dyn_cmd) = rule.dynamic_cache_cmd {
                if let Some(dyn_path) = get_single_dynamic_cache_path(&name, dyn_cmd) {
                    resolved_paths.push(dyn_path);
                }
            }
            if resolved_paths.is_empty() {
                if let Some(ref templates) = rule.cache_path_templates {
                    for temp in templates {
                        if let Some(p) = resolve_template_path(temp) {
                            if p.exists() {
                                resolved_paths.push(p);
                            }
                        }
                    }
                }
            }

            if !resolved_paths.is_empty() {
                info.cache_path = Some(resolved_paths[0].to_string_lossy().to_string());
                
                let mut total_size = 0;
                for p in &resolved_paths {
                    total_size += calculate_dir_size(p);
                }
                info.cache_size = Some(total_size);
            }
        }

        tools.push(info);
    }

    tools
}

fn find_vscode_fallback_path() -> Option<PathBuf> {
    // 1. Standard paths
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let programfiles = env::var_os("ProgramFiles").map(PathBuf::from);
    let programfiles_x86 = env::var_os("ProgramFiles(x86)").map(PathBuf::from);

    let mut paths_to_check = Vec::new();

    if let Some(ref p) = localappdata {
        paths_to_check.push(p.join("Programs").join("Microsoft VS Code").join("bin").join("code.cmd"));
    }
    if let Some(ref p) = programfiles {
        paths_to_check.push(p.join("Microsoft VS Code").join("bin").join("code.cmd"));
    }
    if let Some(ref p) = programfiles_x86 {
        paths_to_check.push(p.join("Microsoft VS Code").join("bin").join("code.cmd"));
    }

    for p in &paths_to_check {
        if p.exists() {
            return Some(p.clone());
        }
    }

    // 2. Registry paths
    let registry_hives = vec![
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    for (hive, subkey_path) in registry_hives {
        let hk = RegKey::predef(hive);
        if let Ok(uninstall_key) = hk.open_subkey(subkey_path) {
            for name_result in uninstall_key.enum_keys().filter_map(|e| e.ok()) {
                if let Ok(sub_key) = uninstall_key.open_subkey(&name_result) {
                    let display_name: String = sub_key.get_value("DisplayName").unwrap_or_default();
                    if display_name.to_lowercase().contains("visual studio code") {
                        let install_location: String = sub_key.get_value("InstallLocation").unwrap_or_default();
                        if !install_location.is_empty() {
                            let path = PathBuf::from(install_location).join("bin").join("code.cmd");
                            if path.exists() {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }
    }

    None
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
    pub manager: String, // "npm" | "yarn" | "pnpm" | "bun" | "cargo" | "pip" | "dotnet" | "composer" | "deno" | "go"
}

pub fn scan_global_cli_packages() -> Vec<GlobalCliPackage> {
    let mut packages = Vec::new();

    // NPM
    if let Some(appdata) = env::var_os("APPDATA").map(PathBuf::from) {
        let npm_modules_path = appdata.join("npm").join("node_modules");
        scan_dir_for_packages(&npm_modules_path, "npm", &mut packages);
    }

    // Yarn
    if let Some(appdata) = env::var_os("APPDATA").map(PathBuf::from) {
        let yarn_path = appdata.join("Yarn").join("global").join("node_modules");
        scan_dir_for_packages(&yarn_path, "yarn", &mut packages);
    }
    if let Some(localappdata) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let yarn_path = localappdata.join("Yarn").join("Data").join("global").join("node_modules");
        scan_dir_for_packages(&yarn_path, "yarn", &mut packages);
    }

    // PNPM
    if let Some(localappdata) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let pnpm_path = localappdata.join("pnpm").join("node_modules");
        scan_dir_for_packages(&pnpm_path, "pnpm", &mut packages);
    }
    if let Some(appdata) = env::var_os("APPDATA").map(PathBuf::from) {
        let pnpm_path = appdata.join("pnpm").join("node_modules");
        scan_dir_for_packages(&pnpm_path, "pnpm", &mut packages);
    }

    // Bun
    if let Some(userprofile) = env::var_os("USERPROFILE").map(PathBuf::from) {
        let bun_path = userprofile.join(".bun").join("install").join("global").join("node_modules");
        scan_dir_for_packages(&bun_path, "bun", &mut packages);
    }

    // Cargo
    scan_cargo_packages(&mut packages);

    // Pip
    scan_pip_packages(&mut packages);

    // Dotnet
    scan_dotnet_packages(&mut packages);

    // Composer
    scan_composer_packages(&mut packages);

    // Deno
    scan_deno_packages(&mut packages);

    // Go
    scan_go_packages(&mut packages);

    packages
}

fn scan_dir_for_packages(modules_path: &Path, manager: &str, packages: &mut Vec<GlobalCliPackage>) {
    if !modules_path.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(modules_path) {
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
                                let info = parse_package_info(&sub_path, &full_name, manager);
                                packages.push(info);
                            }
                        }
                    }
                } else {
                    let info = parse_package_info(&path, &name, manager);
                    packages.push(info);
                }
            }
        }
    }
}

fn parse_package_info(path: &Path, name: &str, manager: &str) -> GlobalCliPackage {
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
        manager: manager.to_string(),
    }
}

fn scan_cargo_packages(packages: &mut Vec<GlobalCliPackage>) {
    let userprofile = match env::var_os("USERPROFILE").map(PathBuf::from) {
        Some(p) => p,
        None => return,
    };

    let cargo_dir = userprofile.join(".cargo");
    let crates_json = cargo_dir.join(".crates2.json");
    let crates_toml = cargo_dir.join(".crates.toml");

    if crates_json.exists() {
        if let Ok(content) = fs::read_to_string(&crates_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(installs) = json.get("installs").and_then(|i| i.as_object()) {
                    for key in installs.keys() {
                        if let Some((name, version)) = parse_cargo_key(key) {
                            let pkg_path = cargo_dir.join("bin").join(&name);
                            packages.push(GlobalCliPackage {
                                name: name.clone(),
                                version: version.clone(),
                                size_bytes: if pkg_path.exists() { pkg_path.metadata().map(|m| m.len()).unwrap_or(0) } else { 0 },
                                path: pkg_path.to_string_lossy().to_string(),
                                description: Some("Rust global binary package".to_string()),
                                manager: "cargo".to_string(),
                            });
                        }
                    }
                }
            }
        }
    } else if crates_toml.exists() {
        if let Ok(content) = fs::read_to_string(&crates_toml) {
            for line in content.lines() {
                if line.contains('=') && line.contains(' ') {
                    let parts: Vec<&str> = line.split('=').collect();
                    let key = parts[0].trim().trim_matches('"');
                    if let Some((name, version)) = parse_cargo_key(key) {
                        let pkg_path = cargo_dir.join("bin").join(&name);
                        packages.push(GlobalCliPackage {
                            name: name.clone(),
                            version: version.clone(),
                            size_bytes: if pkg_path.exists() { pkg_path.metadata().map(|m| m.len()).unwrap_or(0) } else { 0 },
                            path: pkg_path.to_string_lossy().to_string(),
                            description: Some("Rust global binary package".to_string()),
                            manager: "cargo".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn parse_cargo_key(key: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = key.split_whitespace().collect();
    if parts.len() >= 2 {
        let name = parts[0].to_string();
        let version = parts[1].to_string();
        Some((name, version))
    } else {
        None
    }
}

fn scan_pip_packages(packages: &mut Vec<GlobalCliPackage>) {
    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);

    let mut search_dirs = Vec::new();
    if let Some(ref p) = appdata {
        search_dirs.push(p.join("Python"));
    }
    if let Some(ref p) = localappdata {
        search_dirs.push(p.join("Python"));
    }

    for python_dir in search_dirs {
        if !python_dir.exists() {
            continue;
        }

        if let Ok(versions) = fs::read_dir(&python_dir) {
            for ver_entry in versions.filter_map(|e| e.ok()) {
                let site_packages = ver_entry.path().join("site-packages");
                if site_packages.exists() {
                    scan_pip_site_packages(&site_packages, packages);
                }
            }
        }
    }
}

fn scan_pip_site_packages(site_packages_path: &Path, packages: &mut Vec<GlobalCliPackage>) {
    if let Ok(entries) = fs::read_dir(site_packages_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let folder_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                if folder_name.ends_with(".dist-info") {
                    let name_ver = &folder_name[..folder_name.len() - 10];
                    let parts: Vec<&str> = name_ver.split('-').collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let version = parts[1].to_string();

                        let mut description = None;
                        let metadata_path = path.join("METADATA");
                        if metadata_path.exists() {
                            if let Ok(content) = fs::read_to_string(&metadata_path) {
                                for line in content.lines() {
                                    if line.starts_with("Summary: ") {
                                        description = Some(line["Summary: ".len()..].to_string());
                                        break;
                                    }
                                }
                            }
                        }

                        let mut size_bytes = 0;
                        let record_path = path.join("RECORD");
                        if record_path.exists() {
                            if let Ok(content) = fs::read_to_string(&record_path) {
                                for line in content.lines() {
                                    let parts: Vec<&str> = line.split(',').collect();
                                    if parts.len() >= 3 {
                                        if let Ok(sz) = parts[parts.len() - 1].parse::<u64>() {
                                            size_bytes += sz;
                                        }
                                    }
                                }
                            }
                        }
                        if size_bytes == 0 {
                            size_bytes = calculate_dir_size(&path);
                        }

                        packages.push(GlobalCliPackage {
                            name,
                            version,
                            size_bytes,
                            path: path.to_string_lossy().to_string(),
                            description,
                            manager: "pip".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn scan_dotnet_packages(packages: &mut Vec<GlobalCliPackage>) {
    let userprofile = match env::var_os("USERPROFILE").map(PathBuf::from) {
        Some(p) => p,
        None => return,
    };

    let store_dir = userprofile.join(".dotnet").join("tools").join(".store");
    if !store_dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(&store_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                if let Ok(versions) = fs::read_dir(&path) {
                    for ver_entry in versions.filter_map(|e| e.ok()) {
                        let ver_path = ver_entry.path();
                        if ver_path.is_dir() {
                            let version = match ver_path.file_name().and_then(|n| n.to_str()) {
                                Some(v) => v.to_string(),
                                None => continue,
                            };

                            let size = calculate_dir_size(&ver_path);
                            packages.push(GlobalCliPackage {
                                name: name.clone(),
                                version,
                                size_bytes: size,
                                path: ver_path.to_string_lossy().to_string(),
                                description: Some(".NET Global Tool".to_string()),
                                manager: "dotnet".to_string(),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn scan_composer_packages(packages: &mut Vec<GlobalCliPackage>) {
    let appdata = match env::var_os("APPDATA").map(PathBuf::from) {
        Some(p) => p,
        None => return,
    };

    let installed_json = appdata.join("Composer").join("vendor").join("composer").join("installed.json");
    if !installed_json.exists() {
        return;
    }

    if let Ok(content) = fs::read_to_string(&installed_json) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let pkg_list = if let Some(arr) = json.as_array() {
                Some(arr)
            } else {
                json.get("packages").and_then(|p| p.as_array())
            };

            if let Some(arr) = pkg_list {
                for item in arr {
                    if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                        let version = item.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        let description = item.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());

                        let path = appdata.join("Composer").join("vendor").join(name);
                        let size = if path.exists() { calculate_dir_size(&path) } else { 0 };

                        packages.push(GlobalCliPackage {
                            name: name.to_string(),
                            version,
                            size_bytes: size,
                            path: path.to_string_lossy().to_string(),
                            description,
                            manager: "composer".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn scan_deno_packages(packages: &mut Vec<GlobalCliPackage>) {
    let userprofile = match env::var_os("USERPROFILE").map(PathBuf::from) {
        Some(p) => p,
        None => return,
    };

    let bin_dir = userprofile.join(".deno").join("bin");
    if !bin_dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(&bin_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                let name = if file_name.ends_with(".cmd") {
                    file_name[..file_name.len() - 4].to_string()
                } else if file_name.ends_with(".ps1") {
                    continue;
                } else {
                    file_name.to_string()
                };

                if packages.iter().any(|p| p.name == name && p.manager == "deno") {
                    continue;
                }

                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                packages.push(GlobalCliPackage {
                    name,
                    version: "local".to_string(),
                    size_bytes: size,
                    path: path.to_string_lossy().to_string(),
                    description: Some("Deno global script binary".to_string()),
                    manager: "deno".to_string(),
                });
            }
        }
    }
}

fn scan_go_packages(packages: &mut Vec<GlobalCliPackage>) {
    let userprofile = match env::var_os("USERPROFILE").map(PathBuf::from) {
        Some(p) => p,
        None => return,
    };

    let bin_dir = userprofile.join("go").join("bin");
    if !bin_dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(&bin_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                let name = if file_name.ends_with(".exe") {
                    file_name[..file_name.len() - 4].to_string()
                } else {
                    file_name.to_string()
                };

                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                packages.push(GlobalCliPackage {
                    name,
                    version: "compiled".to_string(),
                    size_bytes: size,
                    path: path.to_string_lossy().to_string(),
                    description: Some("Go global compiled binary".to_string()),
                    manager: "go".to_string(),
                });
            }
        }
    }
}

fn safe_remove_dir_all(path: &Path) -> std::io::Result<()> {
    if let Err(e) = crate::winutil::is_safe_to_delete(&path.to_string_lossy()) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }
    fs::remove_dir_all(path)
}

fn safe_remove_file(path: &Path) -> std::io::Result<()> {
    if let Err(e) = crate::winutil::is_safe_to_delete(&path.to_string_lossy()) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }
    fs::remove_file(path)
}

pub fn uninstall_global_cli_package(name: &str, manager: &str) -> Result<(), String> {
    // SECURITY: the name is interpolated into a `cmd /C` string. Without
    // validation, a crafted name like "pkg & calc.exe" would be executed as
    // an arbitrary command. Enforce the package name charset (allowing uppercase).
    let is_valid = !name.is_empty()
        && name.len() <= 214
        && !name.starts_with('.')
        && !name.contains("..")
        && name.matches('/').count() <= 1
        && name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit()
                || matches!(c, '-' | '_' | '.' | '@' | '/' | '~')
        });
    if !is_valid {
        return Err(format!("Invalid package name: {}", name));
    }

    let cmd_arg = match manager {
        "npm" => format!("npm uninstall -g {}", name),
        "yarn" => format!("yarn global remove {}", name),
        "pnpm" => format!("pnpm uninstall -g {}", name),
        "bun" => format!("bun remove -g {}", name),
        "cargo" => format!("cargo uninstall {}", name),
        "pip" => format!("pip uninstall -y {}", name),
        "dotnet" => format!("dotnet tool uninstall -g {}", name),
        "composer" => format!("composer global remove {}", name),
        "deno" | "go" => "".to_string(),
        _ => return Err(format!("Unsupported package manager: {}", manager)),
    };

    if !cmd_arg.is_empty() {
        let secure_path = get_secure_system_path();
        let output = Command::new("cmd")
            .creation_flags(CREATE_NO_WINDOW)
            .env("PATH", &secure_path)
            .args(&["/C", &cmd_arg])
            .output()
            .map_err(|e| format!("Failed to execute uninstall command: {}", e))?;

        if !output.status.success() {
            // Proceed to manual cleanup as fallback
        }
    }

    let userprofile = env::var_os("USERPROFILE").map(PathBuf::from);
    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);

    match manager {
        "npm" => {
            if let Some(ref p) = appdata {
                let path = p.join("npm").join("node_modules").join(name);
                if path.exists() {
                    let _ = safe_remove_dir_all(&path);
                }
            }
        }
        "yarn" => {
            let paths = vec![
                appdata.as_ref().map(|p| p.join("Yarn").join("global").join("node_modules").join(name)),
                localappdata.as_ref().map(|p| p.join("Yarn").join("Data").join("global").join("node_modules").join(name)),
            ];
            for p_opt in paths.into_iter().flatten() {
                if p_opt.exists() {
                    let _ = safe_remove_dir_all(&p_opt);
                }
            }
        }
        "pnpm" => {
            let paths = vec![
                localappdata.as_ref().map(|p| p.join("pnpm").join("node_modules").join(name)),
                appdata.as_ref().map(|p| p.join("pnpm").join("node_modules").join(name)),
            ];
            for p_opt in paths.into_iter().flatten() {
                if p_opt.exists() {
                    let _ = safe_remove_dir_all(&p_opt);
                }
            }
        }
        "bun" => {
            if let Some(ref p) = userprofile {
                let path = p.join(".bun").join("install").join("global").join("node_modules").join(name);
                if path.exists() {
                    let _ = safe_remove_dir_all(&path);
                }
            }
        }
        "deno" => {
            if let Some(ref p) = userprofile {
                let bin_dir = p.join(".deno").join("bin");
                let _ = safe_remove_file(&bin_dir.join(name));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.cmd", name)));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.ps1", name)));
            }
        }
        "go" => {
            if let Some(ref p) = userprofile {
                let bin_dir = p.join("go").join("bin");
                let _ = safe_remove_file(&bin_dir.join(name));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.exe", name)));
            }
        }
        "cargo" => {
            if let Some(ref p) = userprofile {
                let bin_dir = p.join(".cargo").join("bin");
                let _ = safe_remove_file(&bin_dir.join(name));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.exe", name)));
            }
        }
        "pip" => {
            let python_dirs = vec![
                appdata.map(|p| p.join("Python")),
                localappdata.map(|p| p.join("Python")),
            ];
            for p_opt in python_dirs.into_iter().flatten() {
                if p_opt.exists() {
                    if let Ok(versions) = fs::read_dir(&p_opt) {
                        for ver_entry in versions.filter_map(|e| e.ok()) {
                            let site_packages = ver_entry.path().join("site-packages");
                            if site_packages.exists() {
                                if let Ok(entries) = fs::read_dir(&site_packages) {
                                    for entry in entries.filter_map(|e| e.ok()) {
                                        let path = entry.path();
                                        if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                                            let clean_folder = folder_name.split('-').next().unwrap_or(folder_name);
                                            if clean_folder.eq_ignore_ascii_case(name) {
                                                let _ = safe_remove_dir_all(&path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "dotnet" => {
            if let Some(ref p) = userprofile {
                let store_dir = p.join(".dotnet").join("tools").join(".store");
                let _ = safe_remove_dir_all(&store_dir.join(name));
                let bin_dir = p.join(".dotnet").join("tools");
                let _ = safe_remove_file(&bin_dir.join(name));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.exe", name)));
            }
        }
        "composer" => {
            if let Some(ref p) = appdata {
                let comp_dir = p.join("Composer").join("vendor");
                let _ = safe_remove_dir_all(&comp_dir.join(name));
                let bin_dir = comp_dir.join("bin");
                let _ = safe_remove_file(&bin_dir.join(name));
                let _ = safe_remove_file(&bin_dir.join(format!("{}.bat", name)));
            }
        }
        _ => {}
    }

    Ok(())
}

pub fn get_cli_package_bin_names(package_path: &str) -> Vec<String> {
    let mut bin_names = Vec::new();
    let path = Path::new(package_path);

    // If it is a Python dist-info directory, parse RECORD file
    if package_path.contains(".dist-info") {
        let record_path = path.join("RECORD");
        if record_path.exists() {
            if let Ok(content) = fs::read_to_string(&record_path) {
                for line in content.lines() {
                    if line.contains("Scripts/") || line.contains("Scripts\\") || line.contains("../../../bin") {
                        let parts: Vec<&str> = line.split(',').collect();
                        if !parts.is_empty() {
                            let file_path = parts[0];
                            if let Some(file_name) = Path::new(file_path).file_name().and_then(|n| n.to_str()) {
                                let clean_name = if file_name.ends_with(".exe") {
                                    file_name[..file_name.len() - 4].to_string()
                                } else if file_name.ends_with(".cmd") {
                                    file_name[..file_name.len() - 4].to_string()
                                } else if file_name.ends_with(".ps1") {
                                    file_name[..file_name.len() - 4].to_string()
                                } else {
                                    file_name.to_string()
                                };
                                if !clean_name.is_empty() {
                                    bin_names.push(clean_name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let pkg_json_path = path.join("package.json");
    if pkg_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&pkg_json_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(bin_val) = json.get("bin") {
                    if let Some(_s) = bin_val.as_str() {
                        if let Some(name_val) = json.get("name").and_then(|n| n.as_str()) {
                            let clean_name = if name_val.contains('/') {
                                name_val.split('/').last().unwrap_or(name_val).to_string()
                            } else {
                                name_val.to_string()
                            };
                            bin_names.push(clean_name);
                        }
                    } else if let Some(obj) = bin_val.as_object() {
                        for key in obj.keys() {
                            bin_names.push(key.clone());
                        }
                    }
                }
            }
        }
    }

    if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
        let clean_folder = if folder_name.contains(".dist-info") {
            let stripped = &folder_name[..folder_name.len() - 10];
            let name_part = stripped.split('-').next().unwrap_or(stripped);
            name_part.to_string()
        } else if folder_name.contains('/') {
            folder_name.split('/').last().unwrap_or(folder_name).to_string()
        } else {
            folder_name.to_string()
        };
        if !bin_names.contains(&clean_folder) {
            bin_names.push(clean_folder);
        }
    }

    bin_names.sort();
    bin_names.dedup();
    bin_names
}

pub fn get_cli_package_remnants(
    name: &str,
    _manager: &str,
    package_path: &str,
    bin_names: Vec<String>,
) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();

    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let userprofile = env::var_os("USERPROFILE").map(PathBuf::from);

    // 1. Check if the package folder still exists
    let pkg_path = Path::new(package_path);
    if pkg_path.exists() {
        let size = calculate_dir_size(pkg_path);
        remnants.push(RemnantItem {
            path: pkg_path.to_string_lossy().to_string(),
            item_type: "Directory".to_string(),
            size,
            confidence: "VeryHigh".to_string(),
            score: 100,
        });
    }

    // 2. Scan bin directories for shims
    let mut bin_dirs = Vec::new();

    if let Some(ref p) = appdata {
        bin_dirs.push(p.join("npm"));
    }
    if let Some(ref p) = localappdata {
        bin_dirs.push(p.join("yarn").join("bin"));
        bin_dirs.push(p.join("Yarn").join("bin"));
    }
    if let Some(ref p) = appdata {
        bin_dirs.push(p.join("local").join("yarn").join("bin"));
        bin_dirs.push(p.join("Yarn").join("global").join("node_modules").join(".bin"));
        bin_dirs.push(p.join("Yarn").join("config").join("global").join("node_modules").join(".bin"));
    }
    if let Some(ref p) = localappdata {
        bin_dirs.push(p.join("pnpm"));
    }
    if let Some(ref p) = appdata {
        bin_dirs.push(p.join("pnpm"));
    }
    if let Some(ref p) = userprofile {
        bin_dirs.push(p.join(".bun").join("bin"));
        bin_dirs.push(p.join(".cargo").join("bin"));
        bin_dirs.push(p.join("go").join("bin"));
        bin_dirs.push(p.join(".deno").join("bin"));
        bin_dirs.push(p.join(".dotnet").join("tools"));
    }
    if let Some(ref p) = appdata {
        bin_dirs.push(p.join("Composer").join("vendor").join("bin"));
    }

    // Python Pip scripts folders
    let python_parents = vec![appdata.clone(), localappdata.clone()];
    for parent_opt in python_parents.into_iter().flatten() {
        let python_dir = parent_opt.join("Python");
        if python_dir.exists() {
            if let Ok(versions) = fs::read_dir(&python_dir) {
                for ver_entry in versions.filter_map(|e| e.ok()) {
                    let scripts_dir = ver_entry.path().join("Scripts");
                    if scripts_dir.exists() {
                        bin_dirs.push(scripts_dir);
                    }
                }
            }
        }
    }

    bin_dirs.sort();
    bin_dirs.dedup();

    for bin_dir in bin_dirs {
        if !bin_dir.exists() {
            continue;
        }

        for bin_name in &bin_names {
            let extensions = vec!["", ".cmd", ".ps1", ".bat", ".exe"];
            for ext in extensions {
                let file_name = if ext.is_empty() {
                    bin_name.clone()
                } else {
                    format!("{}{}", bin_name, ext)
                };
                let shim_path = bin_dir.join(&file_name);
                if shim_path.exists() && shim_path.is_file() {
                    if let Ok(meta) = shim_path.metadata() {
                        remnants.push(RemnantItem {
                            path: shim_path.to_string_lossy().to_string(),
                            item_type: "File".to_string(),
                            size: meta.len(),
                            confidence: "High".to_string(),
                            score: 90,
                        });
                    }
                }
            }
        }
    }

    // 3. Scan configuration/cache directories
    let mut search_names = Vec::new();
    let clean_name = if name.contains('/') {
        name.split('/').last().unwrap_or(name).to_string()
    } else {
        name.to_string()
    };

    search_names.push(clean_name.clone());
    if name.contains('/') {
        search_names.push(name.replace('/', "-"));
        search_names.push(name.replace('@', ""));
    }

    for b in &bin_names {
        if !search_names.contains(b) {
            search_names.push(b.clone());
        }
    }

    search_names.sort();
    search_names.dedup();

    let mut parent_dirs = Vec::new();
    if let Some(ref p) = userprofile {
        parent_dirs.push(p.clone());
        parent_dirs.push(p.join(".config"));
        parent_dirs.push(p.join(".cache"));
    }
    if let Some(ref p) = appdata {
        parent_dirs.push(p.clone());
    }
    if let Some(ref p) = localappdata {
        parent_dirs.push(p.clone());
        parent_dirs.push(p.join("Temp"));
    }

    parent_dirs.sort();
    parent_dirs.dedup();

    for parent in parent_dirs {
        if !parent.exists() {
            continue;
        }

        for search_name in &search_names {
            if search_name.len() < 3 || search_name == "cli" || search_name == "bin" || search_name == "npm" || search_name == "yarn" || search_name == "pnpm" || search_name == "bun" {
                continue;
            }

            let folder_variations = vec![
                format!(".{}", search_name),
                search_name.clone(),
            ];

            for folder_var in folder_variations {
                let target_path = parent.join(&folder_var);
                if target_path.exists() && target_path.to_string_lossy().to_string() != package_path {
                    if target_path.is_dir() {
                        let size = calculate_dir_size(&target_path);
                        let path_str = target_path.to_string_lossy().to_string();
                        if !remnants.iter().any(|r| r.path == path_str) {
                            remnants.push(RemnantItem {
                                path: path_str,
                                item_type: "Directory".to_string(),
                                size,
                                confidence: "Medium".to_string(),
                                score: 70,
                            });
                        }
                    } else if target_path.is_file() {
                        if let Ok(meta) = target_path.metadata() {
                            let path_str = target_path.to_string_lossy().to_string();
                            if !remnants.iter().any(|r| r.path == path_str) {
                                remnants.push(RemnantItem {
                                    path: path_str,
                                    item_type: "File".to_string(),
                                    size: meta.len(),
                                    confidence: "Medium".to_string(),
                                    score: 70,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 4. Scan Environment Variables for remnants
    let env_locations = vec![
        ("HKCU", "Environment"),
        ("HKLM", r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"),
    ];

    for (hive_name, subpath) in env_locations {
        let hive = match hive_name {
            "HKCU" => RegKey::predef(HKEY_CURRENT_USER),
            "HKLM" => RegKey::predef(HKEY_LOCAL_MACHINE),
            _ => continue,
        };

        if let Ok(key) = hive.open_subkey_with_flags(subpath, KEY_READ) {
            for (val_name, val_data) in key.enum_values().filter_map(|x| x.ok()) {
                let val_name_upper = val_name.to_uppercase();
                let clean_name_upper = clean_name.to_uppercase();
                
                let is_match = val_name_upper == clean_name_upper 
                    || val_name_upper.starts_with(&format!("{}_", clean_name_upper))
                    || val_name_upper.ends_with(&format!("_{}", clean_name_upper));
                
                if is_match && val_name_upper != "PATH" && val_name_upper != "TEMP" && val_name_upper != "TMP" {
                    let full_reg_path = format!(r"{}\{}", subpath, val_name);
                    if crate::winutil::is_safe_registry_key(hive_name, &full_reg_path).is_ok() {
                        remnants.push(RemnantItem {
                            path: format!(r"{}\{}\{}", hive_name, subpath, val_name),
                            item_type: "RegistryValue".to_string(),
                            size: val_data.bytes.len() as u64,
                            confidence: "High".to_string(),
                            score: 85,
                        });
                    }
                }
            }
        }
    }

    // 5. Scan npm cache _npx folder
    if let Some(ref p) = appdata {
        let npx_cache = p.join("npm-cache").join("_npx");
        if npx_cache.exists() {
            if let Ok(entries) = fs::read_dir(&npx_cache) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let pkg_json = path.join("package.json");
                        let mut matches_pkg = false;
                        if pkg_json.exists() {
                            if let Ok(content) = fs::read_to_string(&pkg_json) {
                                if content.contains(&format!("\"{}\"", name)) {
                                    matches_pkg = true;
                                }
                            }
                        }
                        if matches_pkg {
                            let path_str = path.to_string_lossy().to_string();
                            if crate::winutil::is_safe_to_delete(&path_str).is_ok() {
                                let size = calculate_dir_size(&path);
                                remnants.push(RemnantItem {
                                    path: path_str,
                                    item_type: "Directory".to_string(),
                                    size,
                                    confidence: "High".to_string(),
                                    score: 90,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Centralized Safety Gate Filter
    remnants.retain(|item| {
        match item.item_type.as_str() {
            "File" | "Directory" => crate::winutil::is_safe_to_delete(&item.path).is_ok(),
            "RegistryKey" | "RegistryValue" => {
                let parts: Vec<&str> = item.path.splitn(2, '\\').collect();
                if parts.len() >= 2 {
                    crate::winutil::is_safe_registry_key(parts[0], parts[1]).is_ok()
                } else {
                    false
                }
            }
            _ => false,
        }
    });

    remnants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_vscode_fallback_path() {
        let path = find_vscode_fallback_path();
        println!("Detected VSCode fallback path: {:?}", path);
    }

    #[test]
    fn test_scan_dev_tools_vscode() {
        let tools = scan_dev_tools();
        let vscode = tools.iter().find(|t| t.name == "vscode");
        assert!(vscode.is_some(), "VSCode info should be present in the results");
        let vscode_info = vscode.unwrap();
        println!("VSCode Info: {:?}", vscode_info);
    }

    #[test]
    fn test_get_dynamic_cache_path() {
        let pnpm_path = get_dynamic_cache_path("pnpm");
        println!("Dynamic pnpm path: {:?}", pnpm_path);
        let npm_path = get_dynamic_cache_path("npm");
        println!("Dynamic npm path: {:?}", npm_path);
        let pip_path = get_dynamic_cache_path("pip");
        println!("Dynamic pip path: {:?}", pip_path);
        let go_path = get_dynamic_cache_path("go");
        println!("Dynamic go path: {:?}", go_path);
    }

    #[test]
    fn test_uninstall_global_cli_package_validation() {
        let valid_names = vec![
            "express",
            "@types/node",
            "lodash-es",
            "my_pkg_123",
            "~some-pkg"
        ];
        for name in valid_names {
            let res = uninstall_global_cli_package(name, "nonexistent");
            assert_eq!(res.unwrap_err(), "Unsupported package manager: nonexistent");
        }

        let invalid_names = vec![
            "express & calc.exe",
            "express; calc.exe",
            "express | calc.exe",
            "../express",
            "express/nested/deep",
            ".hidden",
            "",
        ];
        for name in invalid_names {
            let res = uninstall_global_cli_package(name, "npm");
            assert!(res.is_err());
            assert!(res.unwrap_err().contains("Invalid package name"));
        }
    }
}

