use std::fs;
use std::path::PathBuf;

use chrono::Local;

fn logs_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\AppData\Local"));
    let dir = base.join("PurgeKit").join("Logs");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn safe_operation_name(operation: &str) -> String {
    let name: String = operation
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "operation".to_string()
    } else {
        name
    }
}

pub fn write_operation_log(operation: &str, lines: &[String]) -> Result<PathBuf, String> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let file_name = format!("{}_{}.log", timestamp, safe_operation_name(operation));
    let path = logs_dir().join(file_name);
    let mut content = format!(
        "PurgeKit operation log\noperation: {}\ncreated: {}\n\n",
        operation,
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    content.push_str(&lines.join("\n"));
    content.push('\n');
    fs::write(&path, content).map_err(|error| format!("Failed to write operation log: {}", error))?;
    Ok(path)
}
