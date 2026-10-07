use rusqlite::Connection;
use std::env;
use std::fs;
use std::path::PathBuf;

fn get_base_dir() -> PathBuf {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Roaming"))
                .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public"))
        })
}

pub fn get_db_path() -> PathBuf {
    let app_dir = get_base_dir().join("PurgeKit");
    if !app_dir.exists() {
        let _ = fs::create_dir_all(&app_dir);
    }
    app_dir.join("purgekit.db")
}

pub fn get_snapshots_dir() -> PathBuf {
    let snap_dir = get_base_dir().join("PurgeKit").join("snapshots");
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
            data_file_path TEXT NOT NULL,
            reg_count INTEGER DEFAULT 0,
            file_count INTEGER DEFAULT 0
         )",
        [],
    ).map_err(|e| e.to_string())?;

    // Safe migrations for existing databases
    let _ = conn.execute("ALTER TABLE snapshots ADD COLUMN reg_count INTEGER DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE snapshots ADD COLUMN file_count INTEGER DEFAULT 0", []);

    conn.execute(
        "CREATE TABLE IF NOT EXISTS quarantine (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            original_path TEXT NOT NULL,
            quarantine_path TEXT NOT NULL,
            created_at TEXT NOT NULL
         )",
        [],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
