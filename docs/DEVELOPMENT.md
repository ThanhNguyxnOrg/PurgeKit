# 🔧 PurgeKit Development Guide

This guide describes how to configure your local development environment, add new features, compile the binaries, and manage Administrator elevation requests on Windows.

---

## ⚙️ Development Environment Setup

### 📦 Prerequisites
* **Git**: To clone the repository and manage revisions.
* **Node.js** (v18.0 or newer): To compile Svelte and build the Vite frontend bundles.
* **Rust Toolchain** (MSRV 1.77+): Installed via [rustup](https://rustup.rs). Requires stable channel compilers.
* **C++ Build Tools for Windows**: Open the Visual Studio Installer, select the "Desktop development with C++" workload, and install it. This is required by Cargo to compile native dependencies on Windows.

### 🚀 Bootstrapping the Project
1. Clone the repository and install npm packages:
   ```bash
   git clone https://github.com/ThanhNguyxnOrg/PurgeKit.git
   cd PurgeKit
   npm install
   ```
2. Start the Tauri development server:
   ```bash
   npm run tauri dev
   ```
   Tauri will automatically fetch Rust dependencies, compile the background executable, build the Svelte frontend, and launch the developer window with Hot Module Replacement (HMR) enabled.

---

## 👑 Windows UAC Elevation (Administrator Privileges)

Several modules in PurgeKit interact directly with system files and registries that require Administrator privileges:
* **System PATH Cleaner**: Deleting dead System PATH variables.
* **WSL2 Disk Shrinker**: Running `diskpart` to attach and compact virtual disks (`.vhdx`).
* **Apps Manager**: Uninstalling applications installed in `%ProgramFiles%` or modifying `HKLM` keys.

### Running with Admin Rights during Development
You can right-click your terminal application (e.g. PowerShell or Command Prompt) and select **"Run as Administrator"**, then run `npm run tauri dev`. The app will inherit the elevated token, and the **Settings** view will display **"Privileged Mode Active"**.

### Enforcing Administrator Elevation in Production
To ensure the compiled application always requests elevation via Windows User Account Control (UAC) when launched:
1. Under `src-tauri/capabilities/`, capabilities can configure privileges.
2. In Tauri v2, you can also embed an application manifest inside the executable resource. In `src-tauri/Cargo.toml` or `build.rs`, Tauri's build tool automatically handles manifest embedding.
3. Users can manually configure the executable properties to **"Run this program as an administrator"**, or let PurgeKit prompt UAC natively when calling high-privilege Tauri commands by executing child processes with `runas` verbs.

---

## 🧹 Adding a New Developer Cache

To add a new tool to the **Dev Tools Cache** panel, you no longer need to write Rust code. You can simply add a new rule entry to the dynamic devtools rules JSON configuration file:

### 1. Add Entry to `devtools_rules.json`
Locate the rules file at:
`%LocalAppData%\PurgeKit\devtools_rules.json`

Add a new JSON object to the array. For example, to add the **Python Poetry** cache:
```json
{
  "name": "poetry",
  "cmd_to_run": "poetry",
  "args": ["--version"],
  "clean_command": "",
  "cache_path_templates": ["{APPDATA}\\pypoetry\\cache"],
  "dynamic_cache_cmd": ""
}
```

### 2. Field Guide for New Entries
- **`name`**: The display name of the developer tool.
- **`cmd_to_run`**: (Optional) The executable name used to check if the tool is installed. If omitted, defaults to `name`.
- **`args`**: Array of arguments passed to check installation (e.g. `["--version"]`).
- **`clean_command`**: (Optional) Standard shell clean command. Note that only whitelisted commands will execute via shell. If you want PurgeKit to purge the folder directly, leave this empty or unwhitelisted.
- **`cache_path_templates`**: (Optional) List of cache directories to purge. You can use placeholders like `{APPDATA}`, `{LOCALAPPDATA}`, and `{USERPROFILE}`.
- **`dynamic_cache_cmd`**: (Optional) A command to run to query the cache path dynamically.

---

## 🎨 Frontend Guidelines & Design System

To maintain the premium look of PurgeKit, adhere to the following UI standards:

### Svelte 5 Runes
Write all state declarations using Svelte 5 runes:
```html
<script lang="ts">
  // Reactive state
  let selectedCategory = $state("all");
  
  // Bindable props
  let { activeTab = $bindable() } = $props();
  
  // Derived state (auto-computes)
  let itemsCount = $derived(items.length);
</script>
```

### Design System Constants
* **Color Palette (Dark Glassmorphic Tech)**:
  - Background: `bg-app-bg` (deep dark grey/black).
  - Cards/Containers: `bg-surface-bg/50` or `bg-sidebar-bg/60` with thin `border-border-default` borders.
  - Active Accents: `bg-accent` (deep orange/ember) and `text-accent`.
  - Danger/Destructive: `bg-danger/10` and `text-danger` (with border).
* **Border Radii**:
  - Small items (badges): `rounded` (4px).
  - Interactive components (buttons, text inputs, list elements): `rounded-lg` (8px).
  - Modals and large containers: `rounded-xl` (12px).
  - *Constraint*: Do NOT use `rounded-2xl` or larger.
* **Layout Sizing**:
  - Main containers must use `h-full` to inherit the parent layout window height.
  - Tables must be wrapped in `<div class="overflow-x-auto w-full">` and have a `min-w-[800px]` width constraint to support horizontal scrolling on small/split windows.
