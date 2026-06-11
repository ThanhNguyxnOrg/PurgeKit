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
  <a href="#-changelog">Changelog</a>
</p>

---

## 💡 What is PurgeKit?

**PurgeKit** is a professional-grade system uninstaller designed specifically for **Power Users** and **Developers** on Windows. Unlike standard uninstallers (like Geek Uninstaller or Revo) which often miss deep developer cache folders, environment configuration files, or local CLI runtimes, PurgeKit aims for **100% trace-free removal** of software, compilers, virtual disks, and development packages.

---

## 🔥 Core Modules

PurgeKit is structured around five main modules:
1. **🧹 Apps Manager & Bulk Silent Uninstaller**: Fetch standard desktop apps and UWP Store packages, uninstall them in batch, and scan/purge leftovers in Registry & file systems.
2. **🗂️ Universal Project Sweeper**: Recursively scans workspace directories for heavy compile folders and dependencies (`node_modules`, `target`, `venv`, `.vs`...) and purges them.
3. **🐋 WSL2 Virtual Disk Shrinker**: Safely compacts bloating virtual drive files (`ext4.vhdx`) using Windows DiskPart and manages dynamic auto-shrink (Sparse mode).
4. **🛠️ Toolchain Version Sweeper & Dev Caches**: Detects and uninstalls unused/obsolete versions of Rustup compiler toolchains and Node runtimes (NVM / FNM) with safe folder purging fallbacks.
5. **🖥️ PATH Environment Cleaner**: Identifies and repairs broken, duplicate, or redundant path variables in Windows User & System environments.

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

Detailed documents are located in the [`docs/`](docs/) directory:
* [✨ Detailed Feature Overview](docs/FEATURES.md) - Learn how deep scanning, snapshot diffing, and WSL2 compaction work.
* [📐 Architecture Design](docs/ARCHITECTURE.md) - Deep dive into Tauri IPC commands, Rust scanner modules, and SQLite schemas.
* [🔧 Development Guide](docs/DEVELOPMENT.md) - Guidelines for setting up, writing commands, and adding new developer caches.

---

## 📝 Changelog

### 🚀 [v1.0.0] - 2026-06-11
This is the first stable release of PurgeKit for Windows.
* **Implemented WSL2 Disk Shrinker**: Quets registered distributions, toggles sparse drive property, and executes safe `diskpart` compaction with a live Terminal UI logs viewer.
* **Implemented Toolchain Version Sweeper**: Detects NVM, FNM, and Rustup runtimes, calculates sizes, identifies active compiler versions, and performs clean uninstallation.
* **Implemented Universal Project Sweeper**: Recursively scans configured folders for heavy dependencies (`node_modules`, `target`, `venv`, etc.) and batch sweeps them.
* **Implemented Bulk Silent Uninstaller**: Decoupled registry scan, UWP package manager, and remnant cleaning with silent fallback.
* **Improved Responsiveness**: Fixed vertical and horizontal overflow layouts to ensure full usability in compact, windowed multitasking modes.

---

<div align="center">
  <sub>Made with ❤️ by <a href="https://github.com/ThanhNguyxnOrg">ThanhNguyxnOrg</a></sub>
</div>
