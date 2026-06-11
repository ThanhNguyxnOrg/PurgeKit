# ✨ PurgeKit Detailed Features Guide (v1.0.0)

This document provides a comprehensive, developer-level breakdown of all core features available in **PurgeKit v1.0.0** and how they interact with the Windows operating system, filesystem, and registry under the hood.

---

## 🔎 1. Active Monitoring & System Snapshots

One of the standout features of PurgeKit is its ability to track application installations dynamically. Instead of relying solely on guesswork after an application is installed, PurgeKit allows you to capture the state of the OS before and after an installer runs, isolating every file and registry key modified.

### 📸 Registry & Filesystem Snapshotting
Before running an installer, you capture a **Baseline Snapshot**:
* **Registry Crawler**: PurgeKit recursively crawls three primary registry hives:
  - `HKEY_CURRENT_USER\SOFTWARE`
  - `HKEY_LOCAL_MACHINE\SOFTWARE`
  - `HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node` (representing 32-bit software installed on 64-bit Windows)
  It logs all key paths, value names, types, and values.
* **Filesystem Crawler**: It crawls major system and user directories:
  - `%AppData%` (`C:\Users\<User>\AppData\Roaming`)
  - `%LocalAppData%` (`C:\Users\<User>\AppData\Local`)
  - `%ProgramData%` (`C:\ProgramData`)
  - `%ProgramFiles%` (`C:\Program Files`)
  - `%ProgramFiles(x86)%` (`C:\Program Files (x86)`)
  It catalogs all file names, directory trees, file sizes, and creation dates.
* **Storage & Indexing**: Snapshots are serialized into a compressed JSON structure and saved to `.gemini/antigravity-ide/snapshots/` in the user's AppData directory. Key metadata is stored in a local SQLite database (`purgekit.db`) for quick querying.

### ⚖️ Comparison (Diff) Engine
After the application installation completes, you take a **Post-Install Snapshot**. The diff engine performs a comparison between the two snapshots:
* **Registry Additions**: Identifies keys or values present in the post-install snapshot that were absent in the baseline.
* **Filesystem Additions**: Identifies files, binaries, or folder hierarchies created during the installation timeframe.
* **Deep Clean Output**: The differences are presented in a Git-like diff format (marked with `+`). When you decide to uninstall the app, PurgeKit uses this diff list to target and remove every single addition, returning the system to its exact pre-installation state.

---

## 🧹 2. Apps Manager & Bulk Silent Uninstaller

PurgeKit provides a centralized management console to inspect, uninstall, and deep-clean standard desktop applications and Windows Store packages (UWP).

### 🔎 Hybrid Scanner
PurgeKit queries two distinct sources to compile its application list:
1. **Desktop Applications (Registry)**: Queries the standard Windows uninstallation registry paths to extract display names, publishers, versions, install locations, and uninstallation strings:
   - `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`
   - `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall`
   - `HKLM\Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
2. **Windows Store Apps (UWP)**: Executes PowerShell bindings in the background (`Get-AppxPackage -AllUsers`) to retrieve installed package names, publishers, versions, installation directories, and package IDs.

### 📦 Bulk Silent Uninstallation
* **Command Extraction**: PurgeKit inspects the registry uninstallation string. If a silent or quiet uninstallation parameter is supported (e.g. `/S`, `/quiet`, `/qn`, `/uninstall`, `/silent`), it extracts and runs it.
* **Execution Flow**: Spawns uninstallation processes sequentially. UWP packages are uninstalled using native PowerShell bindings (`Remove-AppxPackage`).
* **Auto-Purge Leftovers**: Leverages regex heuristics to clean remnant directories and registry keys left behind by standard uninstallers, operating silently in the background.

---

## 🗂️ 3. Universal Project Sweeper

A crucial module for software developers, the **Universal Project Sweeper** scans programming directories to find and purge space-consuming build artifacts, temporary caches, and dependency folders.

### 🔍 Search Presets & Traversals
You specify one or more workspace directories (e.g. `D:\Code`). PurgeKit performs a multi-threaded recursive directory search using `walkdir` to find matching targets:
* **NodeJS**: `node_modules` (dependency directory)
* **Rust**: `target` (cargo build output)
* **Python**: `venv`, `.venv` (virtual environments)
* **C# / .NET**: `bin`, `obj` (build output), `.vs` (Visual Studio user settings cache)
* **Java / Gradle**: `build` (compiler output), `.gradle` (Gradle wrapper cache)
* **SvelteKit**: `.svelte-kit` (Svelte compiler cache)
* **Next.js**: `.next` (Next.js build files)

### 📊 Size Calculation & Last Modified Time
* **Size Accumulation**: For each matching folder, PurgeKit recursively sums the file sizes of all nested files rather than relying on folder metadata, ensuring 100% accuracy.
* **Last Modified Timestamp**: Queries file metadata to determine the last time a file in the directory was written to. This helps identify stale or abandoned projects that have not been touched in months.
* **Batch Purging**: Users can select all or specific directories and wipe them in a single, safe, multi-threaded operation.

---

## 🐋 4. WSL2 Virtual Disk Shrinker

Windows Subsystem for Linux 2 (WSL2) uses virtual hard disks (`ext4.vhdx`) to store files. These files expand automatically as files are created inside Linux. However, when files are deleted in Linux, the Windows host does not automatically shrink the `.vhdx` file, causing the virtual drive to occupy gigabytes of unused space on the Windows host.

### 🛠️ DiskPart Compaction Sequence
PurgeKit automates the complex manual compaction steps:
1. **WSL Termination**: Shuts down the WSL subsystem via `wsl --shutdown` to release all file locks on the virtual drives.
2. **Registry Lookup**: Queries `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss` to discover registered WSL distributions, their base directories, and `ext4.vhdx` pathways.
3. **DiskPart Script Generation**: Creates a temporary DiskPart script with the following sequence:
   ```cmd
   select vdisk file="[PATH_TO_VHDX]"
   attach vdisk readonly
   compact vdisk
   detach vdisk
   ```
4. **Elevation & Terminal Log**: Spawns DiskPart as Administrator (`runas` command equivalent via Tauri) and hooks stdout/stderr, streaming logs line-by-line in real-time to a Svelte console view.

### 🍃 Auto-Shrink (Sparse Mode)
For Windows 11 systems, PurgeKit allows toggling the native sparse property:
`wsl --manage [DistroName] --set-sparse true`
This enables Windows to automatically reclaim unused space from the WSL2 virtual disk in the background, eliminating the need for manual compactions.

---

## 📦 5. Toolchain Version Sweeper

Node runtime managers (NVM, FNM) and Rust compilers (Rustup) save every version you download, consuming gigabytes of storage over time.

### 🔎 Version Discovery
PurgeKit scans the default directories for these tools:
* **Rustup**: `%USERPROFILE%\.rustup\toolchains` (contains compilers like `stable-x86_64-pc-windows-msvc`, `nightly-...`)
* **NVM**: Reads `%NVM_HOME%` or defaults to `%APPDATA%\nvm`, listing directories prefixed with `v` followed by digits.
* **FNM**: Scans candidates like `%LOCALAPPDATA%\fnm\node-versions`, `%APPDATA%\fnm\node-versions`, and `%USERPROFILE%\.fnm\node-versions`.

### 🛡️ Active Safety Lock
Before allowing deletion, PurgeKit runs shell checks in the background (`node -v` and `rustup show active-toolchain`). The versions currently in use are marked with an **Active** badge in the UI. If a user attempts to delete an active compiler, a detailed modal prompt will trigger, requiring explicit confirmation.

### 🐚 Fallback Purging Strategy
* **CLI Command**: Spawns `rustup toolchain uninstall`, `nvm uninstall`, or `fnm uninstall` to perform clean uninstallation.
* **Purge Fallback**: If the runtime manager is missing from the environment PATH, PurgeKit bypasses the CLI and deletes the version directory directly using Windows directory deletion APIs.

---

## 🍃 6. Developer Tool Cache Purger

Developer tool caches store downloaded libraries and logs. PurgeKit queries their default local directories:
* **npm**: `%LocalAppData%\npm-cache`
* **pip**: `%LocalAppData%\pip\Cache`
* **cargo**: `%USERPROFILE%\.cargo\registry` (and `git` index)
* **go**: `%GOPATH%\pkg\mod` and `%GOCACHE%`
* **composer**: `%AppData%\Composer\cache`
* **gradle**: `%USERPROFILE%\.gradle\caches`

Caches are calculated and cleaned using direct directory deletion, freeing up valuable storage instantly.

---

## 🛠️ 7. PATH Environment Cleaner

The Windows `PATH` environment variable accumulates dead links from deleted software, causing terminal command conflicts.

### 🔎 Scans & Identifications
PurgeKit retrieves PATH arrays from the Registry:
* **User PATH**: `HKCU\Environment`
* **System PATH**: `HKLM\System\CurrentControlSet\Control\Session Manager\Environment`
Each entry is validated:
* **Broken**: Resolves nested environment variables (e.g. `%USERPROFILE%`) and checks if the directory exists on disk.
* **Duplicate**: Flags duplicate entries.
* **Redundant (Overlap)**: Identifies if a subdirectory path is already covered by a parent directory in PATH.

### 🔧 Repair & Propagation
When paths are deleted, PurgeKit updates the Registry entries with `REG_EXPAND_SZ` types and broadcasts a Windows message `WM_SETTINGCHANGE` to all running processes so that the updated PATH is loaded immediately without requiring a system reboot.
