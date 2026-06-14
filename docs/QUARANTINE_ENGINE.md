# 🛡️ Quarantine & Backup Engine

To ensure that system cleaning is risk-free, PurgeKit features a robust **Quarantine & Backup Engine**. Instead of deleting files and registry keys permanently right away, PurgeKit backs them up, allowing developers to restore items if an uninstallation triggers unexpected system behavior.

---

## 🏗️ Architecture & Storage Hives

When an application remnant or compiler version is swept:
1.  **Registry Backups**: PurgeKit exports registry keys into standard Windows Registry files (`.reg`) before they are deleted.
    *   **Folder Location**: `%LocalAppData%\PurgeKit\Backups\`
    *   **Naming Pattern**: `[TIMESTAMP]_[REG_PATH].reg` (with backslashes replaced by underscores).
    *   **Command execution**: Runs the native Windows CLI tool:
        `reg export [KEY_PATH] [BACKUP_FILE_PATH] /y`
2.  **Filesystem Quarantine**: Files and directories are moved to a sandboxed quarantine folder.
    *   **Folder Location**: `%LocalAppData%\PurgeKit\Quarantine\`
    *   **Naming Pattern**: `[TIMESTAMP]_[UUID]_[FOLDER_NAME]`

---

## 🗄️ Database Tracking Schema

Quarantined files/directories are tracked in the local SQLite database (`purgekit.db`) in the `quarantine` table:

```sql
CREATE TABLE IF NOT EXISTS quarantine (
    id TEXT PRIMARY KEY,             -- Unique UUID v4
    name TEXT NOT NULL,              -- File/Folder base name
    original_path TEXT NOT NULL,     -- Absolute original path on disk
    quarantine_path TEXT NOT NULL,   -- Absolute path inside the Quarantine directory
    created_at TEXT NOT NULL         -- Snapshot timestamp of quarantine action (YYYYMMDD_HHMMSS)
);
```

---

## 🧼 The Quarantine Lifecycle

### 1. Moving to Quarantine
When you clean files, PurgeKit calls `quarantine_file_or_directory`:
*   **Fast Rename**: It first attempts a fast rename `fs::rename` from the source path to the quarantine path.
*   **Copy-and-Delete Fallback**: If renaming fails (e.g. crossing drive volumes from `D:\` to `C:\`), PurgeKit:
    *   Recursively copies all directory contents to the quarantine destination.
    *   Deletes the original files/folders once copying completes successfully.
*   **Database Log**: Creates a record in the database.

---

### 2. Restoring Quarantined Items
If a restored application needs its folders back, clicking **Restore** in the UI executes `restore_quarantine_item`:
*   **Safety Checks**: If the original destination path points to protected locations (`C:\Program Files`, `C:\Windows`, or `C:\ProgramData`), the engine checks for Admin rights. If not elevated, it cancels the action with an error.
*   **Recreation**: Automatically creates any missing parent directories.
*   **Move Back**: Tries to rename the quarantine folder back to the original path.
*   **Copy Fallback**: If renaming fails, it runs:
    `cmd /C xcopy /E /I /Y "[QUARANTINE_PATH]" "[ORIGINAL_PATH]"`
    Upon completion, the quarantined copy is deleted.
*   **Clean database**: Deletes the metadata row from the SQLite database.

---

### 3. Permanent Deletion
Clicking **Delete Permanently** in the Quarantine tab:
*   Deletes the quarantined files or directories using `fs::remove_file` or `fs::remove_dir_all`.
*   Clears the tracking metadata row from the database.
*   Frees up host drive space.
