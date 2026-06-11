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
  <a href="#-key-features">Key Features</a> • 
  <a href="#-quick-start">Quick Start</a> • 
  <a href="#-cli-usage">CLI Usage</a> • 
  <a href="#-architecture">Architecture</a> • 
  <a href="#-changelog">Changelog</a>
</p>

---

## 💡 What is PurgeKit?

**PurgeKit** is a professional-grade system uninstaller designed specifically for **Power Users** and **Developers** on Windows. 

Unlike standard uninstallers (like Geek Uninstaller or Revo) which often miss deep developer cache folders, environment configuration files, or local CLI runtimes, PurgeKit aims for **100% trace-free removal** of software, compilers, virtual disks, and development packages.

---

## 🔥 Key Features

### 1. 🧹 Apps Manager & Bulk Silent Uninstaller
* **Hybrid Scanner**: Fetches both standard desktop apps (x86/x64) and Windows Store packages (UWP).
* **Batch Operations**: Select multiple applications and uninstall them sequentially in a single click.
* **Auto-Purge Leftovers**: Leverages regex-based rules to scan `%AppData%`, `%LocalAppData%`, `%ProgramData%`, and Windows Registry keys for orphaned leftovers.

### 2. 🗂️ Universal Project Sweeper
* **Developer Directory Cleaner**: Scan workspace roots (e.g., `D:\Code`, `D:\Projects`) for space-hogging build artifacts and dependencies.
* **Supported Frameworks**: Includes NodeJS (`node_modules`), Rust (`target`), Python (`venv`, `.venv`), C#/.NET (`bin`, `obj`, `.vs`), Java/Gradle (`build`, `.gradle`), SvelteKit (`.svelte-kit`), and Next.js (`.next`).
* **Batch Purging**: Calculates exact directory sizes and sweeps them safely with multi-threaded deletion.

### 3. 🐋 WSL2 Virtual Disk Shrinker
* **WSL VHDX Compaction**: Safely compacts bloating virtual drive files (`ext4.vhdx`) using Windows DiskPart with live logs streamed directly to a custom Terminal UI.
* **Auto-Shrink (Sparse Mode)**: Toggle the NTFS `FILE_ATTRIBUTE_SPARSE_FILE` attribute on WSL virtual disks, allowing Windows to reclaim deleted blocks dynamically.
* **Distro Manager**: Displays names, paths, sizes, and sparse properties of all registered WSL distributions.

### 4. 🛠️ Toolchain Version Sweeper & Dev Caches
* **Compiler Cleaner**: Detect and uninstall old/unused versions of Rustup compiler toolchains and Node.js runtimes (NVM and FNM).
* **Safe Fallback**: Invokes the official CLI uninstall commands with direct directory purging fallback if they are missing from `PATH`.
* **Safety Lock**: Prevents accidental deletion of the active compiler/runtime version currently set in the environment.
* **Dev Cache Cleaner**: Drains caches for Cargo, npm, yarn, pip, bun, and VS Code.

### 5. 🖥️ PATH Environment Cleaner
* **Env Sanitizer**: Scans Windows User and System `PATH` variables for broken, duplicate, or redundant directory paths.
* **One-Click Repair**: Clean up invalid paths to prevent environment conflicts.

---

## ⚡ Quick Start

### 📦 Prerequisites
* **Rust Toolchain** (MSRV 1.77+)
* **Node.js** (v18+)
* **Windows 10 / 11** (with Administrator privileges for disk/registry changes)

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

# Clean developer tool caches (e.g. cargo, npm)
purgekit.exe cache prune --all
purgekit.exe cache prune npm cargo

# Compact a WSL2 distribution disk
purgekit.exe wsl compact "Ubuntu"
```

---

## 📐 Architecture

PurgeKit is split into two components:
1. **Core Backend (Rust)**: High-performance registry scanning, Windows Win32 API interactions, filesystem traversal, process creation, and DiskPart scripting.
2. **Frontend UI (Svelte 5 & CSS)**: A fully responsive, modern glassmorphism UI styled with Tailwind CSS, supporting horizontal table scrolling, auto-adapt layouts, and real-time logs streaming over IPC events.

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
