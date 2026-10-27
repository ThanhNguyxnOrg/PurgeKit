use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::dev_rules;
use super::{
    scan_global_cli_packages, scan_toolchain_versions, scan_uwp_apps, InstalledApp,
};

#[derive(Debug, Serialize, Clone)]
pub struct InventoryApp {
    #[serde(flatten)]
    pub app: InstalledApp,
    pub source: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub is_dev: bool,
    pub managed_by: Option<String>,
    pub is_active: Option<bool>,
    pub is_portable: bool,
    pub is_uninstallable: bool,
    pub requires_admin: bool,
    pub modify_path: Option<String>,
    pub release_type: Option<String>,
    pub is_update: bool,
}

fn location_requires_admin(location: Option<&str>) -> bool {
    let Some(location) = location else {
        return false;
    };
    let location = location.to_lowercase();
    ["program files", "programdata", "c:\\windows"]
        .iter()
        .any(|prefix| location.starts_with(prefix) || location.contains(&format!("\\{}", prefix)))
}

fn wrap(
    app: InstalledApp,
    source: &str,
    managed_by: Option<String>,
    is_active: Option<bool>,
) -> InventoryApp {
    let requires_admin = source == "registry"
        && (app.hive.to_uppercase().starts_with("HKLM")
            || location_requires_admin(app.install_location.as_deref()));
    let is_uninstallable = match source {
        "registry" => app.uninstall_string.is_some(),
        "store" | "cli" | "toolchain" | "portable" => true,
        _ => false,
    };
    let is_update = source == "registry"
        && (app.release_type.as_deref().map_or(false, |release| {
            release.eq_ignore_ascii_case("update") || release.eq_ignore_ascii_case("security update")
        }) || app.display_name.to_lowercase().starts_with("update for ")
            || app.display_name.to_lowercase().starts_with("security update"));
    let mut classification = dev_rules::classify(
        &app.display_name,
        app.publisher.as_deref(),
        app.install_location.as_deref(),
        source,
        managed_by.as_deref(),
    );
    if is_update && !classification.categories.iter().any(|category| category == "Windows Update") {
        classification.categories.push("Windows Update".to_string());
    }

    InventoryApp {
        modify_path: app.modify_path.clone(),
        release_type: app.release_type.clone(),
        is_update,
        app,
        source: source.to_string(),
        categories: classification.categories,
        tags: classification.tags,
        is_dev: classification.is_dev,
        managed_by,
        is_active,
        is_portable: source == "portable",
        is_uninstallable,
        requires_admin,
    }
}

fn collect_claimed_path(app: &InstalledApp, claimed: &mut Vec<String>) {
    if let Some(location) = &app.install_location {
        if !location.trim().is_empty() {
            claimed.push(location.clone());
        }
    }

    for command in [&app.uninstall_string, &app.quiet_uninstall_string]
        .into_iter()
        .flatten()
    {
        let executable = crate::winutil::extract_executable_path(command);
        if let Some(parent) = std::path::Path::new(&executable).parent() {
            let parent = parent.to_string_lossy().to_string();
            if !parent.is_empty() {
                claimed.push(parent);
            }
        }
    }

    if let Some(icon) = &app.display_icon {
        let icon = icon.trim().trim_matches('"');
        let icon = icon
            .rsplit_once(',')
            .filter(|(_, suffix)| suffix.chars().all(|c| c.is_ascii_digit()))
            .map(|(path, _)| path)
            .unwrap_or(icon);
        if let Some(parent) = std::path::Path::new(icon).parent() {
            let parent = parent.to_string_lossy().to_string();
            if !parent.is_empty() {
                claimed.push(parent);
            }
        }
    }
}

fn find_on_path(candidates: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for candidate in candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn icon_base64_for(path: &Path) -> Option<String> {
    windows_icons::get_icon_base64_by_path(path).ok()
}

fn runtime_icon_base64(manager: &str) -> Option<String> {
    let candidates: &[&str] = match manager {
        "npm" | "yarn" | "pnpm" | "bun" => &["node.exe"],
        "rustup" | "cargo" => &["cargo.exe", "rustup.exe"],
        "pip" => &["python.exe", "pip.exe"],
        "dotnet" => &["dotnet.exe"],
        "composer" => &["php.exe"],
        "deno" => &["deno.exe"],
        "go" => &["go.exe"],
        _ => &[],
    };
    if candidates.is_empty() {
        return None;
    }
    find_on_path(candidates).as_deref().and_then(icon_base64_for)
}

fn toolchain_icon_base64(manager: &str, path: &str) -> Option<String> {
    let base = Path::new(path);
    let candidates: Vec<PathBuf> = match manager {
        "rustup" => vec![
            base.join("bin").join("rustc.exe"),
            base.join("bin").join("cargo.exe"),
        ],
        "fnm" | "nvm" => vec![base.join("node.exe"), base.join("bin").join("node.exe")],
        _ => Vec::new(),
    };
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| icon_base64_for(candidate))
        .or_else(|| runtime_icon_base64(manager))
}

pub fn build_inventory() -> Vec<InventoryApp> {
    let mut items = Vec::new();
    let mut runtime_icons: HashMap<String, Option<String>> = HashMap::new();

    let registry_apps = super::registry::scan_registry();
    let store_apps = scan_uwp_apps();
    let mut claimed_paths = Vec::new();
    let mut claimed_names = std::collections::HashSet::new();
    for app in registry_apps.iter().chain(store_apps.iter()) {
        claimed_names.insert(app.display_name.to_lowercase());
        collect_claimed_path(app, &mut claimed_paths);
    }
    claimed_paths.sort();
    claimed_paths.dedup();

    for app in registry_apps {
        items.push(wrap(app, "registry", None, None));
    }
    for app in store_apps {
        items.push(wrap(app, "store", None, None));
    }
    for package in scan_global_cli_packages() {
        let cli_icon = runtime_icons
            .entry(package.manager.clone())
            .or_insert_with(|| runtime_icon_base64(&package.manager))
            .clone();
        let app = InstalledApp {
            id: format!("cli:{}:{}", package.manager, package.name),
            display_name: package.name.clone(),
            display_version: Some(package.version.clone()),
            publisher: Some(package.manager.clone()),
            uninstall_string: None,
            quiet_uninstall_string: None,
            modify_path: None,
            release_type: None,
            install_location: Some(package.path.clone()),
            install_date: None,
            display_icon: None,
            icon_base64: cli_icon,
            estimated_size: Some(package.size_bytes),
            registry_path: format!("cli:{}:{}", package.manager, package.name),
            hive: "CLI".to_string(),
            is_uwp: false,
            is_verified: None,
        };
        items.push(wrap(app, "cli", Some(package.manager), None));
    }
    for toolchain in scan_toolchain_versions() {
        let toolchain_icon = toolchain_icon_base64(&toolchain.manager, &toolchain.path);
        let app = InstalledApp {
            id: format!("toolchain:{}:{}", toolchain.manager, toolchain.version),
            display_name: toolchain.version.clone(),
            display_version: Some(toolchain.version.clone()),
            publisher: Some(toolchain.manager.clone()),
            uninstall_string: None,
            quiet_uninstall_string: None,
            modify_path: None,
            release_type: None,
            install_location: Some(toolchain.path.clone()),
            install_date: None,
            display_icon: None,
            icon_base64: toolchain_icon,
            estimated_size: Some(toolchain.size_bytes),
            registry_path: format!("toolchain:{}:{}", toolchain.manager, toolchain.version),
            hive: "TOOLCHAIN".to_string(),
            is_uwp: false,
            is_verified: None,
        };
        items.push(wrap(
            app,
            "toolchain",
            Some(toolchain.manager),
            Some(toolchain.is_active),
        ));
    }

    for portable in super::portable_apps::scan_portable_apps(&claimed_paths) {
        // A registered app can lack InstallLocation while still being listed
        // in the registry. Avoid showing the same installation twice.
        if claimed_names.contains(&portable.name.to_lowercase()) {
            continue;
        }
        let portable_icon = icon_base64_for(Path::new(&portable.executable));
        let portable_icon_source = portable.executable.clone();
        let app = InstalledApp {
            id: format!("portable:{}", portable.path),
            display_name: portable.name,
            display_version: None,
            publisher: None,
            uninstall_string: None,
            quiet_uninstall_string: None,
            modify_path: None,
            release_type: None,
            install_location: Some(portable.path),
            install_date: None,
            display_icon: Some(portable_icon_source),
            icon_base64: portable_icon,
            estimated_size: Some(portable.size_bytes),
            registry_path: portable.executable,
            hive: "PORTABLE".to_string(),
            is_uwp: false,
            is_verified: None,
        };
        items.push(wrap(app, "portable", None, None));
    }

    items.sort_by(|left, right| {
        left.app
            .display_name
            .to_lowercase()
            .cmp(&right.app.display_name.to_lowercase())
            .then_with(|| left.source.cmp(&right.source))
    });
    items.dedup_by(|left, right| left.app.id.eq_ignore_ascii_case(&right.app.id));
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, publisher: &str) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            display_name: name.to_string(),
            display_version: Some("1.0".to_string()),
            publisher: Some(publisher.to_string()),
            uninstall_string: Some("uninstall.exe".to_string()),
            quiet_uninstall_string: None,
            modify_path: None,
            release_type: None,
            install_location: None,
            install_date: None,
            display_icon: None,
            icon_base64: None,
            estimated_size: None,
            registry_path: String::new(),
            hive: "HKCU".to_string(),
            is_uwp: false,
            is_verified: None,
        }
    }

    #[test]
    fn wraps_and_classifies_registry_app() {
        let item = wrap(
            app("code", "Visual Studio Code", "Microsoft"),
            "registry",
            None,
            None,
        );
        assert!(item.is_dev);
        assert!(item.categories.contains(&"IDE".to_string()));
        assert!(item.is_uninstallable);
        assert!(!item.is_portable);
    }

    #[test]
    fn requires_admin_for_hklm() {
        let mut input = app("x", "Some App", "Vendor");
        input.hive = "HKLM".to_string();
        let item = wrap(input, "registry", None, None);
        assert!(item.requires_admin);
    }

    #[test]
    fn flags_windows_updates_and_program_files_for_elevation() {
        let mut update = app("kb", "Security Update for Windows (KB123)", "Microsoft");
        update.install_location = Some(r"C:\Program Files\Contoso".to_string());
        update.hive = "HKCU".to_string();

        let item = wrap(update, "registry", None, None);
        assert!(item.is_update);
        assert!(item.requires_admin);
        assert!(item.categories.contains(&"Windows Update".to_string()));
        assert!(item.is_uninstallable);
    }

    #[test]
    fn marks_store_cli_and_portable_entries_uninstallable() {
        let store = wrap(app("store", "Store App", "Vendor"), "store", None, None);
        assert!(store.is_uninstallable);
        assert!(!store.requires_admin);

        let cli = wrap(
            app("cli", "gopls", "go"),
            "cli",
            Some("go".to_string()),
            None,
        );
        assert!(cli.is_uninstallable);
        assert!(!cli.requires_admin);

        let portable = wrap(app("p", "Portable", "Vendor"), "portable", None, None);
        assert!(portable.is_uninstallable);
        assert!(portable.is_portable);
    }

    #[test]
    #[ignore = "manual smoke test scans the host machine"]
    fn inventory_smoke() {
        let items = build_inventory();
        assert!(!items.is_empty());
        println!("inventory items: {}", items.len());
        for source in ["registry", "store", "cli", "toolchain", "portable"] {
            let count = items.iter().filter(|item| item.source == source).count();
            let missing = items
                .iter()
                .filter(|item| item.source == source && item.app.icon_base64.is_none())
                .count();
            println!("  {}: {} ({} without icon)", source, count, missing);
        }
        for item in items
            .iter()
            .filter(|item| item.app.icon_base64.is_none())
            .take(40)
        {
            println!("    no icon: {} [{}]", item.app.display_name, item.source);
        }
        println!(
            "  dev: {}",
            items.iter().filter(|item| item.is_dev).count()
        );
        for item in items.iter().filter(|item| item.is_dev).take(12) {
            println!(
                "    {} [{}] {:?}",
                item.app.display_name, item.source, item.categories
            );
        }
        for item in items.iter().filter(|item| item.source == "portable") {
            println!(
                "    portable: {} @ {}",
                item.app.display_name,
                item.app.install_location.as_deref().unwrap_or("")
            );
        }
    }
}
