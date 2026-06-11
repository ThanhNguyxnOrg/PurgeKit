# 📥 PurgeKit Installation & Setup Guide (Windows)

Welcome to **PurgeKit v1.0.0**! This guide walks you through installing, upgrading, and initializing PurgeKit on your system.

---

> [!IMPORTANT]
> **Platform Support**: PurgeKit is currently optimized and compiled exclusively for **Windows 10 and Windows 11**. It relies on native Win32 FFI bindings, registry structures, and Windows-specific utilities (like `DiskPart` and `wsl.exe`). There is currently no support for macOS or Linux platforms.

---

## 🛠️ Step-by-Step Installation

### 1. Download the Installer
Go to the official GitHub releases page:
👉 [PurgeKit GitHub Releases](https://github.com/ThanhNguyxnOrg/PurgeKit/releases)

Look for the latest release version (e.g., `v1.0.0`) and download one of the following assets:
*   **`PurgeKit_1.0.0_x64_en-US.msi`**: The recommended installer package. It installs PurgeKit to your system and creates start menu and desktop shortcuts.
*   **`PurgeKit.exe`**: A portable version of the app. You can run it directly without installing.

---

### 2. Run the Installer
If using the `.msi` installer:
1. Double-click the `.msi` file.
2. If Windows Defender SmartScreen blocks the launch with a blue window ("Windows protected your PC"), click **More info** and select **Run anyway**. This warning appears because the executable does not have an expensive EV Code Signing Certificate.
3. Follow the wizard steps to complete the installation.

---

### 3. Launch with Administrator Rights
To function correctly, PurgeKit requires elevated system permissions to access system registries, sweep system-wide caches, and shrink WSL files.

*   By default, the application is configured with a manifest that triggers a Windows User Account Control (UAC) prompt when launched.
*   Click **Yes** when prompted by UAC to allow PurgeKit to make changes to your device.

> [!WARNING]
> If you launch the app without Administrator privileges, many features (such as remnant purging, system snapshot compares, WSL virtual disk compacting, and PATH cleaners) will show safety warning banners and will be unable to modify files or keys.

---

## 📂 Directories & File Storage

PurgeKit saves all configurations, snapshots, backups, and databases locally on your computer. No data is sent to external servers.

Here are the locations of the directories created on your system:

| Data Type | Directory Path | Description |
|---|---|---|
| **SQLite Database** | `%AppData%\PurgeKit\purgekit.db` | Contains snapshot records and quarantine listings |
| **System Snapshots** | `%AppData%\PurgeKit\snapshots\` | Compressed JSON files representing system baselines |
| **Settings File** | `%AppData%\PurgeKit\settings.json` | Stores user-configured presets and exclusions |
| **Registry Backups** | `%LocalAppData%\PurgeKit\Backups\` | `.reg` registry keys exported before deletion |
| **Filesystem Quarantine** | `%LocalAppData%\PurgeKit\Quarantine\` | Folders/files moved here before deletion |

---

## 🔄 How to Uninstall PurgeKit

If you need to uninstall PurgeKit:
1. Open Windows **Settings** (Win + I) ➡️ **Apps** ➡️ **Installed apps**.
2. Search for **PurgeKit**.
3. Click the three dots and select **Uninstall**.
4. To clean up leftover databases, settings, and snapshots, delete the following directories manually:
    *   `%AppData%\PurgeKit`
    *   `%LocalAppData%\PurgeKit`
