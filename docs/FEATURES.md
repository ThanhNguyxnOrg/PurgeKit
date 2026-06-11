# ✨ PurgeKit Detailed Features

This document provides an in-depth breakdown of all core features available in **PurgeKit** and how they work under the hood.

---

## 🔎 1. Active Monitoring & System Snapshots

One of the key improvements over tools like Geek Uninstaller is PurgeKit's ability to track installations dynamically.

### 📸 Registry & Filesystem Snapshotting
Before running an installer, you can capture a **System Snapshot**:
- **Registry Scan**: PurgeKit scans the hives `HKEY_CURRENT_USER\SOFTWARE` and `HKEY_LOCAL_MACHINE\SOFTWARE` (including `WOW6432Node`) recursively to catalog all keys and values.
- **Filesystem Scan**: It scans common directory pathways like `%AppData%`, `%LocalAppData%`, `%ProgramData%`, and installation drives.
- **JSON Serialization**: File/registry paths are compressed and stored in `snapshots/` in your local app data directory, indexed via a local **SQLite** database (`purgekit.db`).

### ⚖️ Comparison (Diff) Engine
After installing software, you take a second snapshot. The comparison engine runs a diff algorithm:
- **New Registry Keys**: Keys/values present in the post-install snapshot but missing in the baseline.
- **New Files/Folders**: Directories or files created during the installation.
- This outputs a detailed list formatted like a Git diff (`+` additions) which can be permanently purged if you decide to uninstall the app.

---

## 🧹 2. Developer Tool Cache Purger

Developer environments accumulate massive amounts of package caches, compiler logs, and build artifacts. PurgeKit auto-detects these tools and helps reclaim gigabytes of storage.

### 📦 Supported Tools & Caches
| Tool | Detection Method | Cache Paths |
|---|---|---|
| **Node.js / npm** | `node.exe`, `npm.cmd` on PATH | `%LocalAppData%\npm-cache` |
| **Python / pip** | `python.exe`, `pip.exe` on PATH | `%LocalAppData%\pip\Cache` |
| **Rust / cargo** | `rustup.exe`, `cargo.exe` on PATH | `%USERPROFILE%\.cargo` (registry/git caches) |
| **Go** | `go.exe` on PATH | `%GOPATH%\pkg\mod`, `%GOCACHE%` |
| **Ruby / gem** | `ruby.exe`, `gem.cmd` on PATH | `%USERPROFILE%\.gem` |
| **.NET / nuget** | `dotnet.exe` on PATH | `%USERPROFILE%\.nuget\packages` |
| **Java / Maven** | `java.exe`, `mvn.cmd` on PATH | `%USERPROFILE%\.m2\repository` |
| **Docker** | `docker.exe` on PATH | `%ProgramData%\Docker` |
| **pnpm** | `pnpm.cmd` on PATH | `%LocalAppData%\pnpm-store` |
| **yarn** | `yarn.cmd` on PATH | `%LocalAppData%\Yarn\Cache` |
| **Composer** | `composer.phar` on PATH | `%AppData%\Composer\cache` |
| **Gradle** | `gradle.bat` on PATH | `%USERPROFILE%\.gradle\caches` |

---

## 🍃 3. Deep Remnants Scanner

When uninstalling standard apps, they often leave behind registry settings or user profiles. PurgeKit's heuristics look for:
- **Filesystem Junk**: Orphaned folders containing the app name or publisher's name under `%AppData%`, `%LocalAppData%`, `%ProgramData%`, `%ProgramFiles%`, and `Documents`.
- **Registry Junk**: Registry keys and subkeys matching the application's unique tokens under `HKCU\Software` and `HKLM\Software` (including `WOW6432Node`).

---

## 🛠️ 4. PATH Environment Cleaner

The Windows PATH variable easily gets cluttered with entries from deleted programs. PurgeKit scans PATH:
- **Validation**: Resolves environment variables (like `%SystemRoot%`) and tests if the directory exists.
- **Broken Detection**: Paths that point to non-existent folders are labeled `Broken`.
- **Duplicate Detection**: Identifies duplicated path variables.
- **Clean**: Updates the registry `PATH` string dynamically (`REG_EXPAND_SZ`) to remove chosen dead paths (System PATH removal requires administrator privileges).

---

## 🗂️ 5. Universal Project Sweeper

Locates and purges heavy build artifacts, temporary caches, and dependency folders in programming directories.

### 🔍 Search Presets
You configure one or more **Scan Roots** (e.g. `D:\Code`). The sweeper recursively crawls these directories using multi-threaded traversal via `walkdir` to locate folders matching specific preset names:
* **NodeJS**: `node_modules`
* **Rust**: `target`
* **Python**: `venv`, `.venv`
* **C# / .NET**: `bin`, `obj`, `.vs` (Visual Studio cache)
* **Java / Gradle**: `build`, `.gradle`
* **SvelteKit**: `.svelte-kit`
* **Next.js**: `.next`

### ⚡ Size & Modification Resolution
For each match, PurgeKit calculates the exact directory size by summing all files recursively and retrieves the last modified timestamp to let you know which projects have been inactive. 
It supports selective dọn dẹp (cleaning), allowing you to purge hundreds of gigabytes across multiple projects at once.

---

## 🐋 6. WSL2 Virtual Disk Shrinker

WSL2 virtual drives (`ext4.vhdx`) grow as you write files inside Linux. However, when you delete files in Linux, the Windows host does not automatically shrink the `.vhdx` file, leading to massive storage leakage.

### 🛠️ DiskPart Compaction Sequence
PurgeKit provides a safe interface to run VHDX compaction:
1. **WSL Shutdown**: Executes `wsl --shutdown` to free file handles on the virtual drive.
2. **DiskPart Script**: Generates a temporary `diskpart` script file containing:
   ```cmd
   select vdisk file="[PATH_TO_VHDX]"
   attach vdisk readonly
   compact vdisk
   detach vdisk
   ```
3. **Execution & Live Logs**: Spawns `diskpart /s [script]` with Administrator privileges and streams logs line-by-line to Svelte's Terminal view.

### 🍃 Auto-Shrink (Sparse Mode)
In Windows 11, WSL2 supports the sparse drive property. PurgeKit lets you toggle this option by invoking:
`wsl --manage [DistroName] --set-sparse true`
This configures NTFS to mark the `.vhdx` file as sparse, allowing Windows to reclaim empty blocks automatically in the background.

---

## 📦 7. Toolchain Version Sweeper

Node runtime managers (NVM, FNM) and Rust compilers (Rustup) save every compiler version you download, consuming gigabytes of system storage.

### 🔎 Version Discovery
PurgeKit scans the default storage directories for compiler versions:
* **Rustup**: `%USERPROFILE%\.rustup\toolchains`
* **NVM for Windows**: `%NVM_HOME%` or `%APPDATA%\nvm`
* **FNM**: `%LOCALAPPDATA%\fnm\node-versions`, `%APPDATA%\fnm\node-versions`, or `%USERPROFILE%\.fnm\node-versions`.

### 🛡️ Active Safety Lock
To prevent breaking your active developer environment, PurgeKit executes shell commands `node -v` and `rustup show active-toolchain` to identify the version currently active in your environment, and displays a prominent warning in the UI if you attempt to delete them.

### 🐚 Uninstallation Fallback
When you delete a compiler version, PurgeKit attempts the official uninstallation commands (`rustup toolchain uninstall [version]`, `nvm uninstall [version]`, or `fnm uninstall [version]`). If the commands fail or are missing from the system path, PurgeKit falls back to direct folder purging via Windows directory deletion APIs.
