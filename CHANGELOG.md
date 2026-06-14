# Changelog

All notable changes to this project will be documented in this file.

## [1.1.0] - 2026-06-14

### 🔒 Security Hardening & LPE Prevention
* **🛡️ Secure PATH Utility**: Implemented a centralized secure system PATH lookup querying admin-writable `HKLM` hives, overriding the PATH variable in all spawned process executions (PowerShell, CMD, WSL, DiskPart, XCopy, Registry Export) to block path hijacking attacks.
* **🚧 Confinement & Safety Gates**:
  - Integrated `winutil::is_safe_to_delete` checks into dev tool cache purging and CLI package uninstallation workflows.
  - Added strict path confinement (`is_in_startup_dir` and `is_in_tasks_dir`) to restrict file operations in Startup Manager to valid directories, returning `Access Denied` on unauthorized paths.
  - Safe Unicode and UWP package name parsing to prevent crashes/panics on multi-byte characters in uninstall strings.
* **📂 Dotfolder Repository Protection**: Remnants scans now strictly ignore dotfolders/dotfiles (like `.git`, `.vscode`, `.idea`, `.gradle`) inside sensitive user directories (`Desktop`, `Documents`, `Downloads`), protecting local code repos and IDE configurations from accidental cleanup.

### ⚙️ Dynamic Developer Tools Rules
* **📝 Data-Driven Configuration**: Dev tools definitions are now loaded dynamically from `%LOCALAPPDATA%\PurgeKit\devtools_rules.json` (auto-created with 16 defaults if missing).
* **⚡ Template Resolution**: Supports environment templates (`{APPDATA}`, `{LOCALAPPDATA}`, `{USERPROFILE}`) and dynamic cache command resolution.
* **🛡️ Safe Fallback Purging**: Custom user rules bypass command execution entirely to prevent LPE. Instead, they trigger secure direct directory purging using safe deletion gates.

### 🚀 Advanced Startup Manager & Scheduled Tasks
* **🔍 Registry Hives Expansion**: Expanded Startup Manager scanning to cover `Wow6432Node` user run keys and legacy `RunServices`/`RunServicesOnce` hives under both `HKCU` and `HKLM`.
* **⏰ Logon & Boot Scheduled Tasks**:
  - Recursively scans `C:\Windows\System32\Tasks` for Scheduled Tasks triggered at boot/logon.
  - Parses XML task actions safely for `<Command>`, `<Arguments>`, and COM `<ClassId>` actions.
  - Supports enabling/disabling tasks by appending/removing `.disabled` to task files under strict Admin validation.

### 🐛 GitHub Integration & Reporting
* **📝 Dynamic Bug Reporting**: Split reports into dedicated Bug Reports (automatically gathers OS version, Admin status, and app version for GitHub pre-population) and Feature Suggestions.

---

## [1.0.0] - 2026-06-11

### 🎉 Welcome to PurgeKit v1.0.0! 🚀

We are thrilled to announce the first stable release of **PurgeKit** – the ultimate developer-focused system uninstaller and deep cleaner built for Windows 10 & 11! 🛡️💻

PurgeKit is specifically designed to go beyond typical Windows uninstallers, ensuring that developer caches, build directories, compilers, virtual disks, and broken environment variables are surgically cleaned from your machine.

---

### 🔥 Major Feature Showcases

#### 🧹 1. Apps Manager & Bulk Silent Uninstaller
* **📦 Complete App Registry Crawlers**: Detects standard Win32 applications and UWP Windows Store packages with elevated registry scans.
* **⚡ Silent Uninstallation**: Automates uninstallation using silent/quiet flags so you don't have to click "Next" repeatedly.
* **🔍 Remnant Deep Scan**: Scans standard directories (`AppData`, `ProgramFiles`, `ProgramData`) and registry keys (`HKLM`, `HKCU`) to purge leftovers automatically.

#### 🗂️ 2. Universal Project Sweeper
* **📂 Directory Crawling**: Instantly scans your workspace folders for bulky dependencies (`node_modules`, `target`, `venv`, `.vs`, etc.).
* **📈 Size Estimation**: Visualizes exactly how much disk space is wasted on build artifacts before cleaning.
* **🎥 Live Events Stream**: Outputs real-time Tauri/Rust walkdir log events directly to the UI.

#### 🐋 3. WSL2 Virtual Disk Shrinker
* **💾 VHDX Compaction**: Safely shrinks bloated Windows Subsystem for Linux (WSL2) `ext4.vhdx` virtual disks using native Windows `diskpart`.
* **⚙️ Sparse Mode Toggle**: Easily enables/disables the NTFS sparse property on your WSL disks to restrict auto-bloat.
* **📜 Compact Log Viewer**: Live stdout stream of the compaction process in a retro-terminal UI.

#### 🛠️ 4. Toolchain Version Sweeper
* **🦀 Rustup Manager**: Scans and uninstalls unused/obsolete toolchains, target architectures, and cargo build directories.
* **🟢 Node.js Version Sweeper**: Detects and purges unused NVM or FNM Node.js runtimes.

#### 🖥️ 5. PATH Environment Cleaner
* **🔗 Broken Link Auto-Detect**: Identifies invalid, non-existent directory paths clogging your User and System `PATH`.
* **✨ Deduplication**: Instantly highlights and removes duplicate entries to clean up Windows environment variables safely.

---

### 📸 Technical Underpinnings

* **💾 Snapshot Diff Engine**: Takes a system-wide baseline snapshot of your Registry and filesystem, runs the app installer, and compares the differences (powered by SQLite and HashSets) to track every file added.
* **📡 Active Installation Tracker**: Hooks into the NTFS USN Journal using Win32 FFI to capture real-time file creation events as they occur.
* **🛡️ Quarantine Safe-Guard**: Every registry key deleted is exported to a `.reg` file, and files are copied to a secure Local AppData quarantine folder before erasure. Easily restore anything with one click.
* **🔐 UAC Manifest Elevation**: Includes Windows elevation manifests to ensure registry/diskpart actions run under secure Admin privileges.

---

### 🔧 Fixes & Optimizations
* **📱 Interface Responsiveness**: Fully responsive, liquid-grid dashboard supporting compact multi-tasking modes.
* **⚡ Rust Performance**: Low-overhead Win32 API interactions for ultra-fast NTFS USN tracking.
* **🛠️ Bugfix**: Resolved invalid nesting in Svelte Dev Tools dashboard wrapper.

---

### 📥 Installation Guide
1. Download `PurgeKit_x64_en-US.msi` or `PurgeKit_x64-setup.exe` from the assets below.
2. Run the installer and grant **Administrator Privileges** when prompted (required for DiskPart & Registry operations).
3. Open PurgeKit from the Start Menu and start cleaning! 🚀

---
<sub>Made with ❤️ by the **ThanhNguyxnOrg** team. Feel free to open issues or contribute!</sub>
