use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use winreg::enums::*;
use winreg::RegKey;
use rusqlite::Connection;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::{get_db_path, get_snapshots_dir};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotData {
    pub registry_keys: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotRecord {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub data_file_path: String,
    pub reg_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotDiff {
    pub new_registry_keys: Vec<String>,
    pub new_files: Vec<String>,
}

pub fn create_snapshot(name: &str) -> Result<SnapshotRecord, String> {
    let id = Uuid::new_v4().to_string();
    let created_at = format!("{:?}", SystemTime::now());

    // 1. Scan Registry Keys (depth-limited to keep it fast)
    let mut registry_keys = Vec::new();
    let _ = scan_registry_keys_recursive(HKEY_CURRENT_USER, "HKCU", "SOFTWARE", 0, 3, &mut registry_keys);
    let _ = scan_registry_keys_recursive(HKEY_LOCAL_MACHINE, "HKLM", "SOFTWARE", 0, 3, &mut registry_keys);

    // 2. Scan Filesystem (focus on AppData, ProgramData, ProgramFiles)
    let mut files = Vec::new();
    let dirs = vec![
        env::var_os("APPDATA").map(PathBuf::from),
        env::var_os("LOCALAPPDATA").map(PathBuf::from),
        env::var_os("ProgramData").map(PathBuf::from),
    ];
    for dir in dirs.into_iter().flatten() {
        scan_files_recursive(&dir, 3, &mut files);
    }

    let snap_data = SnapshotData {
        registry_keys: registry_keys.clone(),
        files: files.clone(),
    };

    // Save snap data to file
    let snap_dir = get_snapshots_dir();
    let file_name = format!("snap_{}.json", id);
    let data_file_path = snap_dir.join(file_name);
    
    let json_data = serde_json::to_string(&snap_data).map_err(|e| e.to_string())?;
    fs::write(&data_file_path, json_data).map_err(|e| e.to_string())?;

    // Save to DB
    let db_path = get_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO snapshots (id, name, created_at, data_file_path) VALUES (?1, ?2, ?3, ?4)",
        [&id, name, &created_at, &data_file_path.to_string_lossy().to_string()],
    ).map_err(|e| e.to_string())?;

    Ok(SnapshotRecord {
        id,
        name: name.to_string(),
        created_at,
        data_file_path: data_file_path.to_string_lossy().to_string(),
        reg_count: registry_keys.len(),
        file_count: files.len(),
    })
}

pub fn list_snapshots() -> Result<Vec<SnapshotRecord>, String> {
    let db_path = get_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare("SELECT id, name, created_at, data_file_path FROM snapshots")
        .map_err(|e| e.to_string())?;
        
    let snap_iter = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let created_at: String = row.get(2)?;
        let data_file_path: String = row.get(3)?;
        
        // Count items in file
        let mut reg_count = 0;
        let mut file_count = 0;
        if let Ok(data_str) = fs::read_to_string(&data_file_path) {
            if let Ok(data) = serde_json::from_str::<SnapshotData>(&data_str) {
                reg_count = data.registry_keys.len();
                file_count = data.files.len();
            }
        }

        Ok(SnapshotRecord {
            id,
            name,
            created_at,
            data_file_path,
            reg_count,
            file_count,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for snap in snap_iter {
        if let Ok(s) = snap {
            list.push(s);
        }
    }
    Ok(list)
}

pub fn compare_snapshots_by_id(before_id: &str, after_id: &str) -> Result<SnapshotDiff, String> {
    let before_data = load_snapshot_data(before_id)?;
    let after_data = load_snapshot_data(after_id)?;

    let mut new_registry_keys = Vec::new();
    let mut new_files = Vec::new();

    // Registry Diff
    for key in &after_data.registry_keys {
        if !before_data.registry_keys.contains(key) {
            new_registry_keys.push(key.clone());
        }
    }

    // Filesystem Diff
    for file in &after_data.files {
        if !before_data.files.contains(file) {
            new_files.push(file.clone());
        }
    }

    Ok(SnapshotDiff {
        new_registry_keys,
        new_files,
    })
}

fn load_snapshot_data(id: &str) -> Result<SnapshotData, String> {
    let db_path = get_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare("SELECT data_file_path FROM snapshots WHERE id = ?1")
        .map_err(|e| e.to_string())?;
        
    let data_file_path: String = stmt.query_row([id], |row| row.get(0))
        .map_err(|e| format!("Snapshot not found: {}", e))?;

    let data_str = fs::read_to_string(&data_file_path)
        .map_err(|e| format!("Failed to read snapshot file: {}", e))?;

    let data: SnapshotData = serde_json::from_str(&data_str)
        .map_err(|e| format!("Failed to parse snapshot data: {}", e))?;

    Ok(data)
}

fn scan_registry_keys_recursive(
    hkey: winreg::HKEY,
    hive_name: &str,
    subpath: &str,
    depth: usize,
    max_depth: usize,
    keys: &mut Vec<String>,
) -> Result<(), String> {
    if depth > max_depth {
        return Ok(());
    }

    let root = RegKey::predef(hkey);
    let key = match root.open_subkey_with_flags(subpath, KEY_READ) {
        Ok(k) => k,
        Err(_) => return Ok(()), // Skip inaccessible keys
    };

    for name in key.enum_keys().filter_map(|x| x.ok()) {
        let full_path = format!("{}\\{}", subpath, name);
        keys.push(format!("{}\\{}", hive_name, full_path));
        
        let _ = scan_registry_keys_recursive(hkey, hive_name, &full_path, depth + 1, max_depth, keys);
    }

    Ok(())
}

fn scan_files_recursive(dir: &Path, max_depth: usize, files: &mut Vec<String>) {
    // Walkdir with depth limit
    for entry in WalkDir::new(dir)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        files.push(entry.path().to_string_lossy().to_string());
    }
}
