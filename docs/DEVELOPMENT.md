# 🔧 PurgeKit Development Guide (v1.0.0)

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

To add a new tool to the **Dev Tools Cache** panel:

### 1. Register the Tool in Rust Backend (`src-tauri/src/scanner/cli_dev.rs`)
Add your tool specifications inside `get_dev_tools()`:
```rust
// Example: Adding Python Poetry Cache
let mut poetry_info = DevToolInfo {
    name: "poetry".to_string(),
    detected: false,
    version: None,
    path: None,
    cache_path: None,
    cache_size: None,
    clean_command: Some("poetry cache clear --all .".to_string()),
};

// 1. Check if the tool is installed in the system PATH
if let Ok(poetry_path) = which::which("poetry") {
    poetry_info.detected = true;
    poetry_info.path = Some(poetry_path.to_string_lossy().to_string());
    
    // 2. Resolve the tool version by spawning poetry --version
    if let Ok(version_output) = std::process::Command::new("poetry")
        .arg("--version")
        .output() {
        if version_output.status.success() {
            poetry_info.version = Some(String::from_utf8_lossy(&version_output.stdout).trim().to_string());
        }
    }

    // 3. Set the cache directory and calculate its size
    if let Some(mut cache_dir) = dirs::cache_dir() {
        cache_dir.push("pypoetry");
        cache_dir.push("cache");
        poetry_info.cache_path = Some(cache_dir.to_string_lossy().to_string());
        poetry_info.cache_size = Some(get_dir_size(&cache_dir));
    }
}
```

### 2. Implement the Cleaning Command (`clean_dev_tool_cache`)
In the command handler inside `cli_dev.rs`, add the deletion rule to purge the cache folder safely:
```rust
if name == "poetry" {
    if let Some(mut cache_dir) = dirs::cache_dir() {
        cache_dir.push("pypoetry");
        cache_dir.push("cache");
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
        }
    }
}
```

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
