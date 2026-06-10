# 🔧 PurgeKit Development Guide

This guide describes how to configure your development environment, add new features, and handle administrative permissions on Windows.

---

## ⚙️ Development Environment Setup

### 📦 Prerequisites
- **Git**
- **Node.js** (v18+)
- **Rust compiler** (installed via [rustup](https://rustup.rs))
- **C++ Build Tools for Windows** (installed via Visual Studio Installer, select the "Desktop development with C++" workload)

### 🚀 Bootstrapping the Project
1. Run `npm install` to install frontend dependencies.
2. Build the Tauri dependencies:
   ```bash
   npm run tauri dev
   ```
   Tauri will download crates, compile the Rust backend, compile Svelte, and open the app window.

---

## 👑 Windows UAC & Requesting Administrator Quyền (Privileges)

Some functions in PurgeKit (such as cleaning System PATH environment variables or deleting system files under `C:\Program Files`) require **Administrator** access.

### How to enforce Admin permissions when opening the App
In production, you can configure Tauri to automatically trigger the Windows UAC elevation prompt when the executable is launched:
1. Under `src-tauri/`, Tauri uses a resource manifest file (often configured inside `Cargo.toml` or `build.rs` using a `.manifest` resource).
2. To require administrator privileges, you can modify or create an application manifest (e.g. `purgekit.manifest` or embed it in the build script) containing:
   ```xml
   <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
     <security>
       <requestedPrivileges>
         <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
       </requestedPrivileges>
     </security>
   </trustInfo>
   ```
3. Alternatively, users can right-click the built executable and select **"Run as Administrator"**. The frontend checks for this status and displays it in the **Settings** tab.

---

## 🧹 Adding a New Developer Cache

To add a new tool to the **Dev Tools Cache** list:

### 1. Register in Rust Backend (`src-tauri/src/scanner/cli_dev.rs`)
Add your tool info and logic to the `get_dev_tools()` function:
```rust
// Example: Adding Python Poety cache
let mut poetry_info = DevToolInfo {
    name: "poetry".to_string(),
    detected: false,
    version: None,
    path: None,
    cache_path: None,
    cache_size: None,
    clean_command: Some("poetry cache clear --all .".to_string()),
};

// Check if tool exists on PATH
if let Ok(poetry_path) = which::which("poetry") {
    poetry_info.detected = true;
    poetry_info.path = Some(poetry_path.to_string_lossy().to_string());
    
    // Resolve cache directory
    if let Some(mut cache_dir) = dirs::cache_dir() {
        cache_dir.push("pypoetry");
        cache_dir.push("cache");
        poetry_info.cache_path = Some(cache_dir.to_string_lossy().to_string());
        poetry_info.cache_size = Some(get_dir_size(&cache_dir));
    }
}
```

### 2. Implement the Cleaning logic (`clean_dev_tool_cache` command)
Add the execution code to wipe the cache folder or trigger the command. Usually, deleting the cache directory is safer and faster:
```rust
std::fs::remove_dir_all(&cache_path)?;
```

---

## 🎨 Frontend Styling & Component Guidelines

### Svelte 5 Runes
Always use Svelte 5 runes instead of older structures:
```html
<script lang="ts">
  // Declaring reactive states
  let searchQuery = $state("");
  
  // Bindable props (inputs/outputs)
  let { activeTab = $bindable() } = $props();
  
  // Derived state (computes automatically)
  let filteredList = $derived(items.filter(i => i.includes(searchQuery)));
</script>
```

### Design System Rules
- **Font Scale**: Page Titles are `text-2xl font-bold`, section subtitles are `text-xs text-text-muted`.
- **Bo-góc (Border Radius)**:
  - Small elements (badges): `rounded` (4px).
  - Standard elements (inputs, buttons, list rows): `rounded-lg` (8px).
  - Modals and large layouts: `rounded-xl` (12px).
  - Do NOT use `rounded-2xl` or larger!
- **Accents**: Use `bg-accent` and `text-accent` for highlighting. Do NOT add glowing drop-shadows or gradients.
