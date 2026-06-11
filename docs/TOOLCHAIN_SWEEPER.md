# 🛠️ Toolchain Version Sweeper Specifications

<p align="center">
  <img src="https://img.shields.io/badge/Rust-Rustup-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rustup Badge" />
  <img src="https://img.shields.io/badge/Node-NVM_|_FNM-green?style=for-the-badge&logo=node.js&logoColor=white" alt="Node Runtimes Badge" />
</p>

This document contains the directory mappings, version parsing logics, and safety mechanisms of the **Toolchain Version Sweeper** module.

---

## 🔎 1. Compiler Toolchains & Storage Locations

PurgeKit scans the following directory structures to discover installed runtime compilers:

### 🦀 Rustup (Rust Compiler)
* **Directory**: `%USERPROFILE%\.rustup\toolchains`
* **Sub-directories**: Contains compiled Rust toolchains named after their target triples, e.g.:
  - `stable-x86_64-pc-windows-msvc`
  - `nightly-2026-03-15-x86_64-pc-windows-msvc`

### 🟢 NVM for Windows (Node Version Manager)
* **Directory**: Reads `%NVM_HOME%` environment variable. Falls back to `%APPDATA%\nvm`.
* **Sub-directories**: Looks for folders matching the version naming pattern (starting with `v` and followed by digits, e.g. `v18.16.0`, `v20.10.0`).

### ⚡ FNM (Fast Node Manager)
* **Directory**: Reads `%FNM_DIR%` environment variable. Also checks these default directories:
  - `%USERPROFILE%\.fnm\node-versions`
  - `%LOCALAPPDATA%\fnm\node-versions`
  - `%APPDATA%\fnm\node-versions`
* **Sub-directories**: Contains Node.js versions (e.g. `v20.11.1`).

---

## 🔒 2. Active Version Detection (Safety Lock)

To protect your development environment, PurgeKit checks the currently active compiler/runtime versions:

### Node.js Active Check
* Spawns command: `node -v`
* Reads stdout: returns e.g. `v20.10.0`
* Matching: Marks the NVM/FNM version named `v20.10.0` as `Active`.

### Rustup Active Check
* Spawns command: `rustup show active-toolchain`
* Reads stdout: returns e.g. `stable-x86_64-pc-windows-msvc (default)`
* Parsing: Extracts the first word (`stable-x86_64-pc-windows-msvc`).
* Matching: Marks the Rustup toolchain matching this directory name as `Active`.

In Svelte UI, active runtimes display an `Active` badge, and trying to delete them prompts a warning message.

---

## 🗑️ 3. Uninstallation Execution Flow

When you click **Uninstall** on a compiler version, PurgeKit runs the following sequence:

```
            +---------------------------------+
            |  Trigger Toolchain Deletion     |
            +---------------------------------+
                             |
                             v
            +---------------------------------+
            | Spawn Official CLI command      |
            | (e.g., rustup toolchain delete) |
            +---------------------------------+
                             |
               +--------------+--------------+
               |                             |
      [Exit Code: 0]                [Command Fails /
              |                     Missing from PATH]
              v                             v
      +---------------+             +-----------------+
      |  Clean Exit   |             | Fallback:       |
      |  Success!     |             | Direct Directory|
      +---------------+             | Purge           |
                                    +-----------------+
                                            |
                                            v
                                    +-----------------+
                                    | Delete Folder   |
                                    | via Locker API  |
                                    +-----------------+
```

This hybrid approach ensures that even if you have broken environmental paths or missing CLI binaries, the version folders can be cleaned successfully, reclaiming space instantly.
