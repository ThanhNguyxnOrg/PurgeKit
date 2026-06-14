use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectFolder {
    pub path: String,
    pub name: String,
    pub project_name: String,
    pub folder_type: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub last_modified: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectScanProgress {
    pub current_dir: String,
    pub folders_found: u32,
    pub total_size_bytes: u64,
}

pub fn calculate_dir_size_and_count(path: &Path) -> (u64, u64) {
    let mut size_bytes = 0;
    let mut file_count = 0;
    
    for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                size_bytes += metadata.len();
                file_count += 1;
            }
        }
    }
    
    (size_bytes, file_count)
}

pub fn get_last_modified_time(path: &Path) -> String {
    if let Ok(metadata) = path.metadata() {
        if let Ok(modified) = metadata.modified() {
            let datetime: chrono::DateTime<chrono::Local> = modified.into();
            return datetime.format("%Y-%m-%d %H:%M:%S").to_string();
        }
    }
    "N/A".to_string()
}

pub fn scan_project_folders(
    app: &AppHandle,
    roots: &[String],
    folder_types: &[String],
) -> Result<Vec<ProjectFolder>, String> {
    scan_project_folders_impl(roots, folder_types, |current_dir, folders_found, total_size_bytes| {
        let _ = app.emit("project-scan-progress", ProjectScanProgress {
            current_dir,
            folders_found,
            total_size_bytes,
        });
    })
}

pub fn scan_project_folders_impl<F>(
    roots: &[String],
    folder_types: &[String],
    mut on_progress: F,
) -> Result<Vec<ProjectFolder>, String>
where
    F: FnMut(String, u32, u64),
{
    let mut results = Vec::new();
    let mut queue = VecDeque::new();
    let mut folders_found_count = 0;
    let mut total_size_bytes_found = 0;

    let skip_dirs = vec![
        ".git", ".svn", ".hg", "AppData", "Local Settings", 
        "System Volume Information", "$RECYCLE.BIN", "Program Files", 
        "Program Files (x86)", "Windows"
    ];

    for root in roots {
        let expanded = crate::winutil::expand_env_strings(root);
        let target = crate::winutil::canonicalize_path_safety(&expanded);
        let target_str = target.to_string_lossy().to_string();

        if let Err(e) = crate::winutil::is_safe_to_delete(&target_str) {
            return Err(format!("Scanning blocked: '{}' is a protected directory and cannot be swept.", root));
        }

        let path = PathBuf::from(root);
        if path.exists() && path.is_dir() {
            queue.push_back((path, 0));
        }
    }

    let mut iteration = 0;

    while let Some((dir, depth)) = queue.pop_front() {
        if depth > 8 {
            continue;
        }

        iteration += 1;
        
        if iteration % 50 == 0 {
            on_progress(dir.to_string_lossy().to_string(), folders_found_count, total_size_bytes_found);
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };

            if skip_dirs.iter().any(|&skip| skip.eq_ignore_ascii_case(&file_name)) {
                continue;
            }

            let mut matched_type = None;
            for t in folder_types {
                if file_name.eq_ignore_ascii_case(t) {
                    matched_type = Some(t.clone());
                    break;
                }
            }

            if let Some(t_type) = matched_type {
                let (size_bytes, file_count) = calculate_dir_size_and_count(&path);
                let last_mod = get_last_modified_time(&path);

                let project_name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown Project".to_string());

                let folder = ProjectFolder {
                    path: path.to_string_lossy().to_string(),
                    name: file_name.clone(),
                    project_name,
                    folder_type: t_type,
                    size_bytes,
                    file_count,
                    last_modified: last_mod,
                };

                folders_found_count += 1;
                total_size_bytes_found += size_bytes;
                results.push(folder);

                on_progress(path.to_string_lossy().to_string(), folders_found_count, total_size_bytes_found);
                
                continue;
            }

            queue.push_back((path, depth + 1));
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_project_sweeper_scan() {
        let temp_dir = std::env::temp_dir().join("purgekit_test_sweeper");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Create mock project structures
        let project_a = temp_dir.join("project-a");
        let node_modules = project_a.join("node_modules");
        let node_modules_nested = node_modules.join("nested_folder");
        let git_dir = project_a.join(".git");

        let project_b = temp_dir.join("project-b");
        let target_dir = project_b.join("target");
        let venv_dir = project_b.join(".venv");

        fs::create_dir_all(&node_modules_nested).unwrap();
        fs::create_dir_all(&git_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&venv_dir).unwrap();

        // Create some files to verify size calculations
        let f1_path = node_modules.join("package.json");
        fs::write(&f1_path, "{}").unwrap();
        
        let f2_path = node_modules_nested.join("index.js");
        fs::write(&f2_path, "console.log('hello');").unwrap();

        let f3_path = git_dir.join("config");
        fs::write(&f3_path, "[core]").unwrap();

        let f4_path = target_dir.join("debug_file");
        fs::write(&f4_path, "build_artifact").unwrap();

        // Run the scan logic
        let roots = vec![temp_dir.to_string_lossy().to_string()];
        let folder_types = vec!["node_modules".to_string(), "target".to_string(), ".venv".to_string()];
        
        let scan_results = scan_project_folders_impl(&roots, &folder_types, |_, _, _| {}).unwrap();

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);

        // Verify findings
        assert_eq!(scan_results.len(), 3);
        
        let has_node_modules = scan_results.iter().any(|f| f.name == "node_modules" && f.project_name == "project-a");
        let has_target = scan_results.iter().any(|f| f.name == "target" && f.project_name == "project-b");
        let has_venv = scan_results.iter().any(|f| f.name == ".venv" && f.project_name == "project-b");

        assert!(has_node_modules);
        assert!(has_target);
        assert!(has_venv);

        // Verify .git is NOT in the results
        let has_git = scan_results.iter().any(|f| f.name == ".git");
        assert!(!has_git);

        // Verify node_modules size calculation (includes package.json + index.js sizes)
        let node_modules_item = scan_results.iter().find(|f| f.name == "node_modules").unwrap();
        assert_eq!(node_modules_item.file_count, 2); // package.json and index.js
        assert!(node_modules_item.size_bytes > 0);
    }
}
