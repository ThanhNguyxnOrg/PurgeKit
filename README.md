# 🛠️ PurgeKit

<div align="center">

![PurgeKit Logo](static/logo.png)

### **The Ultimate Developer-Focused System Uninstaller & Deep Cleaner** 🚀

_Clean 100% of app remnants, developer tool caches, and environment PATH junk with surgical precision._

---

[![Tauri Version](https://img.shields.io/badge/Tauri-v2.0-E85D26?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app)
[![Svelte Version](https://img.shields.io/badge/Svelte-v5.0-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-2024-000000?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![TypeScript](https://img.shields.io/badge/TypeScript-v5.6-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=for-the-badge)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows_10_|_11-0078D6?style=for-the-badge&logo=windows&logoColor=white)](#)

[🔥 Key Features](#-key-features) • [⚡ Quick Start](#-quick-start) • [📐 Architecture](#-architecture) • [📖 Documentation](#-documentation) • [🤝 Contributing](#-contributing)

</div>

---

## 💡 What is PurgeKit?

**PurgeKit** is a professional-grade system uninstaller designed specifically for **Power Users** and **Developers**. 

Unlike standard uninstallers (like Geek Uninstaller or Revo) which often miss deep cache folders, environment configuration files, or local CLI packages, PurgeKit aims for **100% trace-free removal** of software and development ecosystems.

---

## 🔥 Key Features

- **🔎 Active Monitoring & System Snapshots**: Take system filesystem and registry snapshots before and after software installation to track and purge all new entries.
- **🧹 Developer Tool Cache Purger**: Safely scan and clean massive package caches, build logs, and temporary files from tools like `npm`, `pip`, `cargo`, `go`, `pnpm`, `yarn`, `docker`, and more.
- **🍃 Deep Remnants Scanner**: Advanced regex-based scan for orphaned folders in `%AppData%`, `%LocalAppData%`, `%ProgramData%`, and orphaned registry keys in `HKCU`/`HKLM`.
- **🛠️ PATH Environment Cleaner**: Detect broken, duplicate, or dead directories inside Windows User & System `PATH` variables.
- **🖥️ Double GUI & CLI Interfaces**:
  - A beautiful, clean, **Ember Orange** custom UI built with Svelte 5 (Runes) and Tailwind CSS v4.
  - A powerful **CLI execution mode** (`purgekit.exe clean <app-name>`) for script automation.

---

## ⚡ Quick Start

### 📦 Prerequisites
- **Rust Compiler** (MSRV 1.75+)
- **Node.js** (v18+)
- **Git**

### 💻 Run GUI in Development
```bash
# Clone the repository
git clone https://github.com/ThanhNguyxnOrg/PurgeKit.git
cd PurgeKit

# Install dependencies
npm install

# Launch Tauri Development Server
npm run tauri dev
```

### 🐚 Run CLI Mode
```bash
# Build the binary
cargo build --manifest-path src-tauri/Cargo.toml

# Clean an app directly from the terminal
./src-tauri/target/debug/tauri-app.exe clean "AppName"
```

---

## 📖 Documentation

Dive deeper into the system with our detailed guides inside the [`docs/`](docs/) directory:

- [✨ Detailed Feature Overview](docs/FEATURES.md) - Learn how deep scanning and snapshot diffing works under the hood.
- [📐 Architecture Design](docs/ARCHITECTURE.md) - Deep dive into Tauri IPC commands, Rust scanner modules, and SQLite schemas.
- [🔧 Development Guide](docs/DEVELOPMENT.md) - Guidelines for setting up, writing commands, and adding new developer caches.

---

## 🤝 Contributing

We welcome contributions from the developer community! Whether it's adding new dev tool cache presets, optimizing the Rust scanner, or polishing the frontend.

Please check our [Contributing Guidelines](CONTRIBUTING.md) to get started.

---

## 📄 License

PurgeKit is distributed under the **Apache License 2.0**. See the [LICENSE](LICENSE) file for more details.

<div align="center">
  <sub>Made with ❤️ by <a href="https://github.com/ThanhNguyxnOrg">ThanhNguyxnOrg</a></sub>
</div>
