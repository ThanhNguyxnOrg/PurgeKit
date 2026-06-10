use rusqlite::Connection;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn get_db_path() -> PathBuf {
    let appdata = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let app_dir = appdata.join("PurgeKit");
    if !app_dir.exists() {
        let _ = fs::create_dir_all(&app_dir);
    }
    app_dir.join("purgekit.db")
}

pub fn get_snapshots_dir() -> PathBuf {
    let appdata = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let snap_dir = appdata.join("PurgeKit").join("snapshots");
    if !snap_dir.exists() {
        let _ = fs::create_dir_all(&snap_dir);
    }
    snap_dir
}

pub fn init_db() -> Result<(), String> {
    let db_path = get_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS snapshots (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            data_file_path TEXT NOT NULL
         )",
        [],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
