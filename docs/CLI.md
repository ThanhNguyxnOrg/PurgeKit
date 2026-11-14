# 🐚 PurgeKit CLI Reference Guide

<p align="center">
  <img src="https://img.shields.io/badge/CLI-Command_Line-0078D6?style=for-the-badge&logo=windows-terminal&logoColor=white" alt="CLI Badge" />
  <img src="https://img.shields.io/badge/Rust-Clap_v4-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust Clap" />
</p>

This document contains the complete specifications, command-line arguments, execution flags, and scripting automation examples for the **PurgeKit Command Line Interface (CLI)**.

---

## 🚀 Overview

The PurgeKit CLI (`purgekit-cli.exe`) is designed for system administrators, DevOps engineers, and power users who want to automate system cleaning, software removal, and cache pruning through command-line scripts or CI/CD pipelines.

> [!IMPORTANT]
> The CLI requires an **elevated console**: it refuses to start without Administrator rights, so registry, PATH and DiskPart operations can never half-succeed.

---

## 📂 Command Structure

```bash
purgekit [COMMAND] [ARGS] [FLAGS]
```

### 📋 Global Flags
* `-h`, `--help`: Prints help information and command syntax.
* `-V`, `--version`: Prints the current version of PurgeKit.

---

## ⚙️ Commands Reference

### 1. 🧹 `clean`
Deep-cleans an application: scans for leftover files, folders, shortcuts, services, tasks and registry entries matching the app, then purges the items you approve. It does **not** launch the app's official uninstaller — run that first (or use the UI preflight, which does both).

```bash
purgekit clean [APP_NAME] [FLAGS]
```

#### 📌 Arguments:
* `APP_NAME` *(Required)*: The exact name or a partial match string of the application in the registry or UWP packages.

#### 📌 Flags:
* `--dry-run`: Analyze and list remnant items only; nothing is deleted.
* `--min-score <N>` *(default `55`)*: Only items with a confidence score ≥ N are purged.
* `-y`, `--yes`: Skip the confirmation prompt (for scripts and CI).

#### 💡 Examples:
```bash
# Preview everything that would be purged
purgekit clean "Visual Studio Code" --dry-run

# Scripted purge with a stricter confidence threshold
purgekit clean "Spotify" --yes --min-score 70
```

---

### 2. 🗜️ `wsl`
Manages and compacts Windows Subsystem for Linux (WSL2) virtual drive disks.

```bash
purgekit wsl [SUBCOMMAND] [DISTRO_NAME]
```

#### 📌 Subcommands:
* `list`: Lists all registered WSL distributions, their disk paths, and sizes.
* `compact`: Shuts down WSL and runs DiskPart compaction on the distro's VHDX file.
* `sparse`: Sets the NTFS sparse file attribute on the distro's virtual drive disk.

#### 💡 Examples:
```bash
# List all distributions
purgekit wsl list

# Compact Ubuntu's virtual disk (requires Administrator rights)
purgekit wsl compact "Ubuntu"

# Configure Ubuntu's virtual disk to run in sparse (auto-shrink) mode
purgekit wsl sparse "Ubuntu"
```

---

### 3. 🧹 `cache`
Cleans developer tool directories, build outputs, and package registries.

```bash
purgekit cache prune [FLAGS] [TOOLS...]
```

#### 📌 Arguments:
* `TOOLS`: Space-separated list of tool caches to prune. Valid values: `npm`, `pip`, `cargo`, `go`, `yarn`, `pnpm`, `composer`, `gradle`, `vscode`.

#### 📌 Flags:
* `--all`: Cleans caches for all detected developer tools on the system.

#### 💡 Examples:
```bash
# Prune Cargo and NPM package caches
purgekit cache prune cargo npm

# Prune caches for all tools on the system
purgekit cache prune --all
```

---

## 💻 Scripting Automation Examples

### 🌀 PowerShell Silent Cleanup Script
Save this script as `CleanDevEnv.ps1` to perform weekly scheduled maintenance on developer rigs:

```powershell
Write-Host "Starting PurgeKit Scheduled Maintenance..." -ForegroundColor Gold

# 1. Clean dev caches to reclaim space
& purgekit cache prune --all

# 2. Compact active WSL virtual disks
& purgekit wsl compact "Ubuntu"

# 3. Clean environment PATH variables
# (removes dead paths automatically)
& purgekit clean-path --prune-broken

Write-Host "Maintenance completed successfully!" -ForegroundColor Green
```
