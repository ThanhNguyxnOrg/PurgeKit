# 🚨 PurgeKit Troubleshooting & Safety Guide

<p align="center">
  <img src="https://img.shields.io/badge/Security-UAC_Elevation-red?style=for-the-badge&logo=windows&logoColor=white" alt="Security Badge" />
  <img src="https://img.shields.io/badge/Status-Help_Desk-yellowgreen?style=for-the-badge&logo=gitbook&logoColor=white" alt="Status Badge" />
</p>

This document addresses common errors, permissions warnings, edge-case failures, and safety designs inside **PurgeKit**.

---

## 🛡️ 1. Administrator UAC Elevation Issues

Many operations in PurgeKit interact directly with system files, Windows services, or machine registry keys.

### 🔴 Symptom: "Compaction failed: Access Denied" or "UAC Elevation Required"
* **Cause**: Windows disk utilities (like `diskpart`) and modifying the system registry hive (`HKEY_LOCAL_MACHINE`) require active **Administrator** rights.
* **Solution**:
  1. Close PurgeKit.
  2. Right-click `PurgeKit.exe` (or your terminal if running via dev mode) and select **"Run as Administrator"**.
  3. Verify status: The **Settings** tab will display `Privileged Mode Active` in green.

---

## 🐋 2. WSL2 Disk Compaction Failures

### 🔴 Symptom: VHDX file is locked or fails to compact
* **Cause**: A Linux terminal is running in the background, or a WSL service (like Docker Desktop) has a file handle open on `ext4.vhdx`.
* **Solution**:
  1. PurgeKit attempts to run `wsl --shutdown` automatically before attaching the drive.
  2. If it still fails, manually run `wsl --shutdown` in your terminal and close applications like Docker Desktop.
  3. Ensure no background WSL distribution is active by running `wsl --list --running`.

---

## 🛠️ 3. Active Toolchain Protection Lock

### 🟡 Symptom: "Warning: Active runtime cannot be deleted"
* **Cause**: To protect your development workspace, PurgeKit checks the versions of Rust (`rustup show active-toolchain`) and Node (`node -v`) currently active in your system PATH.
* **Safety Lock Mechanism**:
  * The active compiler version is highlighted in the UI with a green **Active** badge.
  * Attempting to delete this version triggers a warning pop-up warning that deleting it will break command accessibility in your terminal.
  * **Override**: If you explicitly want to remove it, click "Confirm" in the modal warning dialog. You will need to restart your terminal and install another compiler version afterward.

---

## 📦 4. Registry Uninstallation CLI Failures

### 🟡 Symptom: CLI uninstallation fails, but directory is successfully purged
* **Cause**: Some older software installers register broken uninstall commands in the registry, or the uninstaller command requires a shell environment that Tauri's subprocess engine cannot resolve.
* **Purge Fallback Solution**:
  * When the standard CLI command returns a non-zero exit code, PurgeKit doesn't give up.
  * It falls back to direct directory purging, querying the app's installation path and recursively deleting files and keys via elevated Windows file APIs.

---

## 🖥️ 5. PATH Changes Not Showing in Open Terminals

### 🟡 Symptom: Path is cleaned in PurgeKit, but `cmd` or `powershell` still displays the old PATH
* **Cause**: When environment variables are changed, Windows broadcasts a `WM_SETTINGCHANGE` message to all top-level windows. However, already open console windows (CMD/PowerShell) do not listen to this message and will keep the old environment until they are restarted.
* **Solution**:
  * Simply close your current terminal window and open a new one. The new terminal will load the updated PATH registry entries.
