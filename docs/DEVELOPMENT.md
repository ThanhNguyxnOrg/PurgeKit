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

### Administrator Elevation in Production
PurgeKit ships with an `asInvoker` manifest (`src-tauri/app.manifest`), so the app starts fast and never forces a UAC prompt at launch. High-privilege operations are guarded in the backend (`settings::check_is_admin`); the UI requests an elevated relaunch through the `restart_as_admin` command, which self-elevates with the `runas` verb and exits the old process. The full trust model (LPE prevention, secure child PATH, signature gates) lives in `docs/SECURITY_UAC.md`.

---

## 🧹 Adding a New Developer Cache

Dev-tool cache rules are **compiled in on purpose**: the loader ignores any user-editable JSON file, so on-disk configuration can never point a cleanup at arbitrary paths. To add a tool:

### 1. Add a built-in rule in `src-tauri/src/scanner/cli_dev.rs`
Extend the built-in list returned by `load_dev_tools_rules()` with a `DevToolRule` (works for both the UI and `purgekit-cli cache prune`):
```rust
DevToolRule {
    name: "poetry".into(),
    cmd_to_run: Some("poetry".into()),
    args: vec!["--version".into()],
    clean_command: None,
    cache_path_templates: Some(vec!["{APPDATA}\\pypoetry\\cache".into()]),
    dynamic_cache_cmd: None,
}
```

### 2. Field Guide
- **`name`**: Display name of the developer tool (also the name used by `purgekit-cli cache prune <name>`).
- **`cmd_to_run`** *(optional)*: Executable used to check that the tool is installed; defaults to `name`.
- **`args`**: Arguments passed to the detection command (e.g. `["--version"]`).
- **`clean_command`** *(optional)*: Shell clean command. **Only commands present in `SAFE_DEV_TOOL_CLEAN_COMMANDS`** (same file) are executed via shell; extend that whitelist only with commands that are safe to run unattended. Otherwise the folder-purge fallback is used.
- **`cache_path_templates`** *(optional)*: Cache directories to purge, with `{APPDATA}`, `{LOCALAPPDATA}`, and `{USERPROFILE}` placeholders.
- **`dynamic_cache_cmd`** *(optional)*: Command whose printed path is used as the cache location. The result is only trusted when it resolves inside one of the rule's template roots.

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
