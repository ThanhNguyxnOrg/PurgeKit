# ⚙️ Configuration & App Settings

PurgeKit stores settings and configurations locally in a structured JSON file. This allows developers to customize scanning safety tolerances, backup parameters, and default project directories.

---

## 📂 Config File Path

On Windows, the configuration is saved to:
`%LocalAppData%\PurgeKit\settings.json`

*(This translates to `C:\Users\<YourUsername>\AppData\Local\PurgeKit\settings.json`)*

---

## 🗄️ JSON Schema Definition

Below is the JSON schema showing all configurable settings, their types, and default values:

```json
{
  "enable_undocumented_force_unlock": false,
  "scan_level": "safe",
  "backup_before_delete": true,
  "create_restore_point": true,
  "project_scan_paths": []
}
```

---

## 🎛️ Settings Reference Table

| Key | Type | Default | Description |
|---|---|---|---|
| **`enable_undocumented_force_unlock`** | `boolean` | `false` | Enables aggressive handles-unlocking for locked files/keys. |
| **`scan_level`** | `string` | `"safe"` | Safety scan levels: `"safe"`, `"moderate"`, or `"aggressive"`. |
| **`backup_before_delete`** | `boolean` | `true` | Copies swept files/keys into the Quarantine before purging. |
| **`create_restore_point`** | `boolean` | `true` | Creates a system restore point prior to performing bulk purges. |
| **`project_scan_paths`** | `array[string]` | `[]` | Directories automatically swept by the Project Sweeper. |

---

## 🛡️ Details on Scan Levels

The **`scan_level`** parameter affects the remnant scanning heuristics and deletion tolerances:

### 🟢 `safe` (Default)
*   **Targeting**: Checks only exact matches (file names, registry paths) that match known uninstallation traces.
*   **Protection**: Skips folders containing shared assets or registry hives that could impact other applications.
*   **Recommendation**: Safest mode for standard desktop users.

---

### 🟡 `moderate`
*   **Targeting**: Includes fuzzy match heuristics, scanning for parent publisher directories (e.g. `C:\Users\<Name>\AppData\Local\PublisherName`).
*   **Protection**: Asks for double-confirmation before deleting directories containing shared library folders.
*   **Recommendation**: Good for clean-ups of stubborn applications.

---

### 🔴 `aggressive`
*   **Targeting**: Deep-scans the registry for orphan CLSIDs, COM interfaces, and typelibs matching the application publisher.
*   **Protection**: Allows sweeping lock-protected folders. Backups are highly recommended.
*   **Recommendation**: For advanced developers looking to clean every single registry byte.

---

## 🔄 Editing Settings

### 1. Through the UI
1. Launch PurgeKit.
2. Click **Settings** in the Sidebar.
3. Toggle options and add folders under the **Project Scan Paths** manager.
4. Settings are saved automatically upon edit.

---

### 2. Manual Editing
You can edit the `settings.json` file in any text editor (like Notepad or VS Code) while the app is closed. Ensure the JSON syntax is valid; if corrupted, PurgeKit will automatically overwrite the file with default values on next launch.
