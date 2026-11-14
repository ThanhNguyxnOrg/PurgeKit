# 📥 PurgeKit Installation & Setup Guide (Windows)

Welcome to **PurgeKit**! This guide walks you through installing, upgrading, and initializing PurgeKit on your system.

---

> [!IMPORTANT]
> **Platform Support**: PurgeKit is currently optimized and compiled exclusively for **Windows 10 and Windows 11**. It relies on native Win32 FFI bindings, registry structures, and Windows-specific utilities (like `DiskPart` and `wsl.exe`). There is currently no support for macOS or Linux platforms.

---

## 🛠️ Step-by-Step Installation

### 1. Download the Installer
Go to the official GitHub releases page:
👉 [PurgeKit GitHub Releases](https://github.com/ThanhNguyxnOrg/PurgeKit/releases)

Look for the latest release version (e.g., `v1.x.y`) and download one of the following assets:
*   **`PurgeKit_x64_en-US.msi`**: The recommended installer package. It installs PurgeKit to your system and creates start menu and desktop shortcuts.
*   **`PurgeKit_x64-setup.exe`**: The standalone setup executable.

---

### 2. Run the Installer
If using the `.msi` installer:
1. Double-click the `.msi` file.
2. If Windows Defender SmartScreen blocks the launch with a blue window ("Windows protected your PC"), click **More info** and select **Run anyway**. This warning appears because the executable does not have an expensive EV Code Signing Certificate.
3. Follow the wizard steps to complete the installation.

---

### 3. Launch & Elevate on Demand
PurgeKit starts with **normal user rights** (an `asInvoker` manifest). Read-only features — inventory, disk scanning, snapshots, audits — work instantly without a UAC prompt.

Operations that need Administrator rights (HKLM edits, Program Files deletions, `DiskPart` compaction, scheduled tasks, System PATH writes) are guarded by the backend and will either:

*   prompt you to relaunch elevated from the in-app banner (**Restart as Administrator**), or
*   refuse with a clear "requires Administrator" message instead of failing silently.

> [!WARNING]
> Without elevation, remnant purging, snapshot compares, WSL compaction and PATH repair are blocked with safety banners. Run the app as Administrator when doing heavy cleanup work.

---

## 📂 Directories & File Storage

PurgeKit saves all configurations, snapshots, backups, and databases locally on your computer. No data is sent to external servers.

Here are the locations of the directories created on your system:

| Data Type | Directory Path | Description |
|---|---|---|
| **SQLite Database** | `%AppData%\PurgeKit\purgekit.db` | Contains snapshot records and quarantine listings |
| **System Snapshots** | `%AppData%\PurgeKit\snapshots\` | Compressed JSON files representing system baselines |
| **Settings File** | `%LocalAppData%\PurgeKit\settings.json` | Stores user-configured presets and exclusions (machine-wide overrides possible, see CONFIGURATION.md) |
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
