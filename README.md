# PurgeKit

<p align="center">
  <img src="static/logo.png" alt="PurgeKit Logo" width="140" />
</p>

<h3 align="center"><b>The Ultimate Developer-Focused System Uninstaller & Deep Cleaner</b> 🚀</h3>

<p align="center">
  <i>Clean 100% of app remnants, developer tool caches, virtual disks, and environment PATH junk with surgical precision.</i>
</p>

<p align="center">
  <a href="https://tauri.app"><img src="https://img.shields.io/badge/Tauri-v2.0-E85D26?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri Version" /></a>
  <a href="https://svelte.dev"><img src="https://img.shields.io/badge/Svelte-v5.0-FF3E00?style=for-the-badge&logo=svelte&logoColor=white" alt="Svelte Version" /></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2024-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" /></a>
  <a href="https://www.typescriptlang.org"><img src="https://img.shields.io/badge/TypeScript-v5.6-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" /></a>
  <img src="https://img.shields.io/badge/Platform-Windows_10_|_11-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Platform" />
</p>

<p align="center">
  <a href="#-core-modules">Core Modules</a> • 
  <a href="#-quick-start">Quick Start</a> • 
  <a href="#-cli-usage">CLI Usage</a> • 
  <a href="#-documentation">Documentation</a> • 
  <a href="CHANGELOG.md">Changelog</a> • 
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

---

## 💡 What is PurgeKit?

**PurgeKit** is a professional-grade system uninstaller designed specifically for **Power Users** and **Developers** on Windows. Unlike standard uninstallers (like Geek Uninstaller or Revo) which often miss deep developer cache folders, environment configuration files, or local CLI runtimes, PurgeKit aims for **100% trace-free removal** of software, compilers, virtual disks, and development packages.

---

## 🔥 Core Modules

PurgeKit is structured around five main modules:
1. **🧹 Apps Manager & Bulk Silent Uninstaller**: Fetch standard desktop apps and UWP Store packages, uninstall them in batch, and scan/purge leftovers in Registry & file systems.
2. **🗂️ Universal Project Sweeper**: Recursively scans workspace directories for heavy compile folders and dependencies (`node_modules`, `target`, `venv`, `.vs`...) and purges them. ([Guide](docs/PROJECT_SWEEPER.md))
3. **🐋 WSL2 Virtual Disk Shrinker**: Safely compacts bloating virtual drive files (`ext4.vhdx`) using Windows DiskPart and manages dynamic auto-shrink (Sparse mode). ([Guide](docs/WSL_DISK_SHRINKER.md))
4. **🛠️ Toolchain Version Sweeper & Dev Caches**: Detects and uninstalls unused/obsolete versions of Rustup compiler toolchains and Node runtimes (NVM / FNM) with safe folder purging fallbacks. ([Guide](docs/TOOLCHAIN_SWEEPER.md))
5. **🖥️ PATH Environment Cleaner**: Identifies and repairs broken, duplicate, or redundant path variables in Windows User & System environments. ([Guide](docs/PATH_CLEANER.md))

*For in-depth explanations of how each module works, see [✨ Detailed Feature Overview](docs/FEATURES.md).*

---

## ⚡ Quick Start

### 📦 Prerequisites
* **Rust Toolchain** (MSRV 1.77+)
* **Node.js** (v18+)
* **Windows 10 / 11** (Administrator privileges required for registry & DiskPart operations)

### 💻 Local Development
```bash
# Clone the repository
git clone https://github.com/ThanhNguyxnOrg/PurgeKit.git
cd PurgeKit

# Install frontend dependencies
npm install

# Run the Tauri application in developer mode
npm run tauri dev
```

### 🔨 Building the Installer
To compile the production build and generate standard `.msi` and `.exe` installers:
```bash
npm run tauri build
```
The installers will be generated under `src-tauri/target/release/bundle/`.

---

## 🐚 CLI Usage

PurgeKit includes a standalone CLI executable for script automation and CI/CD pipelines.

```bash
# Clean an application and its remnants silently
purgekit.exe clean "AppName"

# Clean developer tool caches
purgekit.exe cache prune --all
purgekit.exe cache prune npm cargo

# Compact a WSL2 distribution disk
purgekit.exe wsl compact "Ubuntu"
```

---

## 📖 Documentation
 
Detailed documents are categorized below and located in the [`docs/`](docs/) directory:

### 🏗️ Architecture, Security & Setup
*   [📐 Technical Architecture](docs/ARCHITECTURE.md) - Deep dive into Tauri IPC commands, Rust backend modules, and SQLite schemas.
*   [🔐 Security & UAC Elevation Model](docs/SECURITY_UAC.md) - Trust manifest, TokenElevation checks, LPE prevention, and privilege boundaries.
*   [📥 Installation & Setup Guide](docs/INSTALLATION.md) - How to download, install, and initialize PurgeKit on Windows.
*   [⚙️ Configuration & Settings Guide](docs/CONFIGURATION.md) - Format of `settings.json`, scan safety levels (`safe`, `moderate`, `aggressive`), and exclusions.
*   [🚨 Troubleshooting & Safety Guide](docs/TROUBLESHOOTING.md) - Common permissions warnings, locked files, and UAC troubleshooting.

### 🧹 Core Cleaners & Sweepers
*   [✨ Detailed Feature Overview](docs/FEATURES.md) - Full capability breakdown of scanning, deep clean heuristics, and cleaners.
*   [🗂️ Universal Project Sweeper Guide](docs/PROJECT_SWEEPER.md) - Folder presets (`node_modules`, `target`, `venv`), traversal limits, and live events.
*   [🐋 WSL2 Virtual Disk Shrinker Guide](docs/WSL_DISK_SHRINKER.md) - Ext4 VHDX compaction, DiskPart script automation, and sparse mode.
*   [🛠️ Toolchain Version Sweeper Guide](docs/TOOLCHAIN_SWEEPER.md) - Rustup, NVM, and FNM version directories and active compiler locks.
*   [🖥️ PATH Environment Cleaner Guide](docs/PATH_CLEANER.md) - User & System Registry PATH sanitization, variable expansion, and `WM_SETTINGCHANGE`.
*   [📦 Global CLI Package Sweeper Guide](docs/GLOBAL_CLI_SWEEPER.md) - Audit and uninstall global tools (npm, yarn, pnpm, cargo, pip, go).
*   [🚀 Startup Manager & Autoruns Guide](docs/STARTUP_MANAGER.md) - Registry Run keys, user startup folders, and scheduled tasks disabling/enabling.

### 🔬 Low-Level Engines & Tracking
*   [📸 System Snapshot Engine Guide](docs/SNAPSHOT_ENGINE.md) - Baseline snapshotting, registry/filesystem crawls, and $O(1)$ diff engine.
*   [📡 Active Installation Tracker Guide](docs/ACTIVE_TRACKER.md) - Real-time file creation monitoring via the Windows NTFS USN Journal.
*   [🛡️ Quarantine & Backup Engine Guide](docs/QUARANTINE_ENGINE.md) - Automatic `.reg` exports, sandbox quarantine, and one-click restoration.

### 🛠️ Developer, CLI & Operations
*   [🐚 PurgeKit CLI Reference Guide](docs/CLI.md) - Command-line interface syntax, script automation, and headless flags.
*   [🔧 Development Guide](docs/DEVELOPMENT.md) - Local environment bootstrap, adding new dev tool cache rules, and design tokens.
*   [🚀 Automated Release Workflow (CI/CD)](docs/CI_CD_RELEASES.md) - GitHub Actions workflows, automated changelog parsing, and releases.
*   [🤝 Contributing Guidelines](CONTRIBUTING.md) - Code standards, Svelte 5 runes, and pull request workflows.
*   [📝 Full Project Changelog](CHANGELOG.md) - Historical version releases and security hardening records.

---

## 📝 Changelog

### 🚀 [v1.2.0] - 2026-10-07
Major security hardening, reliability, and Win32 safety architecture update:
* **Enterprise Win32 Safety & Centralized Primitives**: Unified all canonicalization, safe path boundaries, service protection, and argument parsing under `winutil.rs`.
* **System PATH Zeroing Fail-Safe**: Backend & UI guards completely block accidental clearing of the Windows System PATH.
* **Uninstaller Privilege Escalation Bypass Defense**: Pre-expands `%VAR%` environment tokens prior to user-writable path inspection and WinVerifyTrust signature checks.
* **Hardened DevTools Rules**: Command validation eliminates shell injection vulnerabilities across all dynamic developer tool cache scripts.
* **Fast Locker Direct Branching**: Optimized directory deletion bypassing redundant `remove_file` system call roundtrips.

*For full historical notes and previous releases, see [📝 CHANGELOG.md](CHANGELOG.md).*

---

<div align="center">
  <sub>Made with ❤️ by <a href="https://github.com/ThanhNguyxnOrg">ThanhNguyxnOrg</a></sub>
</div>
