# ✨ PurgeKit Detailed Features

This document provides a detailed breakdown of all core features available in **PurgeKit** and how they work under the hood.

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
