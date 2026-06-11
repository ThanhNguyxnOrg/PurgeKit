# Changelog

All notable changes to this project will be documented in this file.

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
1. Download `PurgeKit-1.0.0-x64.msi` or `PurgeKit_1.0.0_x64_en-US.msi` from the assets below.
2. Run the installer and grant **Administrator Privileges** when prompted (required for DiskPart & Registry operations).
3. Open PurgeKit from the Start Menu and start cleaning! 🚀

---
<sub>Made with ❤️ by the **ThanhNguyxnOrg** team. Feel free to open issues or contribute!</sub>
