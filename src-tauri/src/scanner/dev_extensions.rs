use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Clone)]
pub struct DevExtension {
    pub ide: String,
    pub name: String,
    pub version: Option<String>,
    pub path: String,
    pub size_bytes: u64,
}

fn tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

fn scan_vscode_root(root: &Path, ide: &str, result: &mut Vec<DevExtension>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let package_json = path.join("package.json");
        if !package_json.is_file() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&package_json) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };

        let name = json
            .get("displayName")
            .and_then(|value| value.as_str())
            .or_else(|| json.get("name").and_then(|value| value.as_str()))
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Unknown extension")
            })
            .to_string();
        let version = json
            .get("version")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        result.push(DevExtension {
            ide: ide.to_string(),
            name,
            version,
            path: path.to_string_lossy().to_string(),
            size_bytes: super::cli_dev::calculate_dir_size(&path),
        });
    }
}

fn scan_jetbrains(appdata: &Path, result: &mut Vec<DevExtension>) {
    let root = appdata.join("JetBrains");
    let Ok(products) = fs::read_dir(&root) else {
        return;
    };

    for product in products.filter_map(|entry| entry.ok()) {
        let product_path = product.path();
        if !product_path.is_dir() {
            continue;
        }
        let plugins = product_path.join("plugins");
        let Ok(entries) = fs::read_dir(&plugins) else {
            continue;
        };
        let product_name = product_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("JetBrains")
            .to_string();

        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let plugin_xml = path.join("META-INF").join("plugin.xml");
            let name = fs::read_to_string(&plugin_xml)
                .ok()
                .and_then(|xml| tag_value(&xml, "name"))
                .unwrap_or_else(|| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Unknown plugin")
                        .to_string()
                });
            let version = fs::read_to_string(&plugin_xml)
                .ok()
                .and_then(|xml| tag_value(&xml, "version"));

            result.push(DevExtension {
                ide: product_name.clone(),
                name,
                version,
                path: path.to_string_lossy().to_string(),
                size_bytes: super::cli_dev::calculate_dir_size(&path),
            });
        }
    }
}

pub fn scan_dev_extensions() -> Vec<DevExtension> {
    let mut result = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        scan_vscode_root(&home.join(".vscode").join("extensions"), "VS Code", &mut result);
        scan_vscode_root(
            &home.join(".vscode-insiders").join("extensions"),
            "VS Code Insiders",
            &mut result,
        );
        scan_vscode_root(&home.join(".cursor").join("extensions"), "Cursor", &mut result);
        scan_vscode_root(&home.join(".windsurf").join("extensions"), "Windsurf", &mut result);
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        scan_jetbrains(&appdata, &mut result);
    }

    result.sort_by_key(|extension| std::cmp::Reverse(extension.size_bytes));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_extension_metadata() {
        let root = std::env::temp_dir().join("purgekit_extensions_test");
        let extension = root.join("publisher.extension-1.0.0");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&extension).unwrap();
        fs::write(
            extension.join("package.json"),
            r#"{"name":"extension","displayName":"My Extension","publisher":"publisher","version":"1.0.0"}"#,
        )
        .unwrap();

        let mut result = Vec::new();
        scan_vscode_root(&root, "VS Code", &mut result);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "My Extension");
        assert_eq!(result[0].version.as_deref(), Some("1.0.0"));

        let _ = fs::remove_dir_all(&root);
    }
}
