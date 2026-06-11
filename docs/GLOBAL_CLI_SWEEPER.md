# 📦 Global CLI Package Sweeper

The **Global CLI Package Sweeper** audits and cleans globally-installed packages and binary tools downloaded by package managers like npm, yarn, pnpm, cargo, pip, and others.

---

## 🔎 Discovery Hives & Scan Paths

PurgeKit scans standard global directories created by package managers on Windows to inventory installed command-line utilities.

| Manager | Scan Target Locations | Metadata Source |
|---|---|---|
| **npm** | `%APPDATA%\npm\node_modules\` | `package.json` version |
| **yarn** | `%APPDATA%\Yarn\global\node_modules\`<br>`%LocalAppData%\Yarn\Data\global\node_modules\` | `package.json` version |
| **pnpm** | `%LocalAppData%\pnpm\node_modules\`<br>`%APPDATA%\pnpm\node_modules\` | `package.json` version |
| **bun** | `%USERPROFILE%\.bun\install\global\node_modules\` | `package.json` version |
| **cargo** | `%USERPROFILE%\.cargo\bin\` | `%USERPROFILE%\.cargo\.crates2.json` / `.crates.toml` |
| **pip** | `%LocalAppData%\Programs\Python\Python*` | `pip list` parsing |
| **dotnet** | `%USERPROFILE%\.dotnet\tools\` | Tool manifest parsing |
| **composer**| `%APPDATA%\Composer\vendor\` | `composer.json` metadata |
| **go** | `%USERPROFILE%\go\bin\` | Direct binary detection |

---

## 📋 Package Managers Crawling Mechanisms

### 1. NodeJS Package Managers (npm, Yarn, pnpm, Bun)
*   **Directory Traversal**: The scanner crawls the target `node_modules` folder.
*   **Namespace Scoped Packages**: The crawler checks if a folder starts with `@` (e.g. `@angular`). If it does, it descends into the subfolder to parse the actual packages (e.g. `@angular/cli`).
*   **Metadata Parsing**: For each package, it opens and parses `package.json` to extract:
    *   `version`: The active version string.
    *   `description`: The package description.
*   **Size Calculation**: Recursively sums up all files in the package directory to show exact byte usage.

---

### 2. Rust (Cargo Global Packages)
*   Instead of guess-scanning the `%USERPROFILE%\.cargo\bin` folder (which only contains compiled `.exe` files), PurgeKit opens and parses `%USERPROFILE%\.cargo\.crates2.json` (or `.crates.toml` as a legacy fallback).
*   It reads the `installs` map to locate the package name, registry source, and version.
*   It matches the binary with the compiled executable in `.cargo/bin/` to show the correct file size.

---

### 3. Python (pip Global Packages)
*   PurgeKit queries the system Python interpreter by spawning:
    `pip list --format=json`
*   If successful, it parses the JSON list of package names and versions.
*   It locates packages in the global Python `site-packages` directory to calculate disk space usage.

---

## 🧹 Safe Uninstallation & Sweeping

When a user selects a global CLI package in the UI and clicks **Uninstall**:
1.  **Official Command Execution**: PurgeKit attempts to run the native uninstallation command for the package manager, redirecting stdout/stderr:
    *   *npm*: `npm uninstall -g [package-name]`
    *   *Yarn*: `yarn global remove [package-name]`
    *   *pnpm*: `pnpm remove -g [package-name]`
    *   *Cargo*: `cargo uninstall [package-name]`
    *   *pip*: `pip uninstall [package-name] -y`
2.  **Filesystem Purging Fallback**: If the toolchain runner is not installed in the system PATH (which happens when environments are deleted manually), PurgeKit falls back to manually removing the package folders from the global install paths and updating registry keys.
